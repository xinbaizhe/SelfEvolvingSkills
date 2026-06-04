use crate::{hash_bytes, now_string};
use anyhow::{anyhow, Result};
use base64::Engine;
use rusqlite::{params, Connection, OptionalExtension};
use super::llm_utils::{call_llm, LlmCallParams};
use serde_json::{json, Value};
use std::io::{Cursor, Read};
use std::path::PathBuf;
use walkdir::WalkDir;
use zip::ZipArchive;

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
        "api_format": api_format,
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
    let text = response
        .text()
        .await
        .unwrap_or_else(|e| format!("[Failed to read response body: {e}]"));
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

fn extract_json_object(text: &str) -> Result<String> {
    let mut stripped = text.trim().to_string();
    if stripped.starts_with("```") {
        let mut lines = stripped.lines().collect::<Vec<_>>();
        if lines
            .first()
            .is_some_and(|line| line.trim_start().starts_with("```"))
        {
            lines.remove(0);
        }
        if lines
            .last()
            .is_some_and(|line| line.trim_start().starts_with("```"))
        {
            lines.pop();
        }
        stripped = lines.join("\n").trim().to_string();
    }
    let start = stripped
        .find('{')
        .ok_or_else(|| anyhow!("大模型未返回 JSON 对象"))?;
    let end = stripped
        .rfind('}')
        .ok_or_else(|| anyhow!("大模型未返回完整 JSON 对象"))?;
    Ok(stripped[start..=end].to_string())
}

fn clamp_score(value: Option<i64>, default: i64) -> i64 {
    value.unwrap_or(default).clamp(0, 100)
}

fn extract_line_value(content: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    content.lines().find_map(|line| {
        line.trim()
            .strip_prefix(&prefix)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    })
}

fn is_auditable_zip_entry(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with("skill.md")
        || lower.ends_with(".py")
        || lower.ends_with(".md")
        || lower.ends_with(".json")
        || lower.ends_with(".toml")
        || lower.ends_with(".yaml")
        || lower.ends_with(".yml")
        || lower.ends_with(".txt")
}

fn scan_uploaded_zip(content: &str) -> Result<Option<Value>> {
    let Some(encoded) = extract_line_value(content, "ZIP_BASE64") else {
        return Ok(None);
    };
    let filename = extract_line_value(content, "ZIP_FILE").unwrap_or_else(|| "uploaded.zip".to_string());
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|err| anyhow!("zip base64 解码失败：{err}"))?;
    let mut archive = ZipArchive::new(Cursor::new(bytes))?;
    let mut entries = Vec::new();
    let mut scanned_files = 0usize;
    let mut skipped_files = 0usize;

    for index in 0..archive.len().min(120) {
        let mut file = archive.by_index(index)?;
        let name = file.name().replace('\\', "/");
        if file.is_dir() {
            continue;
        }
        if !is_auditable_zip_entry(&name) {
            skipped_files += 1;
            continue;
        }
        if file.size() > 512_000 {
            entries.push(json!({
                "path": name,
                "size": file.size(),
                "skipped": true,
                "reason": "文件过大，未读取正文"
            }));
            skipped_files += 1;
            continue;
        }
        let mut text = String::new();
        file.read_to_string(&mut text)?;
        let excerpt = text.chars().take(4_000).collect::<String>();
        entries.push(json!({
            "path": name,
            "size": file.size(),
            "lines": text.lines().count(),
            "excerpt": excerpt
        }));
        scanned_files += 1;
    }

    Ok(Some(json!({
        "filename": filename,
        "entryCount": archive.len(),
        "scannedFiles": scanned_files,
        "skippedFiles": skipped_files,
        "entries": entries
    })))
}

