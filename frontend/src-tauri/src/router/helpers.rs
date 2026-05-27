use std::{fs, path::Path};

use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use crate::{
    count_table, count_where, now_string, open_conn, path_to_string, sanitize_file_name, PageQuery,
};
use crate::services;

// ---- Community skill helpers ----

pub(crate) async fn install_community_skill(
    db_path: &Path,
    body: Option<Value>,
) -> Result<Value> {
    let body = body.unwrap_or(Value::Null);
    let skill_id = body
        .get("id")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .ok_or_else(|| anyhow!("community skill id is required"))?;
    let agent_id = body
        .get("agent_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("agent_id is required"))?
        .to_string();

    let (name, repo_full_name, repo_url, description, file_url): (
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    ) = {
        let conn = open_conn(db_path)?;
        conn.query_row(
            "SELECT name, repo_full_name, repo_url, description, file_url FROM community_skills WHERE id = ?1",
            [skill_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?
        .ok_or_else(|| anyhow!("community skill not found"))?
    };

    let raw_url = community_skill_raw_url(&repo_full_name, file_url.as_deref())
        .ok_or_else(|| anyhow!("cannot resolve GitHub raw SKILL.md URL"))?;
    let skill_content = fetch_community_skill_markdown(&raw_url).await?;
    if skill_content.trim().is_empty() {
        return Err(anyhow!("downloaded SKILL.md is empty"));
    }

    let conn = open_conn(db_path)?;
    services::scan::sync_source_configs(&conn)?;
    let target = services::scan::list_sources(&conn)?
        .into_iter()
        .find(|source| source.agent_id == agent_id)
        .ok_or_else(|| anyhow!("agent not found: {}", agent_id))?;
    let install_dir = target
        .paths
        .get("skills_path")
        .and_then(Clone::clone)
        .map(std::path::PathBuf::from)
        .ok_or_else(|| anyhow!("{} has no skills_path configured", target.agent_name))?;

    fs::create_dir_all(&install_dir)?;
    let skill_dir = install_dir.join(sanitize_file_name(&name));
    fs::create_dir_all(&skill_dir)?;
    let skill_path = skill_dir.join("SKILL.md");
    fs::write(&skill_path, &skill_content)?;

    if let Some(info) =
        services::scan::parse_skill_file(&skill_path, "community", &target.agent_id, None)
    {
        services::scan::upsert_skills(&conn, &[info])?;
    }

    conn.execute(
        "UPDATE community_skills
         SET installed = 1, skill_md_content = ?2, fetched_at = ?3
         WHERE id = ?1",
        params![skill_id, skill_content, now_string()],
    )?;

    Ok(json!({
        "status": "installed",
        "id": skill_id,
        "name": name,
        "repo": repo_full_name,
        "repo_url": repo_url,
        "description": description,
        "source": "GitHub",
        "raw_url": raw_url,
        "path": path_to_string(&skill_path),
        "agent_id": target.agent_id,
        "agent_name": target.agent_name
    }))
}

pub(crate) fn community_skill_raw_url(
    repo_full_name: &str,
    file_url: Option<&str>,
) -> Option<String> {
    if let Some(file_url) = file_url {
        if let Some(rest) = file_url.strip_prefix("https://github.com/") {
            let parts = rest.split('/').collect::<Vec<_>>();
            if parts.len() >= 5 && parts[2] == "blob" {
                let repo = format!("{}/{}", parts[0], parts[1]);
                let branch = parts[3];
                let file_path = parts[4..].join("/");
                return Some(format!(
                    "https://raw.githubusercontent.com/{repo}/{branch}/{file_path}"
                ));
            }
        }
    }
    Some(format!(
        "https://raw.githubusercontent.com/{repo_full_name}/HEAD/SKILL.md"
    ))
}

pub(crate) async fn fetch_community_skill_markdown(raw_url: &str) -> Result<String> {
    let resp = reqwest::get(raw_url)
        .await
        .map_err(|err| anyhow!("failed to download SKILL.md: {}", err))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|err| anyhow!("failed to read SKILL.md body: {}", err))?;
    if status.is_success() {
        Ok(text)
    } else {
        Err(anyhow!(
            "failed to download community skill: HTTP {}. The repository may not have SKILL.md at root, or GitHub is unreachable.",
            status.as_u16()
        ))
    }
}

// ---- Session CRUD ----

pub(crate) fn list_sessions(conn: &Connection, query: &PageQuery) -> Result<Value> {
    let page = query.page.unwrap_or(1).max(1);
    let size = query.size.unwrap_or(50).clamp(1, 200);
    let total: i64 = conn.query_row(
        "SELECT COUNT(id) FROM sessions WHERE (?1 IS NULL OR project_name = ?1)",
        params![query.project],
        |row| row.get(0),
    )?;
    let mut stmt = conn.prepare(
        "SELECT id, session_id, project_name, entrypoint, version, started_at, message_count, cwd,
                agent_source, first_prompt, compressed_summary, jsonl_path, jsonl_size
         FROM sessions WHERE (?1 IS NULL OR project_name = ?1)
         ORDER BY started_at DESC LIMIT ?2 OFFSET ?3",
    )?;
    let items = stmt
        .query_map(
            params![query.project, size, (page - 1) * size],
            |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "session_id": row.get::<_, String>(1)?,
                    "project_name": row.get::<_, Option<String>>(2)?,
                    "entrypoint": row.get::<_, Option<String>>(3)?,
                    "version": row.get::<_, Option<String>>(4)?,
                    "started_at": row.get::<_, Option<String>>(5)?,
                    "message_count": row.get::<_, i64>(6)?,
                    "cwd": row.get::<_, Option<String>>(7)?,
                    "agent_source": row.get::<_, Option<String>>(8)?,
                    "first_prompt": row.get::<_, Option<String>>(9)?,
                    "compressed_summary": row.get::<_, Option<String>>(10)?,
                    "jsonl_path": row.get::<_, Option<String>>(11)?,
                    "jsonl_size": row.get::<_, i64>(12)?,
                }))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "items": items, "total": total, "page": page, "size": size }))
}

