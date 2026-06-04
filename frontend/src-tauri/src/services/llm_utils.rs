use anyhow::{anyhow, Result};
use reqwest::header;
use serde_json::{json, Value};

pub(crate) fn extract_text_from_content(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.trim().is_empty() => Some(text.to_string()),
        Value::Array(items) => {
            let parts = items
                .iter()
                .filter_map(|item| {
                    item.as_str()
                        .map(ToString::to_string)
                        .or_else(|| {
                            item.get("text")
                                .and_then(Value::as_str)
                                .map(ToString::to_string)
                        })
                        .or_else(|| item.get("content").and_then(extract_text_from_content))
                })
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        Value::Object(map) => map
            .get("text")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or_else(|| map.get("content").and_then(extract_text_from_content)),
        _ => None,
    }
}

pub(crate) fn extract_llm_text(value: &Value, is_anthropic: bool) -> Option<String> {
    if let Some(text) = value.get("output_text").and_then(Value::as_str) {
        if !text.trim().is_empty() {
            return Some(text.to_string());
        }
    }
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        if !text.trim().is_empty() {
            return Some(text.to_string());
        }
    }
    if is_anthropic {
        if let Some(text) = value.get("content").and_then(extract_text_from_content) {
            return Some(text);
        }
    }
    value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| {
            choice
                .get("message")
                .and_then(|message| {
                    message
                        .get("content")
                        .and_then(extract_text_from_content)
                        .or_else(|| {
                            message
                                .get("reasoning_content")
                                .and_then(Value::as_str)
                                .map(ToString::to_string)
                        })
                })
                .or_else(|| {
                    choice
                        .get("text")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                })
        })
        .or_else(|| {
            value
                .get("output")
                .and_then(Value::as_array)
                .and_then(|items| {
                    items
                        .iter()
                        .find_map(|item| item.get("content").and_then(extract_text_from_content))
                })
        })
}

pub(crate) fn response_shape(value: &Value) -> String {
    match value {
        Value::Object(map) => format!(
            "top-level keys: {}",
            map.keys().cloned().collect::<Vec<_>>().join(", ")
        ),
        other => format!("top-level type: {}", other),
    }
}

pub(crate) struct LlmCallParams {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub api_format: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub max_tokens: u32,
    pub temperature: f64,
    pub error_label: String,
}

impl LlmCallParams {
    pub(crate) fn new(
        base_url: String,
        api_key: String,
        model: String,
        api_format: String,
        system_prompt: impl Into<String>,
        user_prompt: impl Into<String>,
    ) -> Self {
        Self {
            base_url,
            api_key,
            model,
            api_format,
            system_prompt: system_prompt.into(),
            user_prompt: user_prompt.into(),
            max_tokens: 2048,
            temperature: 0.1,
            error_label: "大模型请求".into(),
        }
    }

    pub(crate) fn max_tokens(mut self, value: u32) -> Self {
        self.max_tokens = value;
        self
    }

    pub(crate) fn temperature(mut self, value: f64) -> Self {
        self.temperature = value;
        self
    }

    pub(crate) fn error_label(mut self, value: impl Into<String>) -> Self {
        self.error_label = value.into();
        self
    }
}

pub(crate) async fn call_llm(params: LlmCallParams) -> Result<String> {
    let is_anthropic = params.api_format == "anthropic";
    let url = if is_anthropic {
        format!("{}/v1/messages", params.base_url)
    } else {
        format!("{}/chat/completions", params.base_url)
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let mut req = client
        .post(&url)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json");

    if is_anthropic {
        req = req
            .header("x-api-key", &params.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": params.model,
                "max_tokens": params.max_tokens,
                "system": params.system_prompt,
                "messages": [
                    { "role": "user", "content": params.user_prompt }
                ],
                "temperature": params.temperature
            }));
    } else {
        req = req.bearer_auth(&params.api_key).json(&json!({
            "model": params.model,
            "messages": [
                { "role": "system", "content": params.system_prompt },
                { "role": "user", "content": params.user_prompt }
            ],
            "max_tokens": params.max_tokens,
            "temperature": params.temperature
        }));
    }

    let response = req.send().await?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown")
        .to_string();
    let text = response
        .text()
        .await
        .unwrap_or_else(|err| format!("[Failed to read response body: {err}]"));

    if !status.is_success() {
        let preview: String = text.chars().take(500).collect();
        return Err(anyhow!(
            "{}：HTTP {} {} {}",
            params.error_label,
            status.as_u16(),
            content_type,
            preview
        ));
    }

    let value: Value = serde_json::from_str(&text)?;
    extract_llm_text(&value, is_anthropic).ok_or_else(|| {
        anyhow!(
            "大模型响应缺少文本内容（{}）。原始响应预览：{}",
            response_shape(&value),
            text.chars().take(500).collect::<String>()
        )
    })
}
