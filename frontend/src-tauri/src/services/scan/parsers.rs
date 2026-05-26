use crate::{
    format_system_time, hash_bytes, parse_frontmatter, system_time_secs, truncate_chars, AgentInfo,
    SessionInfo, SkillInfo, SourcePaths,
};
use chrono::TimeZone;
use serde_json::{json, Value};
use std::{fs, path::Path};

use crate::{count_jsonl_lines, derive_project_name, extract_first_user_prompt};

pub(super) fn is_agent_definition_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|s| s.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("md") | Some("yaml") | Some("yml")
    )
}

pub(crate) fn parse_skill_file(
    path: &Path,
    source_type: &str,
    agent_source: &str,
    plugin_name: Option<String>,
) -> Option<SkillInfo> {
    let text = fs::read_to_string(path).ok()?;
    let meta = fs::metadata(path).ok()?;
    let (yaml_raw, parsed, body) = parse_frontmatter(&text);
    let name = parsed
        .get("name")
        .and_then(Value::as_str)
        .map(String::from)
        .or_else(|| {
            path.parent()?
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
        })?;
    let description = parsed
        .get("description")
        .and_then(Value::as_str)
        .map(String::from);
    let origin = parsed
        .get("origin")
        .or_else(|| parsed.get("license"))
        .and_then(Value::as_str)
        .map(String::from);
    let line_count = text.lines().count() as i64;
    let body_size = body.len() as i64;
    let category = derive_category(&name);
    let category_tags = derive_tags(&name, description.as_deref());
    let body_text = Some(truncate_chars(&body, 10_000));
    let file_hash = hash_bytes(text.as_bytes());

    Some(SkillInfo {
        name,
        description,
        origin,
        source_type: source_type.into(),
        agent_source: agent_source.into(),
        plugin_name,
        file_path: path.to_string_lossy().to_string(),
        yaml_raw,
        body_text,
        body_size,
        line_count,
        file_mtime: meta.modified().ok().map(system_time_secs),
        file_size: meta.len() as i64,
        category,
        category_tags,
        file_hash,
    })
}

fn derive_category(name: &str) -> String {
    let lower = name.to_lowercase();
    let prefix = lower.split('-').next().unwrap_or(&lower);
    let groups: [(&str, &[&str]); 10] = [
        ("python", &["python"]),
        ("golang", &["golang", "go"]),
        (
            "java",
            &[
                "java",
                "kotlin",
                "springboot",
                "jpa",
                "android",
                "compose",
                "gradle",
            ],
        ),
        ("cpp", &["cpp"]),
        ("rust", &["rust"]),
        (
            "frontend",
            &[
                "frontend",
                "vue",
                "react",
                "nextjs",
                "nuxt",
                "typescript",
                "javascript",
                "flutter",
                "swiftui",
                "swift",
            ],
        ),
        (
            "document",
            &["document", "docx", "pptx", "xlsx", "pdf", "internal-comms"],
        ),
        (
            "workflow",
            &[
                "tdd",
                "e2e",
                "verification",
                "code-review",
                "debugging",
                "build",
                "refactor",
            ],
        ),
        (
            "meta",
            &[
                "colleague",
                "continuous-learning",
                "skill",
                "configure",
                "eval",
                "harness",
                "project",
                "rules",
                "sessions",
                "strategic",
                "brainstorm",
            ],
        ),
        ("backend", &["django", "laravel", "ktor", "exposed", "api"]),
    ];
    for (category, values) in groups {
        if values.contains(&prefix) || values.iter().any(|v| lower.contains(v)) {
            return category.to_string();
        }
    }
    "other".into()
}

fn derive_tags(name: &str, description: Option<&str>) -> Vec<String> {
    let haystack = format!(
        "{} {}",
        name.to_lowercase(),
        description.unwrap_or("").to_lowercase()
    );
    [
        "python",
        "golang",
        "java",
        "kotlin",
        "rust",
        "cpp",
        "typescript",
        "frontend",
        "backend",
        "testing",
        "tdd",
        "code-review",
        "security",
        "document",
        "skill",
    ]
    .into_iter()
    .filter(|tag| haystack.contains(tag))
    .map(String::from)
    .collect()
}