pub(crate) fn get_session(conn: &Connection, session_id: &str) -> Result<Value> {
    conn.query_row(
        "SELECT id, session_id, pid, cwd, project_name, agent_source, entrypoint, version, kind,
                started_at, message_count, first_prompt, compressed_summary, jsonl_path, jsonl_size
         FROM sessions WHERE session_id = ?1",
        [session_id],
        |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "session_id": row.get::<_, String>(1)?,
                "pid": row.get::<_, Option<i64>>(2)?,
                "cwd": row.get::<_, Option<String>>(3)?,
                "project_name": row.get::<_, Option<String>>(4)?,
                "agent_source": row.get::<_, Option<String>>(5)?,
                "entrypoint": row.get::<_, Option<String>>(6)?,
                "version": row.get::<_, Option<String>>(7)?,
                "kind": row.get::<_, Option<String>>(8)?,
                "started_at": row.get::<_, Option<String>>(9)?,
                "message_count": row.get::<_, i64>(10)?,
                "first_prompt": row.get::<_, Option<String>>(11)?,
                "compressed_summary": row.get::<_, Option<String>>(12)?,
                "jsonl_path": row.get::<_, Option<String>>(13)?,
                "jsonl_size": row.get::<_, i64>(14)?,
                "skill_usage": [],
            }))
        },
    )
    .optional()?
    .ok_or_else(|| anyhow!("Session not found"))
}

// ---- Stats ----

pub(crate) fn stats_summary(conn: &Connection) -> Result<Value> {
    Ok(json!({
        "total_skills": count_table(conn, "skills")?,
        "total_agents": count_table(conn, "agents")?,
        "total_sessions": count_table(conn, "sessions")?,
        "total_memories": count_table(conn, "memories")?,
        "total_skill_usages": count_table(conn, "skill_usage")?,
        "enabled_sources": count_where(conn, "source_configs", "is_enabled = 1")?,
        "available_sources": count_where(conn, "source_configs", "is_available = 1")?,
    }))
}

pub(crate) fn top_skills(conn: &Connection, limit: i64) -> Result<Value> {
    let mut stmt = conn.prepare(
        "SELECT name, category, agent_source, source_type, description, usage_count, session_count
         FROM skills ORDER BY usage_count DESC, name ASC LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit.clamp(1, 100)], |row| {
            Ok(json!({
                "name": row.get::<_, String>(0)?,
                "category": row.get::<_, Option<String>>(1)?,
                "agent_source": row.get::<_, Option<String>>(2)?,
                "source_type": row.get::<_, String>(3)?,
                "description": row.get::<_, Option<String>>(4)?,
                "usage_count": row.get::<_, i64>(5)?,
                "session_count": row.get::<_, i64>(6)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!(items))
}

pub(crate) fn skills_by_category(conn: &Connection) -> Result<Value> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(category, 'other') AS category, COUNT(*) AS cnt
         FROM skills GROUP BY category ORDER BY cnt DESC",
    )?;
    let items = stmt
        .query_map([], |row| {
            Ok(json!({
                "category": row.get::<_, String>(0)?,
                "count": row.get::<_, i64>(1)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!(items))
}

// ---- Scan history ----

pub(crate) fn list_scan_history(conn: &Connection, query: &PageQuery) -> Result<Value> {
    let page = query.page.unwrap_or(1).max(1);
    let size = query.size.unwrap_or(20).clamp(1, 100);
    let total: i64 = conn.query_row("SELECT COUNT(id) FROM scan_jobs", [], |row| row.get(0))?;
    let mut stmt = conn.prepare(
        "SELECT id, scan_type, status, started_at, completed_at, skills_found, agents_found,
                sessions_found, conversations_analyzed, memories_found, sources_scanned, errors
         FROM scan_jobs ORDER BY started_at DESC LIMIT ?1 OFFSET ?2",
    )?;
    let items = stmt
        .query_map(params![size, (page - 1) * size], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "scan_type": row.get::<_, String>(1)?,
                "status": row.get::<_, String>(2)?,
                "started_at": row.get::<_, Option<String>>(3)?,
                "completed_at": row.get::<_, Option<String>>(4)?,
                "skills_found": row.get::<_, i64>(5)?,
                "agents_found": row.get::<_, i64>(6)?,
                "sessions_found": row.get::<_, i64>(7)?,
                "conversations_analyzed": row.get::<_, i64>(8)?,
                "memories_found": row.get::<_, i64>(9)?,
                "sources_scanned": row.get::<_, i64>(10)?,
                "errors": row.get::<_, Option<String>>(11)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "items": items, "total": total, "page": page, "size": size }))
}
