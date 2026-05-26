use crate::{hash_bytes, now_string};
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use super::scan;

pub(crate) fn get_config(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_config WHERE key = ?1",
        [key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(Into::into)
}

pub(crate) fn get_config_bool(conn: &Connection, key: &str, default: bool) -> Result<bool> {
    Ok(get_config(conn, key)?
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(default))
}

fn set_config(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO app_config (key, value, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, now_string()],
    )?;
    Ok(())
}

pub(crate) fn save_llm_config(conn: &Connection, body: Option<Value>) -> Result<Value> {
    let body = body.unwrap_or(Value::Null);
    let enabled = body
        .get("llm_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let provider = body
        .get("llm_provider")
        .and_then(Value::as_str)
        .unwrap_or("openai")
        .trim();
    let base_url = body
        .get("llm_base_url")
        .and_then(Value::as_str)
        .unwrap_or("https://api.openai.com/v1")
        .trim();
    let model = body
        .get("llm_model")
        .and_then(Value::as_str)
        .unwrap_or("gpt-5.5")
        .trim();

    if base_url.is_empty() {
        return Err(anyhow!("base_url is required"));
    }
    if model.is_empty() {
        return Err(anyhow!("model is required"));
    }

    let api_format = body
        .get("llm_api_format")
        .and_then(Value::as_str)
        .unwrap_or("openai")
        .trim();

    set_config(conn, "llm_enabled", &enabled.to_string())?;
    set_config(conn, "llm_provider", provider)?;
    set_config(conn, "llm_base_url", base_url)?;
    set_config(conn, "llm_model", model)?;
    set_config(conn, "llm_api_format", api_format)?;

    if let Some(api_key) = body
        .get("llm_api_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        set_config(conn, "llm_api_key", api_key)?;
    }

    Ok(json!({
        "enabled": enabled,
        "provider": provider,
        "base_url": base_url,
        "model": model,
        "has_api_key": get_config(conn, "llm_api_key")?.is_some()
    }))
}

pub(crate) async fn test_llm_connection(body: Option<Value>) -> Result<Value> {
    let body = body.unwrap_or(Value::Null);
    let base_url = body
        .get("llm_base_url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .trim_end_matches('/')
        .to_string();
    let api_key = body
        .get("llm_api_key")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let model = body
        .get("llm_model")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let api_format = body
        .get("llm_api_format")
        .and_then(Value::as_str)
        .unwrap_or("openai")
        .trim()
        .to_string();

    if base_url.is_empty() {
        return Err(anyhow!("base_url is required"));
    }
    if model.is_empty() {
        return Err(anyhow!("model is required"));
    }
    if api_key.is_empty() {
        return Err(anyhow!("api_key is required"));
    }

    let is_anthropic = api_format == "anthropic";
    let url = if is_anthropic {
        format!("{}/v1/messages", base_url)
    } else {
        format!("{}/chat/completions", base_url)
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;

    let mut req = client
        .post(&url)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json");
    if is_anthropic {
        req = req
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": model,
                "max_tokens": 1,
                "messages": [
                    { "role": "user", "content": "ping" }
                ],
                "temperature": 0
            }));
    } else {
        req = req.bearer_auth(&api_key).json(&json!({
            "model": model,
            "messages": [
                { "role": "user", "content": "ping" }
            ],
            "max_tokens": 1,
            "temperature": 0
        }));
    }

    let response = req.send().await?;

    let status = response.status();
    let text = response.text().await.unwrap_or_else(|e| {
        format!("[Failed to read response body: {e}]")
    });
    if status.is_success() {
        Ok(json!({ "message": "大模型连接测试成功" }))
    } else {
        let message = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|value| {
                if is_anthropic {
                    value
                        .get("error")
                        .and_then(|error| error.get("message"))
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                } else {
                    value
                        .get("error")
                        .and_then(|error| error.get("message").or(Some(error)))
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                }
            })
            .unwrap_or_else(|| text.chars().take(300).collect::<String>());
        Err(anyhow!(
            "大模型连接测试失败：HTTP {} {}",
            status.as_u16(),
            message
        ))
    }
}

pub(crate) fn system_info(conn: &Connection) -> Result<Value> {
    let total_skills: i64 = crate::count_table(conn, "skills")?;
    let total_agents: i64 = crate::count_table(conn, "agents")?;
    let total_sessions: i64 = crate::count_table(conn, "sessions")?;
    let first_launch = total_skills == 0 && total_agents == 0 && total_sessions == 0;
    Ok(json!({
        "runtime": "tauri-rust",
        "database": "sqlite",
        "total_skills": total_skills,
        "total_agents": total_agents,
        "total_sessions": total_sessions,
        "first_launch": first_launch,
        "sources": scan::list_sources(conn)?,
    }))
}

pub(crate) fn list_admin_users(conn: &Connection) -> Result<Value> {
    let mut stmt = conn
        .prepare("SELECT id, username, is_active, created_at FROM admin_users ORDER BY id ASC")?;
    let items = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "username": row.get::<_, String>(1)?,
                "is_active": row.get::<_, i64>(2)? != 0,
                "created_at": row.get::<_, Option<String>>(3)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "items": items, "total": items.len() }))
}

pub(crate) fn create_admin_user(conn: &Connection, body: Option<Value>) -> Result<Value> {
    let body = body.unwrap_or(Value::Null);
    let username = body
        .get("username")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| anyhow!("username is required"))?;
    let password_hash = body
        .get("password")
        .and_then(Value::as_str)
        .map(|p| hash_bytes(p.as_bytes()))
        .unwrap_or_default();
    conn.execute(
        "INSERT INTO admin_users (username, password_hash, is_active, created_at)
         VALUES (?1, ?2, 1, ?3)",
        params![username, password_hash, now_string()],
    )?;
    Ok(json!({ "id": conn.last_insert_rowid(), "username": username, "is_active": true }))
}

pub(crate) fn update_admin_user(conn: &Connection, id: i64, body: Option<Value>) -> Result<Value> {
    let body = body.unwrap_or(Value::Null);
    if let Some(username) = body
        .get("username")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
    {
        conn.execute(
            "UPDATE admin_users SET username = ?2 WHERE id = ?1",
            params![id, username],
        )?;
    }
    if let Some(is_active) = body.get("is_active").and_then(Value::as_bool) {
        conn.execute(
            "UPDATE admin_users SET is_active = ?2 WHERE id = ?1",
            params![id, is_active as i64],
        )?;
    }
    if let Some(password) = body
        .get("password")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
    {
        conn.execute(
            "UPDATE admin_users SET password_hash = ?2 WHERE id = ?1",
            params![id, hash_bytes(password.as_bytes())],
        )?;
    }
    Ok(json!({ "id": id }))
}

pub(crate) fn delete_admin_user(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM admin_users WHERE id = ?1 AND id != 1", [id])?;
    Ok(())
}
