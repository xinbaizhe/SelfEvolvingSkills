use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::path::Path;

use crate::db;
use crate::utils::time::now_string;

async fn fetch_github_skills(query: &str, page: i64, per_page: i64) -> Result<Vec<Value>> {
    let search_query = format!("{} skill OR SKILL.md", query);
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Self-Evolving-Skills"));
    headers.insert(ACCEPT, HeaderValue::from_static("application/vnd.github+json"));

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(20))
        .build()?;

    let response = client
        .get("https://api.github.com/search/repositories")
        .query(&[
            ("q", search_query.as_str()),
            ("sort", "stars"),
            ("order", "desc"),
            ("page", &page.to_string()),
            ("per_page", &per_page.clamp(1, 50).to_string()),
        ])
        .send()
        .await
        .map_err(|err| anyhow!("GitHub network request failed: {}", err))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        let message = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|value| value.get("message").and_then(Value::as_str).map(ToString::to_string))
            .unwrap_or_else(|| text.chars().take(300).collect());
        return Err(anyhow!("GitHub API request failed: HTTP {} {}", status.as_u16(), message));
    }

    let payload: Value = serde_json::from_str(&text)?;
    payload
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| anyhow!("GitHub API response is missing items"))
}

fn save_github_skills(conn: &Connection, items: &[Value]) -> Result<()> {
    let now = now_string();
    for item in items {
        let Some(repo_full_name) = item.get("full_name").and_then(Value::as_str) else {
            continue;
        };
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(repo_full_name);
        let repo_url = item
            .get("html_url")
            .and_then(Value::as_str)
            .unwrap_or("");
        if repo_url.is_empty() {
            continue;
        }
        let stars = item
            .get("stargazers_count")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let description = item
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");
        let default_branch = item
            .get("default_branch")
            .and_then(Value::as_str)
            .unwrap_or("main");
        let file_url = format!("{}/blob/{}/SKILL.md", repo_url, default_branch);

        conn.execute(
            "INSERT INTO community_skills
             (name, repo_full_name, repo_url, stars, description, file_url, installed, fetched_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?7)
             ON CONFLICT(name, repo_full_name) DO UPDATE SET
                repo_url = excluded.repo_url,
                stars = excluded.stars,
                description = excluded.description,
                file_url = excluded.file_url,
                fetched_at = excluded.fetched_at",
            params![name, repo_full_name, repo_url, stars, description, file_url, now],
        )?;
    }
    Ok(())
}

