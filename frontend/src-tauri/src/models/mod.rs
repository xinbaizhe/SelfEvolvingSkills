use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SkillInfo {
    pub name: String,
    pub description: Option<String>,
    pub origin: Option<String>,
    pub source_type: String,
    pub agent_source: String,
    pub plugin_name: Option<String>,
    pub file_path: String,
    pub yaml_raw: Option<String>,
    pub body_text: Option<String>,
    pub body_size: i64,
    pub line_count: i64,
    pub file_mtime: Option<f64>,
    pub file_size: i64,
    pub category: String,
    pub category_tags: Vec<String>,
    pub file_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentInfo {
    pub name: String,
    pub description: Option<String>,
    pub tools: Option<Value>,
    pub model: Option<String>,
    pub agent_source: String,
    pub file_path: String,
    pub yaml_raw: Option<String>,
    pub body_text: Option<String>,
    pub body_size: i64,
    pub line_count: i64,
    pub file_mtime: Option<f64>,
    pub file_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SessionInfo {
    pub session_id: String,
    pub pid: Option<i64>,
    pub cwd: Option<String>,
    pub project_name: Option<String>,
    pub agent_source: String,
    pub entrypoint: Option<String>,
    pub version: Option<String>,
    pub kind: Option<String>,
    pub started_at: Option<String>,
    pub message_count: i64,
    pub first_prompt: Option<String>,
    pub jsonl_path: Option<String>,
    pub jsonl_size: i64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SourcePayload {
    pub agent_id: String,
    pub agent_name: String,
    pub detected_path: Option<String>,
    pub paths: HashMap<String, Option<String>>,
    pub is_enabled: bool,
    pub is_available: bool,
    pub record_count: i64,
    pub last_activity: Option<String>,
    pub last_scan_at: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct SourcePaths {
    pub agent_id: String,
    pub paths: HashMap<String, Option<String>>,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentSource {
    pub id: &'static str,
    pub name: &'static str,
    pub skills_path: Option<PathBuf>,
    pub agents_path: Option<PathBuf>,
    pub sessions_path: Option<PathBuf>,
    pub projects_path: Option<PathBuf>,
    pub plugins_path: Option<PathBuf>,
    pub memory_path: Option<PathBuf>,
    pub default_enabled: bool,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct PageQuery {
    pub page: Option<i64>,
    pub size: Option<i64>,
    pub category: Option<String>,
    pub source: Option<String>,
    pub agent_source: Option<String>,
    pub search: Option<String>,
    pub project: Option<String>,
    pub limit: Option<i64>,
    pub format: Option<String>,
    pub table: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SourceUpdate {
    pub is_enabled: Option<bool>,
    pub detected_path: Option<String>,
    pub paths: Option<HashMap<String, Option<String>>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: T,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data,
            error: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct EmptyData {}
