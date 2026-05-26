use crate::{now_string, sanitize_file_name, truncate_chars, yaml_scalar};
use anyhow::{anyhow, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use super::scan;

pub(crate) fn list_skills(conn: &Connection, query: &crate::PageQuery) -> Result<Value> {
    let page = query.page.unwrap_or(1).max(1);
    let size = query.size.unwrap_or(50).clamp(1, 200);
    let mut sql = String::from(
        "FROM skills WHERE (?1 IS NULL OR category = ?1)
         AND (?2 IS NULL OR source_type = ?2)
         AND (?3 IS NULL OR agent_source = ?3)
         AND (?4 IS NULL OR name LIKE ?4 OR description LIKE ?4)",
    );
    let search = query.search.as_ref().map(|s| format!("%{s}%"));
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(id) {sql}"),
        params![query.category, query.source, query.agent_source, search],
        |row| row.get(0),
    )?;
    sql.push_str(" ORDER BY usage_count DESC, name ASC LIMIT ?5 OFFSET ?6");
    let mut stmt = conn.prepare(&format!(
        "SELECT id, name, description, category, source_type, plugin_name, origin,
                usage_count, session_count, body_size, line_count, file_mtime {sql}"
    ))?;
    let items = stmt
        .query_map(
            params![
                query.category,
                query.source,
                query.agent_source,
                search,
                size,
                (page - 1) * size
            ],
            |row| {
                let description: Option<String> = row.get(2)?;
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "description": description.map(|d| truncate_chars(&d, 200)),
                    "category": row.get::<_, Option<String>>(3)?,
                    "source_type": row.get::<_, String>(4)?,
                    "plugin_name": row.get::<_, Option<String>>(5)?,
                    "origin": row.get::<_, Option<String>>(6)?,
                    "usage_count": row.get::<_, i64>(7)?,
                    "session_count": row.get::<_, i64>(8)?,
                    "body_size": row.get::<_, i64>(9)?,
                    "line_count": row.get::<_, i64>(10)?,
                    "file_mtime": row.get::<_, Option<f64>>(11)?,
                }))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "items": items, "total": total, "page": page, "size": size }))
}

pub(crate) fn list_categories(conn: &Connection) -> Result<Value> {
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

pub(crate) fn get_skill(conn: &Connection, name: &str, source_type: Option<&str>) -> Result<Value> {
    let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(st) =
        source_type
    {
        (
            "SELECT id, name, description, category, category_tags, source_type, plugin_name, origin,
                    file_path, yaml_raw, body_text, body_size, line_count, file_mtime, file_size,
                    usage_count, session_count, created_at, updated_at
             FROM skills WHERE name = ?1 AND source_type = ?2 LIMIT 1"
                .to_string(),
            vec![Box::new(name.to_string()), Box::new(st.to_string())],
        )
    } else {
        (
            "SELECT id, name, description, category, category_tags, source_type, plugin_name, origin,
                    file_path, yaml_raw, body_text, body_size, line_count, file_mtime, file_size,
                    usage_count, session_count, created_at, updated_at
             FROM skills WHERE name = ?1 LIMIT 1"
                .to_string(),
            vec![Box::new(name.to_string())],
        )
    };
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.query_row(&sql, param_refs.as_slice(), |row| {
            let category_tags: Option<String> = row.get(4)?;
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "description": row.get::<_, Option<String>>(2)?,
                "category": row.get::<_, Option<String>>(3)?,
                "category_tags": category_tags.and_then(|v| serde_json::from_str::<Value>(&v).ok()).unwrap_or(json!([])),
                "source_type": row.get::<_, String>(5)?,
                "plugin_name": row.get::<_, Option<String>>(6)?,
                "origin": row.get::<_, Option<String>>(7)?,
                "file_path": row.get::<_, String>(8)?,
                "yaml_raw": row.get::<_, Option<String>>(9)?,
                "body_text": row.get::<_, Option<String>>(10)?,
                "body_size": row.get::<_, i64>(11)?,
                "line_count": row.get::<_, i64>(12)?,
                "file_mtime": row.get::<_, Option<f64>>(13)?,
                "file_size": row.get::<_, i64>(14)?,
                "usage_count": row.get::<_, i64>(15)?,
                "session_count": row.get::<_, i64>(16)?,
                "created_at": row.get::<_, Option<String>>(17)?,
                "updated_at": row.get::<_, Option<String>>(18)?,
            }))
        },
    )
    .optional()?
    .ok_or_else(|| anyhow!("record not found"))
}