fn search_cached_community_skills(
    conn: &Connection,
    query_str: &str,
    page: i64,
    per_page: i64,
) -> Result<Value> {
    let search = format!("%{}%", query_str);
    let total: i64 = conn.query_row(
        "SELECT COUNT(id) FROM community_skills WHERE name LIKE ?1 OR description LIKE ?1 OR repo_full_name LIKE ?1",
        params![search],
        |row| row.get(0),
    )?;

    let mut stmt = conn.prepare(
        "SELECT id, name, repo_full_name, repo_url, stars, description, installed, fetched_at
         FROM community_skills
         WHERE name LIKE ?1 OR description LIKE ?1 OR repo_full_name LIKE ?1
         ORDER BY stars DESC
         LIMIT ?2 OFFSET ?3",
    )?;

    let items = stmt
        .query_map(params![search, per_page, (page - 1).max(0) * per_page], |row| {
            let installed = row.get::<_, i64>(6)? != 0;
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "repo": row.get::<_, String>(2)?,
                "repo_url": row.get::<_, String>(3)?,
                "stars": row.get::<_, i64>(4)?,
                "description": row.get::<_, Option<String>>(5)?,
                "installed": installed,
                "source": "GitHub",
                "status": if installed { "installed" } else { "available" },
                "fetched_at": row.get::<_, Option<String>>(7)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(json!({ "items": items, "total": total, "page": page, "per_page": per_page }))
}

pub(crate) async fn search_community_skills(
    db_path: &Path,
    query_str: &str,
    page: i64,
    per_page: i64,
) -> Result<Value> {
    let query = if query_str.trim().is_empty() {
        "agent skill"
    } else {
        query_str.trim()
    };

    let items = fetch_github_skills(query, page, per_page).await?;
    if !items.is_empty() {
        let conn = db::open_conn(db_path)?;
        save_github_skills(&conn, &items)?;
    }

    let conn = db::open_conn(db_path)?;
    search_cached_community_skills(&conn, query, page, per_page)
}

pub(crate) fn get_installed_skills(conn: &Connection) -> Result<Value> {
    let mut stmt = conn.prepare(
        "SELECT id, name, repo_full_name, repo_url, stars, description, installed, fetched_at
         FROM community_skills WHERE installed = 1 ORDER BY fetched_at DESC",
    )?;

    let items = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "repo": row.get::<_, String>(2)?,
                "repo_url": row.get::<_, String>(3)?,
                "stars": row.get::<_, i64>(4)?,
                "description": row.get::<_, Option<String>>(5)?,
                "installed": true,
                "source": "GitHub",
                "status": "installed",
                "fetched_at": row.get::<_, Option<String>>(7)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(json!({ "items": items, "total": items.len() }))
}

pub(crate) async fn fetch_top_community_skills(
    db_path: &Path,
    clusters: &[crate::services::workflow_service::WorkflowCluster],
) -> Result<i64> {
    let conn = db::open_conn(db_path)?;
    let before = crate::db::count_table(&conn, "community_skills").unwrap_or(0);
    drop(conn);

    let mut terms: Vec<&str> = clusters
        .iter()
        .filter(|cluster| cluster.can_generate_skill)
        .map(|cluster| cluster.name.as_str())
        .take(3)
        .collect();
    if terms.is_empty() {
        terms.push("agent skill");
    }

    let mut all_items = Vec::new();
    let mut errors = Vec::new();
    for term in &terms {
        match fetch_github_skills(term, 1, 10).await {
            Ok(items) => all_items.extend(items),
            Err(err) => errors.push(format!("{term}: {err}")),
        }
    }

    if all_items.is_empty() && !errors.is_empty() {
        return Err(anyhow!("community skill search failed: {}", errors.join("; ")));
    }

    if !all_items.is_empty() {
        let conn = db::open_conn(db_path)?;
        save_github_skills(&conn, &all_items)?;
    }

    let conn = db::open_conn(db_path)?;
    let after = crate::db::count_table(&conn, "community_skills").unwrap_or(before);
    Ok((after - before).max(0))
}

pub(crate) fn compare_with_community(
    conn: &Connection,
    clusters: &[crate::services::workflow_service::WorkflowCluster],
) -> i64 {
    let mut comparisons = 0;

    for cluster in clusters.iter().filter(|cluster| cluster.can_generate_skill) {
        let Ok(workflow_id) = crate::services::workflow_service::workflow_id_for_cluster(conn, cluster) else {
            continue;
        };
        let lower_name = cluster.name.to_lowercase();
        let mut stmt = match conn.prepare(
            "SELECT name, repo_url, stars, description FROM community_skills
             WHERE lower(name) LIKE ?1 OR lower(description) LIKE ?1 OR lower(repo_full_name) LIKE ?1
             ORDER BY stars DESC LIMIT 5",
        ) {
            Ok(stmt) => stmt,
            Err(_) => continue,
        };

        let similar = stmt
            .query_map(params![format!("%{}%", lower_name)], |row| {
                Ok(json!({
                    "name": row.get::<_, String>(0)?,
                    "url": row.get::<_, String>(1)?,
                    "stars": row.get::<_, i64>(2)?,
                    "description": row.get::<_, Option<String>>(3)?,
                }))
            })
            .map(|rows| rows.filter_map(|row| row.ok()).collect::<Vec<Value>>())
            .unwrap_or_default();

        if !similar.is_empty() {
            let now = now_string();
            let _ = conn.execute(
                "UPDATE workflow_clusters SET similar_skills = ?2, updated_at = ?3 WHERE id = ?1",
                params![
                    workflow_id,
                    serde_json::to_string(&similar).unwrap_or_default(),
                    now,
                ],
            );
            comparisons += 1;
        }
    }

    comparisons
}
