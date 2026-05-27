use rusqlite::params;
use serde_json::Value;
use std::path::Path;

use crate::db;

/// Replace all cached team skills with fresh data from server.
pub(crate) fn cache_team_skills(db_path: &Path, skills: &[Value]) -> Result<usize, String> {
    let conn = db::open_conn(db_path).map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM team_skills_cache", [])
        .map_err(|e| e.to_string())?;

    let mut count = 0;
    for skill in skills {
        let id = skill["id"].as_i64().unwrap_or(0);
        conn.execute(
            "INSERT OR REPLACE INTO team_skills_cache
             (id, name, description, category, source_type, origin_agent, body_md,
              author_id, dept_id, compatible_models, compatible_agents,
              usage_count, avg_score, status, version,
              server_created_at, server_updated_at, cached_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17, CURRENT_TIMESTAMP)",
            params![
                id,
                skill["name"].as_str().unwrap_or(""),
                skill["description"].as_str().unwrap_or(""),
                skill["category"].as_str().unwrap_or(""),
                skill["sourceType"].as_str().unwrap_or(""),
                skill["originAgent"].as_str().unwrap_or(""),
                skill["bodyMd"].as_str().unwrap_or(""),
                skill["authorId"].as_i64().unwrap_or(0),
                skill["deptId"].as_i64().unwrap_or(0),
                skill["compatibleModels"].as_str().unwrap_or(""),
                skill["compatibleAgents"].as_str().unwrap_or(""),
                skill["usageCount"].as_i64().unwrap_or(0),
                skill["avgScore"].as_f64().unwrap_or(0.0),
                skill["status"].as_str().unwrap_or("published"),
                skill["version"].as_i64().unwrap_or(1),
                skill["createdAt"].as_str().unwrap_or(""),
                skill["updatedAt"].as_str().unwrap_or(""),
            ],
        )
        .map_err(|e| e.to_string())?;
        count += 1;
    }
    Ok(count)
}

/// Read cached team skills, returning JSON array + last cached timestamp.
pub(crate) fn get_cached_skills(db_path: &Path) -> Result<Value, String> {
    let conn = db::open_conn(db_path).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, description, category, source_type, origin_agent, body_md,
                    author_id, dept_id, compatible_models, compatible_agents,
                    usage_count, avg_score, status, version,
                    server_created_at, server_updated_at, cached_at
             FROM team_skills_cache
             ORDER BY cached_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "description": row.get::<_, Option<String>>(2)?,
                "category": row.get::<_, Option<String>>(3)?,
                "sourceType": row.get::<_, Option<String>>(4)?,
                "originAgent": row.get::<_, Option<String>>(5)?,
                "bodyMd": row.get::<_, Option<String>>(6)?,
                "authorId": row.get::<_, i64>(7)?,
                "deptId": row.get::<_, i64>(8)?,
                "compatibleModels": row.get::<_, Option<String>>(9)?,
                "compatibleAgents": row.get::<_, Option<String>>(10)?,
                "usageCount": row.get::<_, i64>(11)?,
                "avgScore": row.get::<_, f64>(12)?,
                "status": row.get::<_, Option<String>>(13)?,
                "version": row.get::<_, i64>(14)?,
                "createdAt": row.get::<_, Option<String>>(15)?,
                "updatedAt": row.get::<_, Option<String>>(16)?,
                "cachedAt": row.get::<_, String>(17)?
            }))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let cached_at: String = conn
        .query_row(
            "SELECT COALESCE(MAX(cached_at), '') FROM team_skills_cache",
            [],
            |row| row.get(0),
        )
        .unwrap_or_default();

    Ok(serde_json::json!({
        "skills": rows,
        "cached_at": cached_at,
        "count": rows.len()
    }))
}

/// Get cache summary (count + last update time), returns null if empty.
pub(crate) fn get_cache_summary(db_path: &Path) -> Result<Value, String> {
    let conn = db::open_conn(db_path).map_err(|e| e.to_string())?;
    let (count, cached_at): (i64, Option<String>) = conn
        .query_row(
            "SELECT COUNT(*), MAX(cached_at) FROM team_skills_cache",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "has_cache": count > 0,
        "count": count,
        "cached_at": cached_at,
    }))
}

// ---- Pending operations queue ----

/// Add an operation to the offline queue.
pub(crate) fn queue_operation(
    db_path: &Path,
    op_type: &str,
    path: &str,
    method: &str,
    body: Option<&Value>,
) -> Result<i64, String> {
    let conn = db::open_conn(db_path).map_err(|e| e.to_string())?;
    let body_str = body.map(|b| b.to_string());
    conn.execute(
        "INSERT INTO pending_operations (op_type, path, method, body) VALUES (?1, ?2, ?3, ?4)",
        params![op_type, path, method, body_str],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// Get all pending operations.
pub(crate) fn get_pending_operations(db_path: &Path) -> Result<Value, String> {
    let conn = db::open_conn(db_path).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, op_type, path, method, body, status, error_message, retry_count, created_at
             FROM pending_operations
             WHERE status = 'pending'
             ORDER BY created_at ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "opType": row.get::<_, String>(1)?,
                "path": row.get::<_, String>(2)?,
                "method": row.get::<_, String>(3)?,
                "body": row.get::<_, Option<String>>(4)?,
                "status": row.get::<_, String>(5)?,
                "errorMessage": row.get::<_, Option<String>>(6)?,
                "retryCount": row.get::<_, i64>(7)?,
                "createdAt": row.get::<_, String>(8)?
            }))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "ops": rows, "count": rows.len() }))
}

/// Update a pending operation's status.
pub(crate) fn update_operation_status(
    db_path: &Path,
    id: i64,
    status: &str,
    error_message: Option<&str>,
) -> Result<(), String> {
    let conn = db::open_conn(db_path).map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE pending_operations SET status = ?1, error_message = ?2 WHERE id = ?3",
        params![status, error_message, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Increment retry count for an operation.
pub(crate) fn increment_retry_count(db_path: &Path, id: i64) -> Result<(), String> {
    let conn = db::open_conn(db_path).map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE pending_operations SET retry_count = retry_count + 1 WHERE id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Remove all succeeded operations older than the given number.
pub(crate) fn cleanup_completed_ops(db_path: &Path, keep_last: i64) -> Result<usize, String> {
    let conn = db::open_conn(db_path).map_err(|e| e.to_string())?;
    let deleted = conn
        .execute(
            "DELETE FROM pending_operations WHERE status = 'synced' AND id NOT IN (
                SELECT id FROM pending_operations WHERE status = 'synced'
                ORDER BY created_at DESC LIMIT ?1
            )",
            params![keep_last],
        )
        .map_err(|e| e.to_string())?;
    Ok(deleted)
}