pub(crate) async fn evaluate_share_resource_with_llm(
    base_url: String,
    api_key: String,
    model: String,
    api_format: String,
    body: Option<Value>,
) -> Result<Value> {
    let body = body.ok_or_else(|| anyhow!("Missing request body"))?;
    let content = body
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let zip_scan = scan_uploaded_zip(&content)?;
    let content_preview = content
        .chars()
        .take(16_000)
        .collect::<String>();
    let metadata = body.get("metadata").cloned().unwrap_or_else(|| json!({}));
    let heuristic = body.get("heuristic").cloned().unwrap_or_else(|| json!({}));

    let system_prompt = r#"你是 AI Coding Agent Skill 的安全与性能评估专家。
你必须返回严格 JSON，不要 Markdown，不要解释 JSON 外的文本。
请从安全风险、性能成本、可维护性、可复用性、执行边界、敏感信息暴露、危险命令、依赖和网络访问风险等角度评分。
如果资源包含上传 zip，你必须优先审查 zip 内抽取出的 SKILL.md、Python 代码和配置文件，不要只根据 ZIP_BASE64 字符串评分。
分数必须严格，只有生产级、风险清晰、说明充分、性能成本可控的资源才可达到 95 分以上。"#;
    let user_prompt = format!(
        r#"请评估以下团队分享资源，并给出分数和修改建议。

元数据:
{}

规则预检结果:
{}

资源内容:
{}

上传 zip 扫描结果:
{}

返回 JSON 格式:
{{
  "score": 0-100,
  "securityScore": 0-100,
  "performanceScore": 0-100,
  "summary": "中文总评",
  "risks": ["主要风险1", "主要风险2"],
  "suggestions": ["具体修改建议1", "具体修改建议2"],
  "requiredChanges": ["未达到95分时必须修改的项"],
  "confidence": 0.0-1.0
}}"#,
        serde_json::to_string_pretty(&metadata)?,
        serde_json::to_string_pretty(&heuristic)?,
        content_preview,
        serde_json::to_string_pretty(&zip_scan.clone().unwrap_or_else(|| json!(null)))?
    );

    let raw = call_llm(LlmCallParams::new(base_url, api_key, model, api_format, system_prompt, &user_prompt).error_label("大模型评估请求失败"))
        .await?;
    let parsed: Value = serde_json::from_str(&extract_json_object(&raw)?)?;
    let llm_score = clamp_score(parsed.get("score").and_then(Value::as_i64), 0);
    let security_score = clamp_score(parsed.get("securityScore").and_then(Value::as_i64), llm_score);
    let performance_score =
        clamp_score(parsed.get("performanceScore").and_then(Value::as_i64), llm_score);

    Ok(json!({
        "score": llm_score,
        "securityScore": security_score,
        "performanceScore": performance_score,
        "summary": parsed.get("summary").and_then(Value::as_str).unwrap_or("大模型评估完成"),
        "risks": parsed.get("risks").and_then(Value::as_array).cloned().unwrap_or_default(),
        "suggestions": parsed.get("suggestions").and_then(Value::as_array).cloned().unwrap_or_default(),
        "requiredChanges": parsed.get("requiredChanges").and_then(Value::as_array).cloned().unwrap_or_default(),
        "confidence": parsed.get("confidence").and_then(Value::as_f64).unwrap_or(0.7).clamp(0.0, 1.0),
        "zipScan": zip_scan,
        "source": "llm"
    }))
}