pub(crate) fn update_skill(conn: &Connection, name: &str, body: &Value) -> Result<Value> {
    let existing = conn
        .query_row(
            "SELECT file_path, source_type FROM skills WHERE name = ?1 LIMIT 1",
            [name],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or_else(|| anyhow!("Skill not found"))?;

    let new_name = body.get("name").and_then(|v| v.as_str()).unwrap_or(name);
    let new_desc = body.get("description").and_then(|v| v.as_str());
    let new_category = body.get("category").and_then(|v| v.as_str());

    conn.execute(
        "UPDATE skills SET name = ?1, description = ?2, category = ?3, updated_at = CURRENT_TIMESTAMP WHERE name = ?4",
        params![new_name, new_desc, new_category, name],
    )?;

    if new_name != name {
        let old_path = std::path::Path::new(&existing.0);
        if let Some(parent) = old_path.parent() {
            let ext = old_path.extension().unwrap_or_default();
            let new_path = parent.join(format!("{}.{}", new_name, ext.to_string_lossy()));
            if old_path.exists() && !new_path.exists() {
                std::fs::rename(old_path, &new_path)?;
                conn.execute(
                    "UPDATE skills SET file_path = ?1 WHERE name = ?2",
                    params![new_path.to_string_lossy().to_string(), new_name],
                )?;
            }
        }
    }

    get_skill(conn, new_name, None)
}

pub(crate) fn delete_skill(conn: &Connection, name: &str) -> Result<Value> {
    let row = conn
        .query_row(
            "SELECT file_path, source_type FROM skills WHERE name = ?1 LIMIT 1",
            [name],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or_else(|| anyhow!("Skill not found"))?;

    let file_path = std::path::Path::new(&row.0);
    if file_path.exists() {
        if let Ok(metadata) = std::fs::metadata(file_path) {
            let mut perms = metadata.permissions();
            if perms.readonly() {
                #[allow(clippy::permissions_set_readonly_false)]
                perms.set_readonly(false);
                std::fs::set_permissions(file_path, perms)
                    .with_context(|| format!("无法修改文件权限: {}", file_path.display()))?;
            }
        }
        std::fs::remove_file(file_path)
            .with_context(|| format!("无法删除文件 {}，可能被其他进程占用", file_path.display()))?;
    }

    if let Some(parent) = file_path.parent() {
        let _ = std::fs::remove_dir(parent);
    }

    conn.execute("DELETE FROM skills WHERE name = ?1", [name])?;
    Ok(json!({ "deleted": true, "name": name, "file_path": row.0 }))
}

pub(crate) fn create_manual_skill_draft(conn: &Connection, skill_name: &str) -> Result<Value> {
    let (name, description, category, agent_source, file_path, body_text): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT name, description, category, agent_source, file_path, body_text
             FROM skills WHERE name = ?1 LIMIT 1",
            [skill_name],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| anyhow!("Skill not found: {}", skill_name))?;

    let draft_name = format!("{}-manual-evolution", sanitize_file_name(&name));
    let description_text = description
        .clone()
        .unwrap_or_else(|| format!("Manual evolution draft based on existing Skill {}", name));
    let draft_body = format!(
        "---\nname: {}\ndescription: {}\ncategory: {}\norigin: manual-existing-skill\nsource_type: evolved-draft\n---\n\n# {}\n\n## Evolution Goal\nThis draft was created from an existing Skill. Edit this section to describe the improvements, split, merge, or restructuring you want.\n\n## Original Description\n{}\n\n## Original Content\n\n{}",
        yaml_scalar(&draft_name),
        yaml_scalar(&description_text),
        yaml_scalar(category.as_deref().unwrap_or("other")),
        draft_name,
        description.clone().unwrap_or_else(|| "No description".to_string()),
        body_text.clone().unwrap_or_default()
    );
    let now = now_string();
    let sample_tasks = serde_json::to_string(&vec![format!(
        "Manual evolution from existing Skill: {}",
        name
    )])?;
    let source_agents = serde_json::to_string(
        &agent_source
            .clone()
            .map(|value| vec![value])
            .unwrap_or_default(),
    )?;
    let source_skills = serde_json::to_string(&vec![json!({
        "name": name,
        "file_path": file_path,
        "source": "existing-manual"
    })])?;

    conn.execute(
        "INSERT INTO workflow_clusters
         (name, description, frequency, source_agents, estimated_time_saved,
          can_generate_skill, skill_score, status, draft_body, sample_tasks,
          recommendation_source, confidence, reasoning, source_skills, created_at, updated_at)
         VALUES (?1, ?2, 1, ?3, ?4, 1, 75, 'manual-draft', ?5, ?6,
                 'manual-existing-skill', 0.75, ?7, ?8, ?9, ?9)",
        params![
            draft_name,
            description_text,
            source_agents,
            "Manual improvement may reduce repeated maintenance work",
            draft_body,
            sample_tasks,
            format!("Created manually from existing Skill '{}'.", name),
            source_skills,
            now,
        ],
    )?;
    let id = conn.last_insert_rowid();
    super::workflow_crud::get_workflow_by_id(conn, id)
}

// ---- Workflow CRUD ----

pub(crate) fn list_workflows(conn: &Connection, query: &crate::PageQuery) -> Result<Value> {
    let page = query.page.unwrap_or(1).max(1);
    let size = query.size.unwrap_or(50).clamp(1, 200);
    let total: i64 = conn.query_row("SELECT COUNT(id) FROM workflow_clusters", [], |row| {
        row.get(0)
    })?;
    let mut stmt = conn.prepare(
        "SELECT id, name, description, frequency, source_agents, estimated_time_saved,
                can_generate_skill, skill_score, status, draft_body, sample_tasks,
                recommendation_source, confidence, reasoning, source_skills, similar_skills,
                review_score, review_summary, review_feedback, installed_agent_id
         FROM workflow_clusters ORDER BY skill_score DESC LIMIT ?1 OFFSET ?2",
    )?;
    let items: Vec<Value> = stmt
        .query_map(params![size, (page - 1) * size], |row| {
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
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "items": items, "total": total, "page": page, "size": size }))
}

