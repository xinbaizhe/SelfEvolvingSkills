use crate::{path_to_string, sanitize_file_name};
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::{fs, path::PathBuf};

use super::scan::{self, parse_skill_file, upsert_skills};

pub(crate) fn get_workflow_by_id(conn: &Connection, id: i64) -> Result<Value> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, frequency, source_agents, estimated_time_saved,
                can_generate_skill, skill_score, status, draft_body, sample_tasks,
                recommendation_source, confidence, reasoning, source_skills, similar_skills,
                review_score, review_summary, review_feedback, installed_agent_id
         FROM workflow_clusters WHERE id = ?1",
    )?;
    stmt.query_row([id], |row| {
        let source_agents: Option<String> = row.get(4)?;
        let sample_tasks: Option<String> = row.get(10)?;
        let source_skills: Option<String> = row.get(14)?;
        let similar_skills: Option<String> = row.get(15)?;
        let review_feedback: Option<String> = row.get(18)?;
        Ok(json!({
            "id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "description": row.get::<_, Option<String>>(2)?,
            "frequency": row.get::<_, i64>(3)?,
            "source_agents": source_agents.and_then(|v| serde_json::from_str::<Value>(&v).ok()).unwrap_or(json!([])),
            "estimated_time_saved": row.get::<_, Option<String>>(5)?,
            "can_generate_skill": row.get::<_, i64>(6)? != 0,
            "skill_score": row.get::<_, i64>(7)?,
            "status": row.get::<_, Option<String>>(8)?,
            "draft_body": row.get::<_, Option<String>>(9)?,
            "sample_tasks": sample_tasks.and_then(|v| serde_json::from_str::<Value>(&v).ok()).unwrap_or(json!([])),
            "recommendation_source": row.get::<_, Option<String>>(11)?,
            "confidence": row.get::<_, Option<f64>>(12)?,
            "reasoning": row.get::<_, Option<String>>(13)?,
            "source_skills": source_skills.and_then(|v| serde_json::from_str::<Value>(&v).ok()).unwrap_or(json!([])),
            "similar_skills": similar_skills.and_then(|v| serde_json::from_str::<Value>(&v).ok()).unwrap_or(json!([])),
            "review_score": row.get::<_, Option<i64>>(16)?,
            "review_summary": row.get::<_, Option<String>>(17)?,
            "review_feedback": review_feedback.and_then(|v| serde_json::from_str::<Value>(&v).ok()).unwrap_or(json!(null)),
            "installed_agent_id": row.get::<_, Option<String>>(19)?,
        }))
    })
    .map_err(|e| anyhow!("workflow draft not found: {}", e))
}

pub(crate) fn update_workflow_draft(
    conn: &Connection,
    id: i64,
    body: Option<Value>,
) -> Result<Value> {
    let body = body.unwrap_or(Value::Null);
    let draft_body = body
        .get("draft_body")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("draft_body is required"))?;
    let description = body.get("description").and_then(Value::as_str);
    conn.execute(
        "UPDATE workflow_clusters
         SET draft_body = ?2,
             description = COALESCE(?3, description),
             status = CASE WHEN status = 'installed' THEN status ELSE 'edited' END,
             updated_at = ?4
         WHERE id = ?1",
        params![id, draft_body, description, crate::now_string()],
    )?;
    get_workflow_by_id(conn, id)
}

pub(crate) fn delete_workflow_with_uninstall(
    conn: &Connection,
    id: i64,
    agent_id: Option<&str>,
) -> Result<()> {
    let row = conn
        .query_row(
            "SELECT name, status FROM workflow_clusters WHERE id = ?1",
            [id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()?
        .ok_or_else(|| anyhow!("workflow draft not found: {}", id))?;

    let (name, status) = row;

    if status.as_deref() == Some("installed") {
        if let Some(aid) = agent_id.filter(|v| !v.trim().is_empty()) {
            let sources = scan::list_sources(conn)?;
            if let Some(target) = sources.iter().find(|s| s.agent_id == aid) {
                if let Some(skills_path) = target.paths.get("skills_path").and_then(Clone::clone) {
                    let skill_dir = PathBuf::from(&skills_path).join(sanitize_file_name(&name));
                    if skill_dir.exists() {
                        fs::remove_dir_all(&skill_dir)?;
                    }
                    let skill_path_str = skill_dir.join("SKILL.md").to_string_lossy().to_string();
                    conn.execute("DELETE FROM skills WHERE file_path = ?1", [&skill_path_str])?;
                }
            }
        }
    }

    conn.execute("DELETE FROM workflow_clusters WHERE id = ?1", [id])?;
    Ok(())
}

pub(crate) fn install_workflow_skill(
    conn: &Connection,
    workflow_id: i64,
    body: Option<Value>,
) -> Result<Value> {
    let agent_id = body
        .as_ref()
        .and_then(|value| value.get("agent_id"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("agent_id is required"))?;

    let (name, draft_body): (String, String) = conn
        .query_row(
            "SELECT name, draft_body FROM workflow_clusters WHERE id = ?1",
            [workflow_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                ))
            },
        )
        .optional()?
        .ok_or_else(|| anyhow!("workflow draft not found"))?;

    if draft_body.trim().is_empty() {
        return Err(anyhow!("workflow draft has no installable Skill content"));
    }

    let target = scan::list_sources(conn)?
        .into_iter()
        .find(|source| source.agent_id == agent_id)
        .ok_or_else(|| anyhow!("agent not found: {}", agent_id))?;
    let install_dir = target
        .paths
        .get("skills_path")
        .and_then(Clone::clone)
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("{} has no skills_path configured", target.agent_name))?;

    fs::create_dir_all(&install_dir)?;
    let skill_dir = install_dir.join(sanitize_file_name(&name));
    fs::create_dir_all(&skill_dir)?;
    let skill_path = skill_dir.join("SKILL.md");
    fs::write(&skill_path, draft_body)?;

    conn.execute(
        "UPDATE workflow_clusters SET status = 'installed', installed_agent_id = ?2, updated_at = ?3 WHERE id = ?1",
        params![workflow_id, agent_id, crate::now_string()],
    )?;

    if let Some(info) = parse_skill_file(&skill_path, "generated", &target.agent_id, None) {
        upsert_skills(conn, &[info])?;
    }

    Ok(json!({
        "path": path_to_string(&skill_path),
        "agent_id": target.agent_id,
        "agent_name": target.agent_name
    }))
}