fn scan_skill_directory(directory: &str) -> Result<Value> {
    let path = PathBuf::from(directory.trim());
    if directory.trim().is_empty() {
        return Err(anyhow!("评估目录不能为空"));
    }
    if !path.exists() {
        return Err(anyhow!("评估目录不存在：{}", directory));
    }
    if !path.is_dir() {
        return Err(anyhow!("评估目标不是目录：{}", directory));
    }

    let mut skills = Vec::new();
    let mut python_files = Vec::new();
    let mut scanned_files = 0usize;
    let mut total_bytes = 0u64;
    for entry in WalkDir::new(&path)
        .max_depth(6)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        scanned_files += 1;
        let target_path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();
        let extension = target_path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if file_name != "SKILL.md" && extension != "py" {
            continue;
        }

        let metadata = std::fs::metadata(target_path)?;
        total_bytes = total_bytes.saturating_add(metadata.len());
        let text = std::fs::read_to_string(target_path)
            .unwrap_or_else(|err| format!("[failed to read file: {err}]"));

        if file_name == "SKILL.md" {
            let first_heading = text
                .lines()
                .find(|line| line.trim_start().starts_with('#'))
                .map(|line| line.trim().trim_start_matches('#').trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| {
                    target_path
                        .parent()
                        .and_then(|parent| parent.file_name())
                        .and_then(|name| name.to_str())
                        .unwrap_or("Unnamed Skill")
                        .to_string()
                });
            let excerpt = text.chars().take(2400).collect::<String>();
            skills.push(json!({
                "name": first_heading,
                "path": target_path.to_string_lossy(),
                "bytes": metadata.len(),
                "lines": text.lines().count(),
                "excerpt": excerpt
            }));
        } else {
            let lower = text.to_lowercase();
            let risk_markers = [
                "subprocess",
                "os.system",
                "eval(",
                "exec(",
                "pickle.load",
                "yaml.load",
                "requests.",
                "httpx.",
                "open(",
                "shutil.rmtree",
                "socket.",
                "paramiko",
                "boto3",
                "sqlalchemy",
                "pymysql",
                "psycopg",
                "input(",
            ]
            .iter()
            .filter(|marker| lower.contains(**marker))
            .map(|marker| marker.to_string())
            .collect::<Vec<_>>();
            let imports = text
                .lines()
                .filter_map(|line| {
                    let trimmed = line.trim();
                    if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
                        Some(trimmed.chars().take(160).collect::<String>())
                    } else {
                        None
                    }
                })
                .take(30)
                .collect::<Vec<_>>();
            let excerpt = text
                .lines()
                .filter(|line| {
                    let lower_line = line.to_lowercase();
                    risk_markers.iter().any(|marker| lower_line.contains(marker))
                        || lower_line.starts_with("def ")
                        || lower_line.starts_with("class ")
                        || lower_line.contains("__main__")
                })
                .take(80)
                .collect::<Vec<_>>()
                .join("\n")
                .chars()
                .take(3000)
                .collect::<String>();
            python_files.push(json!({
                "path": target_path.to_string_lossy(),
                "bytes": metadata.len(),
                "lines": text.lines().count(),
                "imports": imports,
                "riskMarkers": risk_markers,
                "excerpt": excerpt
            }));
        }

        if skills.len() >= 40 && python_files.len() >= 80 {
            break;
        }
    }

    if skills.is_empty() && python_files.is_empty() {
        return Err(anyhow!("目录下未发现 SKILL.md 或 Python 代码，无法进行评估"));
    }

    Ok(json!({
        "directory": path.to_string_lossy(),
        "scannedFiles": scanned_files,
        "skillCount": skills.len(),
        "pythonFileCount": python_files.len(),
        "totalBytes": total_bytes,
        "skills": skills,
        "pythonFiles": python_files
    }))
}

