use anyhow::{anyhow, Result};
use reqwest::header;
use serde_json::{json, Value};
use std::path::Path;

use super::LlmConfig;

pub(super) fn llm_config(db_path: &Path) -> Result<Option<LlmConfig>> {
    let conn = crate::db::open_conn(db_path)?;
    if !crate::get_config_bool(&conn, "llm_enabled", false)? {
        return Ok(None);
    }
    let Some(api_key) = crate::get_config(&conn, "llm_api_key")? else {
        return Ok(None);
    };
    let base_url = crate::get_config(&conn, "llm_base_url")?
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string())
        .trim_end_matches('/')
        .to_string();
    let model = crate::get_config(&conn, "llm_model")?.unwrap_or_else(|| "gpt-5.2".to_string());
    let api_format =
        crate::get_config(&conn, "llm_api_format")?.unwrap_or_else(|| "openai".to_string());
    if api_key.trim().is_empty() || base_url.trim().is_empty() || model.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(LlmConfig {
        base_url,
        api_key,
        model,
        api_format,
    }))
}

pub(super) async fn call_llm(
    db_path: &Path,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    call_llm_with_retry(db_path, system_prompt, user_prompt, 2).await
}

pub(super) async fn call_llm_with_retry(
    db_path: &Path,
    system_prompt: &str,
    user_prompt: &str,
    retries: u32,
) -> Result<String> {
    let config = llm_config(db_path)?.ok_or_else(|| anyhow!("大模型未启用或未配置 API Key"))?;
    let is_anthropic = config.api_format == "anthropic";
    let url = if is_anthropic {
        format!("{}/v1/messages", config.base_url)
    } else {
        format!("{}/chat/completions", config.base_url)
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let mut last_error = String::new();
    for attempt in 0..=retries {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(500 * attempt as u64)).await;
        }

        let mut req = client
            .post(&url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json");
        if is_anthropic {
            req = req
                .header("x-api-key", &config.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&json!({
                    "model": config.model,
                    "max_tokens": 4096,
                    "system": system_prompt,
                    "messages": [
                        { "role": "user", "content": user_prompt }
                    ],
                    "temperature": 0.2
                }));
        } else {
            req = req.bearer_auth(&config.api_key).json(&json!({
                "model": config.model,
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": user_prompt }
                ],
                "temperature": 0.2
            }));
        }

        let response = match req.send().await {
            Ok(r) => r,
            Err(err) => {
                last_error = format!("大模型网络连接失败：{}", err);
                continue;
            }
        };

        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        let bytes = match response.bytes().await {
            Ok(b) => b,
            Err(err) => {
                last_error = format!(
                    "大模型响应读取失败 (HTTP {}, Content-Type: {})：{}",
                    status.as_u16(),
                    content_type,
                    err
                );
                continue;
            }
        };

        if bytes.is_empty() {
            last_error = format!(
                "大模型 API 返回空响应 (HTTP {}, Content-Type: {})",
                status.as_u16(),
                content_type
            );
            continue;
        }

        let text = String::from_utf8_lossy(&bytes).into_owned();

        if !status.is_success() {
            let preview: String = text.chars().take(500).collect();
            last_error = format!(
                "大模型 API 请求失败：HTTP {} (Content-Type: {}) {}",
                status.as_u16(),
                content_type,
                preview
            );
            continue;
        }

        match serde_json::from_str::<Value>(&text) {
            Ok(value) => {
                let content = if is_anthropic {
                    value
                        .get("content")
                        .and_then(Value::as_array)
                        .and_then(|items| items.first())
                        .and_then(|item| item.get("text"))
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                } else {
                    value
                        .get("choices")
                        .and_then(Value::as_array)
                        .and_then(|choices| choices.first())
                        .and_then(|choice| choice.get("message"))
                        .and_then(|message| message.get("content"))
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                };

                match content {
                    Some(c) if !c.trim().is_empty() => return Ok(c),
                    Some(_) => {
                        last_error = "大模型返回了空的 content 字段".to_string();
                        continue;
                    }
                    None => {
                        let preview: String = text.chars().take(300).collect();
                        last_error =
                            format!("大模型返回格式异常 (HTTP {})：{}", status.as_u16(), preview);
                        continue;
                    }
                }
            }
            Err(err) => {
                let preview: String = text.chars().take(300).collect();
                last_error = format!(
                    "大模型返回 JSON 解析失败 (HTTP {}): {}。原始响应：{}",
                    status.as_u16(),
                    err,
                    preview
                );
                continue;
            }
        }
    }

    Err(anyhow!("{}（已重试 {} 次）", last_error, retries))
}

/// Parse the two-part QA response: JSON metadata then ===DRAFT=== then markdown
pub(super) fn parse_qa_response(content: &str, fallback_draft: &str) -> (String, String) {
    let cleaned = content.trim();
    if let Some(draft_pos) = cleaned.find("\n===DRAFT===") {
        let meta = cleaned[..draft_pos].trim().to_string();
        let draft = cleaned[draft_pos + "\n===DRAFT===".len()..]
            .trim()
            .to_string();
        let draft = strip_markdown_fence(&draft);
        if draft.trim().len() >= 80 {
            return (meta, draft);
        }
    }
    if let Some(draft_pos) = cleaned.find("\n---DRAFT---") {
        let meta = cleaned[..draft_pos].trim().to_string();
        let draft = cleaned[draft_pos + "\n---DRAFT---".len()..]
            .trim()
            .to_string();
        let draft = strip_markdown_fence(&draft);
        if draft.trim().len() >= 80 {
            return (meta, draft);
        }
    }
    if let Some(draft_pos) = cleaned.find("===DRAFT===") {
        let meta = cleaned[..draft_pos].trim().to_string();
        let draft = cleaned[draft_pos + "===DRAFT===".len()..]
            .trim()
            .to_string();
        let draft = strip_markdown_fence(&draft);
        if draft.trim().len() >= 80 {
            return (meta, draft);
        }
    }
    (cleaned.to_string(), fallback_draft.to_string())
}

pub(super) fn extract_json_object(text: &str) -> Result<String> {
    let stripped = strip_markdown_fence(text);
    let start = stripped
        .find('{')
        .ok_or_else(|| anyhow!("大模型未返回 JSON 对象"))?;
    let end = stripped
        .rfind('}')
        .ok_or_else(|| anyhow!("大模型未返回完整 JSON 对象"))?;
    Ok(stripped[start..=end].to_string())
}

pub(super) fn sanitize_skill_name(name: &str) -> String {
    let mut value = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else if ch.is_whitespace() || ch == '_' || ch == '/' || ch == ':' {
                '-'
            } else {
                ch
            }
        })
        .collect::<String>();
    while value.contains("--") {
        value = value.replace("--", "-");
    }
    value = value.trim_matches('-').to_string();
    if value.is_empty() {
        "generated-skill".to_string()
    } else {
        value
    }
}

pub(super) fn strip_markdown_fence(text: &str) -> String {
    let trimmed = text.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }
    let mut lines = trimmed.lines().collect::<Vec<_>>();
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
    lines.join("\n").trim().to_string()
}