pub(super) fn parse_agent_file(path: &Path, agent_source: &str) -> Option<AgentInfo> {
    let text = fs::read_to_string(path).ok()?;
    let meta = fs::metadata(path).ok()?;
    let is_yaml = matches!(
        path.extension()
            .and_then(|s| s.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("yaml") | Some("yml")
    );
    let (yaml_raw, parsed, body) = if is_yaml {
        let parsed = yaml_rust2::YamlLoader::load_from_str(&text)
            .ok()
            .and_then(|docs| docs.into_iter().next())
            .and_then(crate::yaml_to_json)
            .unwrap_or_else(|| json!({}));
        (Some(text.clone()), parsed, text.clone())
    } else {
        let (yaml, map, body) = parse_frontmatter(&text);
        (
            yaml,
            serde_json::to_value(map).unwrap_or_else(|_| json!({})),
            body,
        )
    };
    let interface = parsed.get("interface");
    let name = parsed
        .get("name")
        .or_else(|| parsed.get("display_name"))
        .or_else(|| interface.and_then(|value| value.get("display_name")))
        .and_then(Value::as_str)
        .map(String::from)
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().to_string()))?;
    let description = parsed
        .get("description")
        .or_else(|| parsed.get("short_description"))
        .or_else(|| interface.and_then(|value| value.get("short_description")))
        .or_else(|| interface.and_then(|value| value.get("default_prompt")))
        .and_then(Value::as_str)
        .map(String::from);
    let tools = parsed
        .get("tools")
        .or_else(|| parsed.get("allowed_tools"))
        .cloned();
    Some(AgentInfo {
        name,
        description,
        tools,
        model: parsed
            .get("model")
            .or_else(|| parsed.get("default_model"))
            .and_then(Value::as_str)
            .map(String::from),
        agent_source: agent_source.into(),
        file_path: path.to_string_lossy().to_string(),
        yaml_raw,
        body_text: Some(truncate_chars(&body, 10_000)),
        body_size: body.len() as i64,
        line_count: text.lines().count() as i64,
        file_mtime: meta.modified().ok().map(system_time_secs),
        file_size: meta.len() as i64,
    })
}

pub(super) fn parse_session_file(
    path: &Path,
    agent_source: &str,
    sources: &[SourcePaths],
) -> Option<SessionInfo> {
    let text = fs::read_to_string(path).ok()?;
    let data: Value = serde_json::from_str(&text).ok()?;
    let session_id = data.get("sessionId").and_then(Value::as_str)?.to_string();
    if session_id.is_empty() {
        return None;
    }
    let cwd = data.get("cwd").and_then(Value::as_str).map(String::from);
    let project_name = derive_project_name(cwd.as_deref(), path);
    let mut jsonl_path = None;
    let mut jsonl_size = 0;
    let mut message_count = 0;
    if let Some(project_name) = &project_name {
        for source in sources {
            if let Some(Some(projects_path)) = source.paths.get("projects_path") {
                let candidate = Path::new(projects_path)
                    .join(project_name)
                    .join(format!("{session_id}.jsonl"));
                if candidate.exists() {
                    jsonl_size = fs::metadata(&candidate)
                        .map(|m| m.len() as i64)
                        .unwrap_or(0);
                    message_count = count_jsonl_lines(&candidate);
                    jsonl_path = Some(candidate.to_string_lossy().to_string());
                    break;
                }
            }
        }
    }
    let first_prompt = jsonl_path
        .as_deref()
        .and_then(|path| extract_first_user_prompt(Path::new(path)))
        .or_else(|| {
            data.get("prompt")
                .or_else(|| data.get("firstPrompt"))
                .or_else(|| data.get("message"))
                .and_then(Value::as_str)
                .map(|value| truncate_chars(value, 500))
        });
    let started_at = data
        .get("startedAt")
        .and_then(Value::as_i64)
        .and_then(|millis| chrono::Local.timestamp_millis_opt(millis).single())
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string());

    Some(SessionInfo {
        session_id,
        pid: data.get("pid").and_then(Value::as_i64),
        cwd,
        project_name,
        agent_source: agent_source.into(),
        entrypoint: data
            .get("entrypoint")
            .and_then(Value::as_str)
            .map(String::from),
        version: data
            .get("version")
            .and_then(Value::as_str)
            .map(String::from),
        kind: data.get("kind").and_then(Value::as_str).map(String::from),
        started_at,
        message_count,
        compressed_summary: jsonl_path
            .as_deref()
            .and_then(|path| compress_session_jsonl(Path::new(path), first_prompt.as_deref())),
        first_prompt,
        jsonl_path,
        jsonl_size,
        matched_skill: None,
    })
}

