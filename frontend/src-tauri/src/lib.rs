mod agent_sources;
mod db;
mod router;
mod services;
mod utils;

use anyhow::{Context, Result};
use base64::Engine;
use chrono::{DateTime, Local, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Emitter, Manager, State};
use walkdir::WalkDir;
use yaml_rust2::YamlLoader;

pub(crate) use agent_sources::{agent_sources, source_default_paths};
pub(crate) use router::{community_skill_raw_url, fetch_community_skill_markdown};
pub(crate) use services::admin_service::{get_config, get_config_bool};
use services::scan::{enabled_source_paths, sync_source_configs};
use services::team_service::{call_server_logout, TeamState};
use tauri_plugin_opener::OpenerExt;

type AppResult<T> = std::result::Result<T, String>;

#[derive(Clone)]
pub(crate) struct AgentSource {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) skills_path: Option<PathBuf>,
    pub(crate) agents_path: Option<PathBuf>,
    pub(crate) sessions_path: Option<PathBuf>,
    pub(crate) projects_path: Option<PathBuf>,
    pub(crate) plugins_path: Option<PathBuf>,
    pub(crate) memory_path: Option<PathBuf>,
    pub(crate) default_enabled: bool,
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) db_path: PathBuf,
    pub(crate) scan_lock: Arc<tokio::sync::Mutex<()>>,
    pub(crate) disk_usage_lock: Arc<tokio::sync::Mutex<()>>,
    pub(crate) disk_usage_cache: Arc<Mutex<Option<services::system::DiskUsageCache>>>,
    pub(crate) watchers: Arc<Mutex<Vec<notify::RecommendedWatcher>>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ApiResponse<T: Serialize> {
    pub(crate) success: bool,
    pub(crate) data: T,
    pub(crate) error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub(crate) fn ok(data: T) -> Self {
        Self {
            success: true,
            data,
            error: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct SourcePayload {
    pub(crate) agent_id: String,
    pub(crate) agent_name: String,
    pub(crate) detected_path: Option<String>,
    pub(crate) paths: HashMap<String, Option<String>>,
    pub(crate) is_enabled: bool,
    pub(crate) is_available: bool,
    pub(crate) record_count: i64,
    pub(crate) last_activity: Option<String>,
    pub(crate) last_scan_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SourceUpdate {
    pub(crate) is_enabled: Option<bool>,
    pub(crate) detected_path: Option<String>,
    pub(crate) paths: Option<HashMap<String, Option<String>>>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub(crate) struct PageQuery {
    pub(crate) page: Option<i64>,
    pub(crate) size: Option<i64>,
    pub(crate) category: Option<String>,
    pub(crate) source: Option<String>,
    pub(crate) agent_source: Option<String>,
    pub(crate) search: Option<String>,
    pub(crate) project: Option<String>,
    pub(crate) limit: Option<i64>,
    pub(crate) format: Option<String>,
    pub(crate) source_type: Option<String>,
    pub(crate) table: Option<String>,
    pub(crate) rowid: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ScopePayload {
    pub(crate) agent_ids: Option<Vec<String>>,
}

#[derive(Debug)]
pub(crate) struct SkillInfo {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) origin: Option<String>,
    pub(crate) source_type: String,
    pub(crate) agent_source: String,
    pub(crate) plugin_name: Option<String>,
    pub(crate) file_path: String,
    pub(crate) yaml_raw: Option<String>,
    pub(crate) body_text: Option<String>,
    pub(crate) body_size: i64,
    pub(crate) line_count: i64,
    pub(crate) file_mtime: Option<f64>,
    pub(crate) file_size: i64,
    pub(crate) category: String,
    pub(crate) category_tags: Vec<String>,
    pub(crate) file_hash: String,
}

#[derive(Debug)]
pub(crate) struct AgentInfo {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) tools: Option<Value>,
    pub(crate) model: Option<String>,
    pub(crate) agent_source: String,
    pub(crate) file_path: String,
    pub(crate) yaml_raw: Option<String>,
    pub(crate) body_text: Option<String>,
    pub(crate) body_size: i64,
    pub(crate) line_count: i64,
    pub(crate) file_mtime: Option<f64>,
    pub(crate) file_size: i64,
}

#[derive(Debug)]
pub(crate) struct SessionInfo {
    pub(crate) session_id: String,
    pub(crate) pid: Option<i64>,
    pub(crate) cwd: Option<String>,
    pub(crate) project_name: Option<String>,
    pub(crate) agent_source: String,
    pub(crate) entrypoint: Option<String>,
    pub(crate) version: Option<String>,
    pub(crate) kind: Option<String>,
    pub(crate) started_at: Option<String>,
    pub(crate) message_count: i64,
    pub(crate) first_prompt: Option<String>,
    pub(crate) compressed_summary: Option<String>,
    pub(crate) jsonl_path: Option<String>,
    pub(crate) jsonl_size: i64,
    pub(crate) matched_skill: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct SourcePaths {
    pub(crate) agent_id: String,
    pub(crate) agent_name: String,
    pub(crate) paths: HashMap<String, Option<String>>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .context("failed to resolve app data dir")?
                .join("data");
            fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("skills_analyzer.db");
            init_db(&db_path)?;
            app.manage(AppState {
                db_path,
                scan_lock: Arc::new(tokio::sync::Mutex::new(())),
                disk_usage_lock: Arc::new(tokio::sync::Mutex::new(())),
                disk_usage_cache: Arc::new(Mutex::new(None)),
                watchers: Arc::new(Mutex::new(Vec::new())),
            });
            app.manage(TeamState::new());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let team_state = window.state::<TeamState>();
                // Best-effort server logout on window close
                tauri::async_runtime::block_on(async {
                    call_server_logout(&team_state).await;
                    let _ = team_state.delete_tokens();
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            health,
            open_external_url,
            configure_claude_code_model,
            api_request,
            select_directory,
            save_file_base64,
            start_watchers,
            services::team_service::login_team,
            services::team_service::logout_team,
            services::team_service::get_team_session,
            services::team_service::team_api_get,
            services::team_service::team_api_post,
            services::team_service::team_api_put,
            services::team_service::team_api_delete,
            services::team_service::team_api_download,
            services::team_service::scan_url_stream,
            services::team_service::scan_url_agent_stream,
            services::team_service::check_team_connection,
            services::team_service::cache_team_skills,
            services::team_service::get_cached_team_skills,
            services::team_service::get_team_cache_summary,
            services::team_service::get_pending_operations,
            services::team_service::flush_pending_operations,
            services::team_service::queue_team_operation,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

use std::sync::OnceLock;
static PROCESS_START_EPOCH: OnceLock<i64> = OnceLock::new();

pub(crate) fn process_start_time() -> i64 {
    *PROCESS_START_EPOCH.get_or_init(|| {
        chrono::Local::now().timestamp()
    })
}

#[tauri::command]
fn health() -> ApiResponse<Value> {
    ApiResponse::ok(json!({ "status": "ok", "runtime": "tauri-rust", "start_time": process_start_time() }))
}

#[tauri::command]
fn open_external_url(app: AppHandle, url: String) -> AppResult<()> {
    let trimmed = url.trim();
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err("only http and https URLs can be opened".to_string());
    }
    app.opener()
        .open_url(trimmed, None::<&str>)
        .map_err(|err| err.to_string())
}

#[derive(Debug, Serialize)]
struct ClaudeCodeConfigResult {
    path: String,
    model: String,
    base_url: String,
}

#[tauri::command]
fn configure_claude_code_model(
    app: AppHandle,
    provider: String,
    base_url: String,
    api_key: String,
    model: String,
) -> AppResult<ClaudeCodeConfigResult> {
    let provider = provider.trim();
    let base_url = base_url.trim();
    let api_key = api_key.trim();
    let model = model.trim();

    if base_url.is_empty() {
        return Err("Base URL cannot be empty".to_string());
    }
    if api_key.is_empty() {
        return Err("API Key cannot be empty".to_string());
    }
    if model.is_empty() {
        return Err("Model cannot be empty".to_string());
    }

    let claude_dir = app
        .path()
        .home_dir()
        .map_err(|err| err.to_string())?
        .join(".claude");
    fs::create_dir_all(&claude_dir).map_err(|err| err.to_string())?;
    let settings_path = claude_dir.join("settings.json");

    let mut settings = if settings_path.exists() {
        let raw = fs::read_to_string(&settings_path).map_err(|err| err.to_string())?;
        serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    if !settings.is_object() {
        settings = json!({});
    }
    let settings_obj = settings
        .as_object_mut()
        .ok_or_else(|| "Claude Code settings must be a JSON object".to_string())?;
    let env_value = settings_obj.entry("env").or_insert_with(|| json!({}));
    if !env_value.is_object() {
        *env_value = json!({});
    }
    let env = env_value
        .as_object_mut()
        .ok_or_else(|| "Claude Code env settings must be a JSON object".to_string())?;

    env.insert(
        "ANTHROPIC_BASE_URL".to_string(),
        Value::String(base_url.to_string()),
    );
    for (key, value) in claude_code_model_envs(model) {
        env.insert(key.to_string(), Value::String(value));
    }
    if provider.eq_ignore_ascii_case("anthropic") {
        env.insert(
            "ANTHROPIC_API_KEY".to_string(),
            Value::String(api_key.to_string()),
        );
        env.remove("ANTHROPIC_AUTH_TOKEN");
    } else {
        env.insert(
            "ANTHROPIC_AUTH_TOKEN".to_string(),
            Value::String(api_key.to_string()),
        );
        env.remove("ANTHROPIC_API_KEY");
    }

    let pretty = serde_json::to_string_pretty(&settings).map_err(|err| err.to_string())?;
    fs::write(&settings_path, pretty).map_err(|err| err.to_string())?;

    Ok(ClaudeCodeConfigResult {
        path: settings_path.to_string_lossy().to_string(),
        model: model.to_string(),
        base_url: base_url.to_string(),
    })
}

fn claude_code_model_envs(model: &str) -> Vec<(&'static str, String)> {
    vec![
        ("ANTHROPIC_DEFAULT_HAIKU_MODEL", model.to_string()),
        ("ANTHROPIC_DEFAULT_OPUS_MODEL", model.to_string()),
        ("ANTHROPIC_DEFAULT_SONNET_MODEL", model.to_string()),
        ("ANTHROPIC_MODEL", model.to_string()),
        ("ANTHROPIC_SMALL_FAST_MODEL", model.to_string()),
    ]
}

#[tauri::command]
async fn select_directory(app: AppHandle) -> AppResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |folder| {
        let selected = folder.map(|path| path.to_string());
        let _ = tx.send(selected);
    });
    rx.await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_file_base64(
    app: AppHandle,
    filename: String,
    content_base64: String,
) -> AppResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;

    let default_name = if filename.trim().is_empty() {
        "skill.zip"
    } else {
        filename.trim()
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(content_base64.trim())
        .map_err(|err| err.to_string())?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(default_name)
        .save_file(move |path| {
            let result = path
                .map(|target| {
                    let target_path = target.as_path().ok_or_else(|| "无效的保存路径".to_string())?;
                    fs::write(target_path, &bytes).map_err(|err| err.to_string())?;
                    Ok::<_, String>(target_path.to_string_lossy().to_string())
                })
                .transpose();
            let _ = tx.send(result);
        });
    rx.await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn api_request(
    state: State<'_, AppState>,
    app: AppHandle,
    method: String,
    path: String,
    params: Option<Value>,
    body: Option<Value>,
) -> AppResult<Value> {
    router::dispatch_api(&state, &app, &method.to_uppercase(), &path, params, body)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn start_watchers(
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<ApiResponse<Value>> {
    use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

    let conn = open_conn(&state.db_path).map_err(|e| e.to_string())?;
    sync_source_configs(&conn).map_err(|e| e.to_string())?;
    let source_paths = enabled_source_paths(&conn).map_err(|e| e.to_string())?;

    let mut watchers = state.watchers.lock().map_err(|e| e.to_string())?;
    watchers.clear();

    for source in source_paths {
        for path in source.paths.values().flatten() {
            let path_buf = PathBuf::from(path);
            if !path_buf.exists() {
                continue;
            }
            let app_handle = app.clone();
            let watch_path = path.clone();
            let mut watcher = RecommendedWatcher::new(
                move |res: notify::Result<notify::Event>| {
                    if let Ok(event) = res {
                        if matches!(
                            event.kind,
                            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                        ) {
                            let _ = app_handle.emit(
                                "file-watch-event",
                                json!({ "path": watch_path, "kind": format!("{:?}", event.kind) }),
                            );
                        }
                    }
                },
                Config::default(),
            )
            .map_err(|e| e.to_string())?;
            watcher
                .watch(&path_buf, RecursiveMode::Recursive)
                .map_err(|e| e.to_string())?;
            watchers.push(watcher);
        }
    }

    Ok(ApiResponse::ok(json!({ "watchers": watchers.len() })))
}

fn init_db(db_path: &Path) -> Result<()> {
    db::init_db(db_path)
}

pub(crate) fn parse_scope_payload(body: Option<Value>) -> Result<ScopePayload> {
    match body {
        None | Some(Value::Null) => Ok(ScopePayload::default()),
        Some(value) => serde_json::from_value(value).map_err(Into::into),
    }
}

// ---- Shared utilities ----

pub(crate) fn open_conn(db_path: &Path) -> Result<Connection> {
    Connection::open(db_path).with_context(|| format!("open sqlite db {}", db_path.display()))
}

#[allow(clippy::ptr_arg)]
pub(crate) fn path_to_string(path: &PathBuf) -> String {
    path.to_string_lossy().to_string()
}

pub(crate) fn truncate_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

pub(crate) fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub(crate) fn system_time_secs(time: std::time::SystemTime) -> f64 {
    time.duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or_default()
}

pub(crate) fn format_system_time(time: std::time::SystemTime) -> String {
    let dt: DateTime<Utc> = time.into();
    dt.with_timezone(&Local)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

pub(crate) fn now_string() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub(crate) fn url_decode(value: &str) -> String {
    let mut buf: Vec<u8> = Vec::new();
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    buf.push(byte);
                    i += 3;
                    continue;
                }
            }
        }
        buf.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(buf)
        .unwrap_or_else(|err| String::from_utf8_lossy(&err.into_bytes()).into_owned())
}

pub(crate) fn sanitize_file_name(name: &str) -> String {
    let cleaned = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if cleaned.is_empty() {
        "generated-skill".to_string()
    } else {
        cleaned
    }
}

pub(crate) fn yaml_scalar(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

pub(crate) fn yaml_to_json(yaml: yaml_rust2::Yaml) -> Option<Value> {
    use yaml_rust2::Yaml;
    match yaml {
        Yaml::Real(s) => s.parse::<f64>().ok().map(Value::from),
        Yaml::Integer(i) => Some(Value::from(i)),
        Yaml::String(s) => Some(Value::String(s)),
        Yaml::Boolean(b) => Some(Value::Bool(b)),
        Yaml::Array(values) => Some(Value::Array(
            values.into_iter().filter_map(yaml_to_json).collect(),
        )),
        Yaml::Hash(map) => {
            let mut out = serde_json::Map::new();
            for (key, value) in map {
                if let Yaml::String(key) = key {
                    if let Some(value) = yaml_to_json(value) {
                        out.insert(key, value);
                    }
                }
            }
            Some(Value::Object(out))
        }
        _ => None,
    }
}

pub(crate) fn parse_frontmatter(text: &str) -> (Option<String>, HashMap<String, Value>, String) {
    if !text.starts_with("---") {
        return (None, HashMap::new(), text.to_string());
    }
    let mut lines = text.lines();
    let _ = lines.next();
    let mut yaml_lines = Vec::new();
    let mut found_end = false;
    for line in &mut lines {
        if line.trim() == "---" {
            found_end = true;
            break;
        }
        yaml_lines.push(line);
    }
    if !found_end {
        return (None, HashMap::new(), text.to_string());
    }
    let yaml_raw = yaml_lines.join("\n");
    let body = lines.collect::<Vec<_>>().join("\n");
    let parsed = YamlLoader::load_from_str(&yaml_raw)
        .ok()
        .and_then(|docs| docs.first().cloned())
        .and_then(yaml_to_json)
        .and_then(|value| match value {
            Value::Object(map) => Some(map.into_iter().collect()),
            _ => None,
        })
        .unwrap_or_default();
    (Some(yaml_raw), parsed, body)
}

pub(crate) fn is_user_message(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("user")
        || value.get("role").and_then(Value::as_str) == Some("user")
        || value
            .get("message")
            .and_then(|message| message.get("role"))
            .and_then(Value::as_str)
            == Some("user")
}

pub(crate) fn first_jsonl_value(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    text.lines()
        .find_map(|line| serde_json::from_str::<Value>(line).ok())
}

pub(crate) fn extract_first_user_prompt(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    for line in text.lines().take(200) {
        let value: Value = serde_json::from_str(line).ok()?;
        if !is_user_message(&value) {
            continue;
        }
        if let Some(content) = extract_message_content(&value) {
            let content = content.trim();
            if !content.is_empty() {
                return Some(truncate_chars(content, 500));
            }
        }
    }
    None
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

pub(crate) fn count_jsonl_lines(path: &Path) -> i64 {
    fs::read_to_string(path)
        .map(|text| text.lines().filter(|line| !line.trim().is_empty()).count() as i64)
        .unwrap_or(0)
}

pub(crate) fn count_records(path: &Path, key: &str) -> (i64, Option<std::time::SystemTime>) {
    let mut count = 0;
    let mut latest = None;
    let walker = match key {
        "projects_path" => WalkDir::new(path).max_depth(8),
        "extra_agents_path" => WalkDir::new(path).max_depth(4),
        "extra_skills_path" => WalkDir::new(path).max_depth(4),
        _ => WalkDir::new(path).max_depth(2),
    };
    for entry in walker.into_iter().flatten() {
        let file_type = entry.file_type();
        let matches = match key {
            "skills_path" | "extra_skills_path" => {
                file_type.is_file() && entry.file_name() == "SKILL.md"
            }
            "agents_path" | "extra_agents_path" => {
                file_type.is_file() && is_agent_definition_file(entry.path())
            }
            "projects_path" => {
                file_type.is_file()
                    && entry.path().extension().and_then(|s| s.to_str()) == Some("jsonl")
            }
            "sessions_path" => {
                file_type.is_file()
                    && matches!(
                        entry.path().extension().and_then(|s| s.to_str()),
                        Some("json") | Some("jsonl")
                    )
            }
            _ => false,
        };
        if matches {
            count += 1;
            if let Ok(meta) = entry.metadata() {
                if let Ok(modified) = meta.modified() {
                    latest = Some(
                        latest.map_or(modified, |cur: std::time::SystemTime| cur.max(modified)),
                    );
                }
            }
        }
    }
    (count, latest)
}

fn is_agent_definition_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|s| s.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("md") | Some("yaml") | Some("yml")
    )
}

pub(crate) fn count_skill_mentions(text: &str, skill_name: &str) -> usize {
    let haystack = text.to_lowercase();
    let needle = skill_name.to_lowercase();
    if needle.len() < 3 {
        return 0;
    }
    let direct = haystack.matches(&needle).count();
    let command = haystack.matches(&format!("/{needle}")).count();
    let at_ref = haystack.matches(&format!("@{needle}")).count();
    direct.max(command + at_ref)
}

pub(crate) fn derive_project_name(cwd: Option<&str>, file_path: &Path) -> Option<String> {
    if let Some(cwd) = cwd {
        return Some(cwd.replace(":\\", "--").replace(['\\', '/', ' '], "-"));
    }
    let path = file_path.to_string_lossy().replace('\\', "/");
    path.split("/projects/")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .map(String::from)
}

pub(crate) fn count_table(conn: &Connection, table: &str) -> Result<i64> {
    conn.query_row(&format!("SELECT COUNT(id) FROM {table}"), [], |row| {
        row.get(0)
    })
    .map_err(Into::into)
}

pub(crate) fn count_where(conn: &Connection, table: &str, clause: &str) -> Result<i64> {
    conn.query_row(
        &format!("SELECT COUNT(id) FROM {table} WHERE {clause}"),
        [],
        |row| row.get(0),
    )
    .map_err(Into::into)
}
