use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::path::Path;

mod comparison;
mod utils;

use comparison::llm_recommend_community_skills;

pub(crate) use comparison::{compare_with_community, compare_with_llm, fetch_top_community_skills};

#[derive(Debug, Clone)]
pub(super) struct CommunityRecommendation {
    pub(super) name: String,
    pub(super) repo_full_name: String,
    pub(super) repo_url: String,
    pub(super) stars: i64,
    pub(super) description: String,
    pub(super) reason: String,
    pub(super) source: String,
    pub(super) license: Option<String>,
    pub(super) pushed_at: Option<String>,
    pub(super) relevance_score: f64,
    pub(super) quality_score: f64,
    pub(super) weighted_score: f64,
}

#[derive(Debug, Clone)]
pub(super) struct LlmConfig {
    pub(super) base_url: String,
    pub(super) api_key: String,
    pub(super) model: String,
    pub(super) api_format: String,
}

pub(crate) async fn search_community_skills(
    db_path: &Path,
    query_str: &str,
    page: i64,
    per_page: i64,
) -> Result<Value> {
    let query = if query_str.trim().is_empty() {
        "AI coding agent skills"
    } else {
        query_str.trim()
    }
    .to_string();

    {
        let conn = crate::db::open_conn(db_path)?;
        ensure_seed_recommendations(&conn, &query)?;
    }

    spawn_background_refresh(db_path.to_path_buf(), query.clone(), page, per_page.min(10));

    let conn = crate::db::open_conn(db_path)?;
    search_cached_community_skills(&conn, &query, page, per_page.min(10))
}

pub(crate) fn get_installed_skills(conn: &Connection) -> Result<Value> {
    search_cached_community_skills(conn, "", 1, 10)
}

pub(crate) fn get_community_skill_detail(conn: &Connection, id: i64) -> Result<Value> {
    conn.query_row(
        "SELECT id, name, repo_full_name, repo_url, stars, description, skill_md_content, file_url,
                installed, verified, relevance_score, quality_score, weighted_score, license,
                pushed_at, matched_file, readme_excerpt, source, recommendation_reason, topic,
                fetched_at, created_at
         FROM community_skills WHERE id = ?1",
        [id],
        |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "repo": row.get::<_, String>(2)?,
                "repo_url": row.get::<_, String>(3)?,
                "stars": row.get::<_, i64>(4)?,
                "description": row.get::<_, Option<String>>(5)?,
                "skill_md_content": row.get::<_, Option<String>>(6)?,
                "file_url": row.get::<_, Option<String>>(7)?,
                "installed": row.get::<_, i64>(8)? != 0,
                "verified": row.get::<_, i64>(9)? != 0,
                "relevance_score": row.get::<_, Option<f64>>(10)?,
                "quality_score": row.get::<_, Option<f64>>(11)?,
                "weighted_score": row.get::<_, Option<f64>>(12)?,
                "license": row.get::<_, Option<String>>(13)?,
                "pushed_at": row.get::<_, Option<String>>(14)?,
                "matched_file": row.get::<_, Option<String>>(15)?,
                "readme_excerpt": row.get::<_, Option<String>>(16)?,
                "source": row.get::<_, Option<String>>(17)?,
                "recommendation_reason": row.get::<_, Option<String>>(18)?,
                "topic": row.get::<_, Option<String>>(19)?,
                "fetched_at": row.get::<_, Option<String>>(20)?,
                "created_at": row.get::<_, Option<String>>(21)?,
            }))
        },
    )
    .map_err(|e| anyhow!("Community skill not found: {}", e))
}

pub(super) async fn fetch_github_skill_candidates(
    query: &str,
    page: i64,
    per_page: i64,
) -> Result<Vec<Value>> {
    let search_query = format!(
        "{} (awesome OR popular OR \"best practices\" OR framework OR toolkit OR agents OR prompts OR instructions) (skill OR SKILL.md OR agent instructions OR prompt engineering) stars:>1000",
        query
    );
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Self-Evolving-Skills"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(12))
        .build()?;

    let response = client
        .get("https://api.github.com/search/repositories")
        .query(&[
            ("q", search_query.as_str()),
            ("sort", "stars"),
            ("order", "desc"),
            ("page", &page.to_string()),
            ("per_page", &per_page.clamp(1, 10).to_string()),
        ])
        .send()
        .await
        .map_err(|err| anyhow!("GitHub network request failed: {}", err))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_else(|e| {
        format!("[Failed to read response body: {e}]")
    });
    if !status.is_success() {
        let message = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|value| {
                value
                    .get("message")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .unwrap_or_else(|| text.chars().take(300).collect());
        return Err(anyhow!(
            "GitHub API request failed: HTTP {} {}",
            status.as_u16(),
            message
        ));
    }

    let payload: Value = serde_json::from_str(&text)?;
    payload
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| anyhow!("GitHub API response is missing items"))
}