pub(crate) fn list_skill_install_targets(conn: &Connection) -> Result<Value> {
    let mut targets = Vec::new();
    for source in scan::list_sources(conn)? {
        if let Some(Some(skills_path)) = source.paths.get("skills_path") {
            targets.push(json!({
                "agent_id": source.agent_id,
                "agent_name": source.agent_name,
                "skills_path": skills_path,
                "is_enabled": source.is_enabled,
                "is_available": source.is_available,
            }));
        }
    }
    Ok(json!(targets))
}

pub(crate) fn list_skill_variants(conn: &Connection, query: &crate::PageQuery) -> Result<Value> {
    let page = query.page.unwrap_or(1).max(1);
    let size = query.size.unwrap_or(50).clamp(1, 200);
    let total: i64 =
        conn.query_row("SELECT COUNT(id) FROM skill_variants", [], |row| row.get(0))?;
    let mut stmt = conn.prepare(
        "SELECT id, skill_name, variant_label, status, usage_count, avg_session_messages,
                performance_score, created_at, updated_at
         FROM skill_variants ORDER BY performance_score DESC LIMIT ?1 OFFSET ?2",
    )?;
    let items: Vec<Value> = stmt
        .query_map(params![size, (page - 1) * size], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "skill_name": row.get::<_, String>(1)?,
                "variant_label": row.get::<_, String>(2)?,
                "status": row.get::<_, String>(3)?,
                "usage_count": row.get::<_, i64>(4)?,
                "avg_session_messages": row.get::<_, f64>(5)?,
                "performance_score": row.get::<_, f64>(6)?,
                "created_at": row.get::<_, Option<String>>(7)?,
                "updated_at": row.get::<_, Option<String>>(8)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(json!({ "items": items, "total": total, "page": page, "size": size }))
}