pub(crate) async fn evaluate_directory_skills_with_llm(
    base_url: String,
    api_key: String,
    model: String,
    api_format: String,
    body: Option<Value>,
) -> Result<Value> {
    let body = body.ok_or_else(|| anyhow!("Missing request body"))?;
    let directory = body
        .get("directory")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Missing directory"))?;
    let metadata = body.get("metadata").cloned().unwrap_or_else(|| json!({}));
    let scan = scan_skill_directory(directory)?;

    let system_prompt = r#"你是一个多 Agent 评估委员会，包含安全审计 Agent、性能评估 Agent、Skill 质量 Agent、Python 代码审计 Agent、可维护性 Agent、平台兼容 Agent。
你必须同时审查 SKILL.md 和 Python 代码。Python 代码重点关注命令执行、反序列化、文件删除、网络请求、密钥泄露、依赖风险、异常处理、资源释放、性能热点和入口行为。
你必须返回严格 JSON，不要 Markdown，不要 JSON 外文本。评分必须严格，只有所有 Agent 都认为可发布时综合分才可达到 95 以上。"#;
    let user_prompt = format!(
        r#"请对目录扫描出的 Skills 和 Python 代码进行多 Agent 评估。

分享元数据:
{}

目录扫描结果:
{}

请返回 JSON:
{{
  "score": 0-100,
  "securityScore": 0-100,
  "performanceScore": 0-100,
  "summary": "中文总评",
  "risks": ["风险1"],
  "suggestions": ["修改建议1"],
  "requiredChanges": ["低于95时必须修改项"],
  "agents": [
    {{"agent": "安全审计 Agent", "score": 0-100, "verdict": "pass|warn|fail", "findings": ["发现"], "suggestions": ["建议"]}},
    {{"agent": "性能评估 Agent", "score": 0-100, "verdict": "pass|warn|fail", "findings": ["发现"], "suggestions": ["建议"]}},
    {{"agent": "Skill 质量 Agent", "score": 0-100, "verdict": "pass|warn|fail", "findings": ["发现"], "suggestions": ["建议"]}},
    {{"agent": "Python 代码审计 Agent", "score": 0-100, "verdict": "pass|warn|fail", "findings": ["发现"], "suggestions": ["建议"]}},
    {{"agent": "可维护性 Agent", "score": 0-100, "verdict": "pass|warn|fail", "findings": ["发现"], "suggestions": ["建议"]}},
    {{"agent": "平台兼容 Agent", "score": 0-100, "verdict": "pass|warn|fail", "findings": ["发现"], "suggestions": ["建议"]}}
  ],
  "skills": [
    {{"name": "Skill 名称", "score": 0-100, "risks": ["风险"], "suggestions": ["建议"]}}
  ],
  "pythonFiles": [
    {{"path": "文件路径", "score": 0-100, "risks": ["风险"], "suggestions": ["建议"]}}
  ],
  "confidence": 0.0-1.0
}}"#,
        serde_json::to_string_pretty(&metadata)?,
        serde_json::to_string_pretty(&scan)?
    );

    let raw = call_llm(LlmCallParams::new(base_url, api_key, model, api_format, system_prompt, &user_prompt).error_label("大模型评估请求失败"))
        .await?;
    let parsed: Value = serde_json::from_str(&extract_json_object(&raw)?)?;
    let llm_score = clamp_score(parsed.get("score").and_then(Value::as_i64), 0);
    let security_score = clamp_score(parsed.get("securityScore").and_then(Value::as_i64), llm_score);
    let performance_score =
        clamp_score(parsed.get("performanceScore").and_then(Value::as_i64), llm_score);

    Ok(json!({
        "score": llm_score,
        "securityScore": security_score,
        "performanceScore": performance_score,
        "summary": parsed.get("summary").and_then(Value::as_str).unwrap_or("目录 Skills 多 Agent 评估完成"),
        "risks": parsed.get("risks").and_then(Value::as_array).cloned().unwrap_or_default(),
        "suggestions": parsed.get("suggestions").and_then(Value::as_array).cloned().unwrap_or_default(),
        "requiredChanges": parsed.get("requiredChanges").and_then(Value::as_array).cloned().unwrap_or_default(),
        "agents": parsed.get("agents").and_then(Value::as_array).cloned().unwrap_or_default(),
        "skills": parsed.get("skills").and_then(Value::as_array).cloned().unwrap_or_default(),
        "pythonFiles": parsed.get("pythonFiles").and_then(Value::as_array).cloned().unwrap_or_default(),
        "scan": scan,
        "confidence": parsed.get("confidence").and_then(Value::as_f64).unwrap_or(0.7).clamp(0.0, 1.0),
        "source": "multi-agent-directory-llm"
    }))
}

pub(crate) fn system_info(conn: &Connection) -> Result<Value> {
    let total_skills: i64 = crate::count_table(conn, "skills")?;
    let total_agents: i64 = crate::count_table(conn, "agents")?;
    let total_sessions: i64 = crate::count_table(conn, "sessions")?;
    let first_launch = total_skills == 0 && total_agents == 0 && total_sessions == 0;
    Ok(json!({
        "runtime": "tauri-rust",
        "database": "sqlite",
        "start_time": crate::process_start_time(),
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