pub(super) fn github_candidates_to_recommendations(
    query: &str,
    items: &[Value],
) -> Vec<CommunityRecommendation> {
    let mut recommendations = items
        .iter()
        .filter_map(|item| {
            let repo_full_name = item.get("full_name").and_then(Value::as_str)?;
            let repo_url = item.get("html_url").and_then(Value::as_str)?;
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(repo_full_name);
            let description = item
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let stars = item
                .get("stargazers_count")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            let license = item
                .get("license")
                .and_then(|value| value.get("spdx_id"))
                .and_then(Value::as_str)
                .filter(|value| *value != "NOASSERTION")
                .map(ToString::to_string);
            let pushed_at = item
                .get("pushed_at")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            let relevance =
                utils::metadata_relevance_score(query, name, &description, repo_full_name);
            if relevance < 0.08 {
                return None;
            }
            let quality =
                utils::metadata_quality_score(name, &description, repo_full_name);
            let weighted = utils::weighted_score(
                stars,
                relevance,
                quality,
                license.as_deref(),
                pushed_at.as_deref(),
            );
            let reason = utils::github_metadata_reason(
                query, name, &description, repo_full_name, stars,
            );
            Some(CommunityRecommendation {
                name: name.to_string(),
                repo_full_name: repo_full_name.to_string(),
                repo_url: repo_url.to_string(),
                stars,
                description,
                reason,
                source: "GitHub".to_string(),
                license,
                pushed_at,
                relevance_score: relevance,
                quality_score: quality,
                weighted_score: weighted,
            })
        })
        .collect::<Vec<_>>();
    recommendations.sort_by(|a, b| {
        b.weighted_score
            .partial_cmp(&a.weighted_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    recommendations
}

pub(super) fn search_cached_community_skills(
    conn: &Connection,
    query_str: &str,
    page: i64,
    per_page: i64,
) -> Result<Value> {
    let total: i64 = conn.query_row(
        "SELECT COUNT(id) FROM community_skills WHERE verified = 1 AND topic = ?1",
        [query_str],
        |row| row.get(0),
    )?;

    let mut stmt = conn.prepare(
        "SELECT id, name, repo_full_name, repo_url, stars, description, installed, fetched_at,
                relevance_score, quality_score, weighted_score, license, pushed_at, matched_file,
                source, recommendation_reason
         FROM community_skills
         WHERE verified = 1 AND topic = ?1
         ORDER BY
           CASE WHEN source = 'LLM' AND stars <= 0 THEN 0 ELSE 1 END DESC,
           weighted_score DESC, stars DESC
         LIMIT ?2 OFFSET ?3",
    )?;

    let items = stmt
        .query_map(
            params![query_str, per_page, (page - 1).max(0) * per_page],
            |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "repo": row.get::<_, String>(2)?,
                    "repo_url": row.get::<_, String>(3)?,
                    "stars": row.get::<_, i64>(4)?,
                    "description": row.get::<_, Option<String>>(5)?,
                    "installed": false,
                    "source": row.get::<_, Option<String>>(14)?.unwrap_or_else(|| "GitHub".to_string()),
                    "status": "recommended",
                    "fetched_at": utils::format_timestamp(row.get::<_, Option<String>>(7)?),
                    "relevance_score": row.get::<_, Option<f64>>(8)?.unwrap_or(0.0),
                    "quality_score": row.get::<_, Option<f64>>(9)?.unwrap_or(0.0),
                    "weighted_score": row.get::<_, Option<f64>>(10)?.unwrap_or(0.0),
                    "license": row.get::<_, Option<String>>(11)?,
                    "pushed_at": utils::format_timestamp(row.get::<_, Option<String>>(12)?),
                    "matched_file": row.get::<_, Option<String>>(13)?,
                    "recommendation_reason": row.get::<_, Option<String>>(15)?,
                }))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(json!({ "items": items, "total": total, "page": page, "per_page": per_page }))
}

pub(super) fn spawn_background_refresh(
    db_path: std::path::PathBuf,
    query: String,
    page: i64,
    per_page: i64,
) {
    tokio::spawn(async move {
        refresh_seed_recommendations_with_metadata(&db_path, &query).await;

        let llm_items = llm_recommend_community_skills(&db_path, &query)
            .await
            .unwrap_or_default();
        if !llm_items.is_empty() {
            if let Ok(conn) = crate::db::open_conn(&db_path) {
                let _ = utils::save_recommendations(&conn, &query, &llm_items);
            }
        }

        let github_items = match fetch_github_skill_candidates(&query, page, per_page).await {
            Ok(candidates) => github_candidates_to_recommendations(&query, &candidates),
            Err(_) => Vec::new(),
        };
        if !github_items.is_empty() {
            if let Ok(conn) = crate::db::open_conn(&db_path) {
                let _ = utils::save_recommendations(&conn, &query, &github_items);
            }
        }

        fetch_and_cache_skill_contents(&db_path).await;
    });
}

pub(super) async fn fetch_and_cache_skill_contents(db_path: &std::path::Path) {
    let ids: Vec<(i64, String, Option<String>)> = match crate::db::open_conn(db_path) {
        Ok(conn) => {
            let mut stmt = match conn.prepare(
                "SELECT id, repo_full_name, file_url FROM community_skills
                 WHERE skill_md_content IS NULL
                   AND repo_full_name NOT LIKE 'llm-recommendation/%'
                   AND repo_full_name NOT LIKE 'skill-seed/%'
                 LIMIT 20",
            ) {
                Ok(stmt) => stmt,
                Err(_) => return,
            };
            let rows_result = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            });
            let collected: Vec<(i64, String, Option<String>)> = match rows_result {
                Ok(mapped) => mapped.filter_map(|r| r.ok()).collect(),
                Err(_) => return,
            };
            collected
        }
        Err(_) => return,
    };

    for (id, repo_full_name, file_url) in ids {
        let Some(raw_url) =
            crate::community_skill_raw_url(&repo_full_name, file_url.as_deref())
        else {
            continue;
        };
        let content = match crate::fetch_community_skill_markdown(&raw_url).await {
            Ok(c) => c,
            Err(_) => continue,
        };
        if content.trim().is_empty() {
            continue;
        }
        if let Ok(conn) = crate::db::open_conn(db_path) {
            let _ = conn.execute(
                "UPDATE community_skills SET skill_md_content = ?1, fetched_at = ?2 WHERE id = ?3",
                params![content, crate::utils::time::now_string(), id],
            );
        }
    }
}