pub(super) fn parse_session_jsonl_file(path: &Path, agent_source: &str) -> Option<SessionInfo> {
    let meta = fs::metadata(path).ok()?;
    let session_id = path.file_stem()?.to_string_lossy().to_string();
    if session_id.is_empty() {
        return None;
    }
    let first_value = crate::first_jsonl_value(path);
    let cwd = first_value
        .as_ref()
        .and_then(|value| {
            value
                .get("cwd")
                .or_else(|| value.get("currentWorkingDirectory"))
                .or_else(|| value.get("workspace"))
                .and_then(Value::as_str)
        })
        .map(String::from);
    let project_name = derive_project_name(cwd.as_deref(), path);
    let started_at = first_value
        .as_ref()
        .and_then(extract_started_at)
        .or_else(|| meta.modified().ok().map(format_system_time));
    let first_prompt = extract_first_user_prompt(path);
    let message_count = count_jsonl_lines(path);
    let jsonl_path = Some(path.to_string_lossy().to_string());
    let jsonl_size = meta.len() as i64;

    Some(SessionInfo {
        session_id,
        pid: first_value
            .as_ref()
            .and_then(|value| value.get("pid").and_then(Value::as_i64)),
        cwd,
        project_name,
        agent_source: agent_source.into(),
        entrypoint: first_value
            .as_ref()
            .and_then(|value| value.get("entrypoint").and_then(Value::as_str))
            .map(String::from),
        version: first_value
            .as_ref()
            .and_then(|value| value.get("version").and_then(Value::as_str))
            .map(String::from),
        kind: Some("jsonl".into()),
        started_at,
        message_count,
        compressed_summary: compress_session_jsonl(path, first_prompt.as_deref()),
        first_prompt,
        jsonl_path,
        jsonl_size,
        matched_skill: None,
    })
}

fn extract_started_at(value: &Value) -> Option<String> {
    if let Some(millis) = value
        .get("startedAt")
        .or_else(|| value.get("timestamp"))
        .and_then(Value::as_i64)
    {
        return chrono::Local
            .timestamp_millis_opt(millis)
            .single()
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string());
    }
    value
        .get("timestamp")
        .or_else(|| value.get("created_at"))
        .or_else(|| value.get("createdAt"))
        .and_then(Value::as_str)
        .map(String::from)
}

fn compress_session_jsonl(path: &Path, first_prompt: Option<&str>) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let mut user_goals = Vec::new();
    let mut tool_names = std::collections::HashSet::new();
    let mut errors = Vec::new();
    let mut outcomes = Vec::new();

    for line in text.lines().take(800) {
        let value: Value = serde_json::from_str(line).ok()?;
        if crate::is_user_message(&value) {
            if let Some(content) = extract_message_content(&value) {
                if !content.trim().is_empty() && user_goals.len() < 5 {
                    user_goals.push(truncate_chars(content.trim(), 180));
                }
            }
        }
        collect_tool_names(&value, &mut tool_names);
        let lower = line.to_lowercase();
        if (lower.contains("error") || lower.contains("failed")) && errors.len() < 5 {
            errors.push(truncate_chars(line, 180));
        }
        if (lower.contains("success") || lower.contains("completed")) && outcomes.len() < 5 {
            outcomes.push(truncate_chars(line, 180));
        }
    }

    if user_goals.is_empty() {
        if let Some(prompt) = first_prompt {
            user_goals.push(truncate_chars(prompt, 180));
        }
    }

    let mut tools = tool_names.into_iter().collect::<Vec<_>>();
    tools.sort();
    tools.truncate(12);

    Some(serde_json::json!({
        "algorithm": "local-structured-session-compression-v1",
        "first_goal": first_prompt.map(|prompt| truncate_chars(prompt, 240)),
        "user_goals": user_goals,
        "tools": tools,
        "errors": errors,
        "outcomes": outcomes,
        "compression_note": "Only goals, tools, errors, outcomes, and limited key context are retained to avoid sending long conversations into clustering or model analysis."
    }).to_string())
}

fn collect_tool_names(value: &Value, tools: &mut std::collections::HashSet<String>) {
    for key in ["tool_name", "name", "tool", "function_name"] {
        if let Some(name) = value.get(key).and_then(Value::as_str) {
            if name.len() <= 80 {
                tools.insert(name.to_string());
            }
        }
    }
    if let Some(message) = value.get("message") {
        collect_tool_names(message, tools);
    }
    if let Some(content) = value.get("content").and_then(Value::as_array) {
        for item in content {
            collect_tool_names(item, tools);
        }
    }
}

fn extract_message_content(value: &Value) -> Option<String> {
    if let Some(content) = value.get("content") {
        return value_content_to_text(content);
    }
    if let Some(message) = value.get("message") {
        if let Some(content) = message.get("content") {
            return value_content_to_text(content);
        }
    }
    None
}

fn value_content_to_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let parts = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(Value::as_str)
                        .or_else(|| item.get("content").and_then(Value::as_str))
                        .map(ToString::to_string)
                })
                .collect::<Vec<_>>();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        _ => None,
    }
}
