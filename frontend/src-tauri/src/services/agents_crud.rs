use crate::{now_string, sanitize_file_name, yaml_scalar};
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

pub(crate) fn list_agents(conn: &Connection, query: &crate::PageQuery) -> Result<Value> {
    let page = query.page.unwrap_or(1).max(1);
    let size = query.size.unwrap_or(50).clamp(1, 200);
    let search = query.search.as_ref().map(|s| format!("%{s}%"));
    let total: i64 = conn.query_row(
        "SELECT COUNT(id) FROM agents
         WHERE (?1 IS NULL OR agent_source = ?1)
           AND (?2 IS NULL OR name LIKE ?2 OR description LIKE ?2)",
        params![query.agent_source, search],
        |row| row.get(0),
    )?;
    let mut stmt = conn.prepare(
        "SELECT id, name, description, tools, model, file_path, body_size, line_count, file_mtime
         FROM agents
         WHERE (?1 IS NULL OR agent_source = ?1)
           AND (?2 IS NULL OR name LIKE ?2 OR description LIKE ?2)
         ORDER BY name ASC LIMIT ?3 OFFSET ?4",
    )?;
    let items = stmt
        .query_map(
            params![query.agent_source, search, size, (page - 1) * size],
            |row| {
                let tools: Option<String> = row.get(3)?;
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "description": row.get::<_, Option<String>>(2)?,
                    "tools": tools.and_then(|v| serde_json::from_str::<Value>(&v).ok()),
                    "model": row.get::<_, Option<String>>(4)?,
                    "file_path": row.get::<_, String>(5)?,
                    "body_size": row.get::<_, i64>(6)?,
                    "line_count": row.get::<_, i64>(7)?,
                    "file_mtime": row.get::<_, Option<f64>>(8)?,
                }))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "items": items, "total": total, "page": page, "size": size }))
}

pub(crate) fn get_agent(conn: &Connection, name: &str) -> Result<Value> {
    conn.query_row(
        "SELECT id, name, description, tools, model, file_path, yaml_raw, body_text,
                body_size, line_count, file_mtime, file_size
         FROM agents WHERE name = ?1 LIMIT 1",
        [name],
        |row| {
            let tools: Option<String> = row.get(3)?;
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "description": row.get::<_, Option<String>>(2)?,
                "tools": tools.and_then(|v| serde_json::from_str::<Value>(&v).ok()),
                "model": row.get::<_, Option<String>>(4)?,
                "file_path": row.get::<_, String>(5)?,
                "yaml_raw": row.get::<_, Option<String>>(6)?,
                "body_text": row.get::<_, Option<String>>(7)?,
                "body_size": row.get::<_, i64>(8)?,
                "line_count": row.get::<_, i64>(9)?,
                "file_mtime": row.get::<_, Option<f64>>(10)?,
                "file_size": row.get::<_, i64>(11)?,
            }))
        },
    )
    .optional()?
    .ok_or_else(|| anyhow!("Agent not found"))
}

pub(crate) fn update_agent(conn: &Connection, name: &str, body: &Value) -> Result<Value> {
    let description = body.get("description").and_then(Value::as_str);
    let model = body.get("model").and_then(Value::as_str);
    let new_name = body.get("name").and_then(Value::as_str);
    let tools = body.get("tools");

    let affected = if let Some(new_name) = new_name.filter(|v| !v.trim().is_empty() && *v != name) {
        conn.execute(
            "UPDATE agents SET name = ?2, description = COALESCE(?3, description),
             model = COALESCE(?4, model), tools = COALESCE(?5, tools)
             WHERE name = ?1",
            params![
                name,
                new_name,
                description,
                model,
                tools.map(|v| v.to_string()),
            ],
        )?
    } else {
        conn.execute(
            "UPDATE agents SET description = COALESCE(?2, description),
             model = COALESCE(?3, model), tools = COALESCE(?4, tools)
             WHERE name = ?1",
            params![name, description, model, tools.map(|v| v.to_string())],
        )?
    };

    if affected == 0 {
        return Err(anyhow!("Agent not found: {}", name));
    }

    let lookup_name = new_name
        .filter(|v| !v.trim().is_empty() && *v != name)
        .unwrap_or(name);
    get_agent(conn, lookup_name)
}

pub(crate) fn delete_agent(conn: &Connection, name: &str) -> Result<Value> {
    let file_path: Option<String> = conn
        .query_row(
            "SELECT file_path FROM agents WHERE name = ?1 LIMIT 1",
            [name],
            |row| row.get(0),
        )
        .optional()?
        .flatten();

    let affected = conn.execute("DELETE FROM agents WHERE name = ?1", [name])?;
    if affected == 0 {
        return Err(anyhow!("Agent not found: {}", name));
    }

    Ok(json!({ "deleted": true, "name": name, "file_path": file_path }))
}

pub(crate) fn create_agent_evolution_draft(conn: &Connection, name: &str) -> Result<Value> {
    let (description, model, agent_source, file_path, body_text): (
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT description, model, agent_source, file_path, body_text
             FROM agents WHERE name = ?1 LIMIT 1",
            [name],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| anyhow!("Agent not found: {}", name))?;

    let draft_name = format!("{}-agent-evolution", sanitize_file_name(name));
    let description_text =
        description.unwrap_or_else(|| format!("Evolution draft based on Agent {}", name));
    let model_text = model.as_deref().unwrap_or("未指定");
    let draft_body = format!(
        "---\nname: {}\ndescription: {}\ncategory: agent-evolution\norigin: agent-evolution\nsource_type: evolved-draft\n---\n\n# Agent Evolution: {}\n\n## Evolution Goal\nThis draft was created from an existing Agent definition. Edit to describe improvements to the agent's prompts, tools, or model configuration.\n\n## Original Agent Info\n- **Name**: {}\n- **Model**: {}\n- **Source**: {}\n- **File**: {}\n\n## Original Description\n{}\n\n## Original Definition\n\n{}",
        yaml_scalar(&draft_name),
        yaml_scalar(&description_text),
        name,
        name,
        model_text,
        agent_source.as_deref().unwrap_or("unknown"),
        file_path,
        description_text,
        body_text.clone().unwrap_or_default()
    );

    let now = now_string();
    let sample_tasks = serde_json::to_string(&vec![format!(
        "Agent evolution: improve definition of {}",
        name
    )])?;
    let source_agents =
        serde_json::to_string(&agent_source.clone().map(|v| vec![v]).unwrap_or_default())?;
    let source_skills = serde_json::to_string(&vec![json!({
        "name": name,
        "file_path": file_path,
        "source": "agent-evolution"
    })])?;

    conn.execute(
        "INSERT INTO workflow_clusters
         (name, description, frequency, source_agents, estimated_time_saved,
          can_generate_skill, skill_score, status, draft_body, sample_tasks,
          recommendation_source, confidence, reasoning, source_skills, created_at, updated_at)
         VALUES (?1, ?2, 1, ?3, ?4, 1, 65, 'manual-draft', ?5, ?6,
                 'agent-evolution', 0.65, ?7, ?8, ?9, ?9)",
        params![
            draft_name,
            description_text,
            source_agents,
            "Agent improvement may enhance workflow efficiency",
            draft_body,
            sample_tasks,
            format!("Created from Agent '{}'.", name),
            source_skills,
            now,
        ],
    )?;
    let id = conn.last_insert_rowid();
    super::workflow_crud::get_workflow_by_id(conn, id)
}