fn ensure_seed_recommendations(conn: &Connection, query: &str) -> Result<()> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(id) FROM community_skills WHERE verified = 1 AND topic = ?1",
        [query],
        |row| row.get(0),
    )?;
    let missing_reason: i64 = conn.query_row(
        "SELECT COUNT(id) FROM community_skills
         WHERE verified = 1
           AND topic = ?1
           AND (recommendation_reason IS NULL
                OR recommendation_reason = ''
                OR recommendation_reason LIKE '该仓库与%')",
        [query],
        |row| row.get(0),
    )?;
    if count < 5 || missing_reason > 0 {
        utils::save_recommendations(conn, query, &seed_recommendations(conn, query))?;
    }
    Ok(())
}

async fn refresh_seed_recommendations_with_metadata(db_path: &Path, query: &str) {
    let items = match crate::db::open_conn(db_path) {
        Ok(conn) => seed_recommendations(&conn, query),
        Err(_) => Vec::new(),
    };
    if items.is_empty() {
        return;
    }
    let mut items = items;
    comparison::enrich_recommendations_with_github_metadata(&mut items).await;
    if let Ok(conn) = crate::db::open_conn(db_path) {
        let _ = utils::save_recommendations(&conn, query, &items);
    }
}

fn seed_recommendations(conn: &Connection, query: &str) -> Vec<CommunityRecommendation> {
    let skills = match query_local_skills_for_seed(conn) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    if skills.is_empty() {
        return Vec::new();
    }

    skills
        .into_iter()
        .enumerate()
        .map(|(index, (name, description, category))| {
            let desc = if description.is_empty() {
                format!("本地 Skill「{}」— 基于你的实际使用数据推荐相关社区项目", name)
            } else {
                description
            };
            let relevance = utils::metadata_relevance_score(query, &name, &desc, &name)
                .max(0.78 - index as f64 * 0.05);
            let quality = (0.88 - index as f64 * 0.04).max(0.70);
            let cat_hint = if category.is_empty() {
                String::new()
            } else {
                format!("，归类为「{}」", category)
            };
            CommunityRecommendation {
                name: format!("{} (社区参考)", name),
                repo_full_name: format!("skill-seed/{}", utils::slug(&name)),
                repo_url: format!(
                    "https://github.com/search?q={}+skill+agent",
                    utils::url_query(&name)
                ),
                stars: 0,
                description: desc,
                reason: format!(
                    "它是什么：根据你本地已安装的 Skill「{}」{}生成的社区参考条目。\n\n对 Skills 工作台有什么用：你的工作流中已在使用此 Skill，社区中可能有更优的同类实现可以参考、对比或合并。\n\n适合什么时候看：当你觉得当前 Skill 的指令不够精准、覆盖场景不够全，或者想看看社区如何解决同类问题时打开。\n\n建议优先看什么：搜索结果的 README、示例目录、agent instructions 或 workflow 文件。",
                    name, cat_hint
                ),
                source: "Seed".to_string(),
                license: None,
                pushed_at: None,
                relevance_score: relevance,
                quality_score: quality,
                weighted_score: (0.82 - index as f64 * 0.05).max(0.62),
            }
        })
        .collect()
}

fn query_local_skills_for_seed(conn: &Connection) -> Result<Vec<(String, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT name, COALESCE(description, ''), COALESCE(category, '')
         FROM skills
         WHERE source_type = 'user'
         ORDER BY usage_count DESC, session_count DESC
         LIMIT 10",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();
    Ok(rows)
}
