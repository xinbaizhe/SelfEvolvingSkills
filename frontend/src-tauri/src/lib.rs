mod db;
mod services;
mod utils;

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use chrono::{DateTime, Local, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
    fs,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Emitter, Manager, State};
use walkdir::WalkDir;
use yaml_rust2::YamlLoader;

type AppResult<T> = std::result::Result<T, String>;

#[derive(Clone)]
struct AgentSource {
    id: &'static str,
    name: &'static str,
    skills_path: Option<PathBuf>,
    agents_path: Option<PathBuf>,
    sessions_path: Option<PathBuf>,
    projects_path: Option<PathBuf>,
    plugins_path: Option<PathBuf>,
    memory_path: Option<PathBuf>,
    default_enabled: bool,
}

#[derive(Clone)]
struct AppState {
    db_path: PathBuf,
    scan_lock: Arc<tokio::sync::Mutex<()>>,
    watchers: Arc<Mutex<Vec<notify::RecommendedWatcher>>>,
}

#[derive(Debug, Serialize)]
struct ApiResponse<T: Serialize> {
    success: bool,
    data: T,
    error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    fn ok(data: T) -> Self {
        Self {
            success: true,
            data,
            error: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct SourcePayload {
    agent_id: String,
    agent_name: String,
    detected_path: Option<String>,
    paths: HashMap<String, Option<String>>,
    is_enabled: bool,
    is_available: bool,
    record_count: i64,
    last_activity: Option<String>,
    last_scan_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SourceUpdate {
    is_enabled: Option<bool>,
    detected_path: Option<String>,
    paths: Option<HashMap<String, Option<String>>>,
}

#[derive(Debug, Deserialize, Default)]
struct PageQuery {
    page: Option<i64>,
    size: Option<i64>,
    category: Option<String>,
    source: Option<String>,
    agent_source: Option<String>,
    search: Option<String>,
    project: Option<String>,
    limit: Option<i64>,
    format: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ScopePayload {
    agent_ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct ImportSkillPayload {
    agent_id: String,
    filename: Option<String>,
    content: String,
    content_base64: Option<String>,
}

#[derive(Debug)]
struct SkillInfo {
    name: String,
    description: Option<String>,
    origin: Option<String>,
    source_type: String,
    agent_source: String,
    plugin_name: Option<String>,
    file_path: String,
    yaml_raw: Option<String>,
    body_text: Option<String>,
    body_size: i64,
    line_count: i64,
    file_mtime: Option<f64>,
    file_size: i64,
    category: String,
    category_tags: Vec<String>,
    file_hash: String,
}

#[derive(Debug)]
struct AgentInfo {
    name: String,
    description: Option<String>,
    tools: Option<Value>,
    model: Option<String>,
    agent_source: String,
    file_path: String,
    yaml_raw: Option<String>,
    body_text: Option<String>,
    body_size: i64,
    line_count: i64,
    file_mtime: Option<f64>,
    file_size: i64,
}

#[derive(Debug)]
struct SessionInfo {
    session_id: String,
    pid: Option<i64>,
    cwd: Option<String>,
    project_name: Option<String>,
    agent_source: String,
    entrypoint: Option<String>,
    version: Option<String>,
    kind: Option<String>,
    started_at: Option<String>,
    message_count: i64,
    first_prompt: Option<String>,
    compressed_summary: Option<String>,
    jsonl_path: Option<String>,
    jsonl_size: i64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SourcePaths {
    agent_id: String,
    agent_name: String,
    paths: HashMap<String, Option<String>>,
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
                watchers: Arc::new(Mutex::new(Vec::new())),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health,
            api_request,
            select_directory,
            start_watchers,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn health() -> ApiResponse<Value> {
    ApiResponse::ok(json!({ "status": "ok", "runtime": "tauri-rust" }))
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
async fn api_request(
    state: State<'_, AppState>,
    app: AppHandle,
    method: String,
    path: String,
    params: Option<Value>,
    body: Option<Value>,
) -> AppResult<Value> {
    dispatch_api(&state, &app, &method.to_uppercase(), &path, params, body)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn start_watchers(state: State<'_, AppState>, app: AppHandle) -> AppResult<ApiResponse<Value>> {
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

async fn dispatch_api(
    state: &AppState,
    app: &AppHandle,
    method: &str,
    path: &str,
    params_value: Option<Value>,
    body: Option<Value>,
) -> Result<Value> {
    let clean_path = path.trim_start_matches("/api").trim_end_matches('/');
    let query: PageQuery = serde_json::from_value(params_value.unwrap_or(Value::Null)).unwrap_or_default();

    // LLM test: pre-fill stored API key if not provided in request
    if matches!((method, clean_path), ("POST", "/admin/config/llm/test") | ("POST", "admin/config/llm/test")) {
        let mut body = body.unwrap_or(Value::Null);
        let api_key_empty = body
            .get("llm_api_key")
            .and_then(Value::as_str)
            .map(|s| s.trim().is_empty())
            .unwrap_or(true);
        if api_key_empty {
            let conn = open_conn(&state.db_path)?;
            if let Ok(Some(stored_key)) = get_config(&conn, "llm_api_key") {
                if let Some(obj) = body.as_object_mut() {
                    obj.insert("llm_api_key".to_string(), Value::String(stored_key));
                }
            }
        }
        return test_llm_connection(Some(body)).await.map(|value| json!(ApiResponse::ok(value)));
    }

    // Scan requires the lock to be acquired before opening the connection
    if matches!((method, clean_path), ("POST", "/scan") | ("POST", "scan")) {
        let _guard = state.scan_lock.lock().await;
        let conn = open_conn(&state.db_path)?;
        let scope = parse_scope_payload(body)?;
        return run_full_scan(&conn, app, scope.agent_ids.as_deref()).map(|value| json!(ApiResponse::ok(value)));
    }

    // Community search uses async HTTP and manages its own connection
    if matches!((method, clean_path), ("GET", "/community/search") | ("GET", "community/search")) {
        let q_str = query.search.unwrap_or_default();
        let page = query.page.unwrap_or(1);
        let size = query.size.unwrap_or(20);
        return services::community_service::search_community_skills(&state.db_path, &q_str, page, size)
            .await
            .map(|value| json!(ApiResponse::ok(value)));
    }

    if matches!((method, clean_path), ("POST", "/community/install") | ("POST", "community/install")) {
        return install_community_skill(&state.db_path, body)
            .await
            .map(|value| json!(ApiResponse::ok(value)));
    }

    let conn = open_conn(&state.db_path)?;

    match (method, clean_path) {
        ("GET", "/health") | ("GET", "health") => Ok(json!(ApiResponse::ok(json!({ "status": "ok" })))),
        ("GET", "/scan/history") | ("GET", "scan/history") => {
            list_scan_history(&conn, query).map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/scan/sources") | ("GET", "scan/sources") => {
            sync_source_configs(&conn)?;
            list_sources(&conn).map(|value| json!(ApiResponse::ok(value)))
        }
        ("POST", "/scan/sources/detect") | ("POST", "scan/sources/detect") => {
            sync_source_configs(&conn)?;
            list_sources(&conn).map(|value| json!(ApiResponse::ok(value)))
        }
        ("PUT", p) if p.starts_with("/scan/sources/") || p.starts_with("scan/sources/") => {
            let agent_id = p.trim_start_matches('/').trim_start_matches("scan/sources/");
            let payload: SourceUpdate = serde_json::from_value(body.unwrap_or(Value::Null))?;
            update_source(&conn, agent_id, payload).map(|value| json!(ApiResponse::ok(value)))
        }
        ("DELETE", p) if p.starts_with("/scan/sources/") || p.starts_with("scan/sources/") => {
            let agent_id = p.trim_start_matches('/').trim_start_matches("scan/sources/");
            reset_source(&conn, agent_id)?;
            Ok(json!(ApiResponse::ok(json!({ "agent_id": agent_id, "message": "Source reset" }))))
        }
        ("GET", "/skills") | ("GET", "skills") => list_skills(&conn, query).map(|value| json!(ApiResponse::ok(value))),
        ("GET", "/skills/categories") | ("GET", "skills/categories") => {
            list_categories(&conn).map(|value| json!(ApiResponse::ok(value)))
        }
        ("POST", "/skills/import") | ("POST", "skills/import") => {
            import_skills(&conn, body).map(|value| json!(ApiResponse::ok(value)))
        }
        ("POST", p) if p.starts_with("/skills/") && p.ends_with("/evolve") || p.starts_with("skills/") && p.ends_with("/evolve") => {
            let name = url_decode(
                p.trim_start_matches('/')
                    .trim_start_matches("skills/")
                    .trim_end_matches("/evolve"),
            );
            create_manual_skill_draft(&conn, &name).map(|value| json!(ApiResponse::ok(value)))
        }
        ("PUT", p) if p.starts_with("/skills/") || p.starts_with("skills/") => {
            let name = url_decode(p.trim_start_matches('/').trim_start_matches("skills/"));
            update_skill(&conn, &name, &body.unwrap_or(Value::Null)).map(|value| json!(ApiResponse::ok(value)))
        }
        ("DELETE", p) if p.starts_with("/skills/") || p.starts_with("skills/") => {
            let name = url_decode(p.trim_start_matches('/').trim_start_matches("skills/"));
            delete_skill(&conn, &name).map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", p) if p.starts_with("/skills/") || p.starts_with("skills/") => {
            let name = url_decode(p.trim_start_matches('/').trim_start_matches("skills/"));
            get_skill(&conn, &name).map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/agents") | ("GET", "agents") => list_agents(&conn, query).map(|value| json!(ApiResponse::ok(value))),
        ("GET", p) if p.starts_with("/agents/") || p.starts_with("agents/") => {
            let name = url_decode(p.trim_start_matches('/').trim_start_matches("agents/"));
            get_agent(&conn, &name).map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/stats/summary") | ("GET", "stats/summary") => {
            stats_summary(&conn).map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/stats/skills/top") | ("GET", "stats/skills/top") => {
            top_skills(&conn, query.limit.unwrap_or(20)).map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/stats/skills/by-category") | ("GET", "stats/skills/by-category") => {
            skills_by_category(&conn).map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/sessions") | ("GET", "sessions") => list_sessions(&conn, query).map(|value| json!(ApiResponse::ok(value))),
        ("GET", p) if p.starts_with("/sessions/") || p.starts_with("sessions/") => {
            let session_id = url_decode(p.trim_start_matches('/').trim_start_matches("sessions/"));
            get_session(&conn, &session_id).map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/admin/system") | ("GET", "admin/system") => {
            sync_source_configs(&conn)?;
            system_info(&conn).map(|value| json!(ApiResponse::ok(value)))
        },
        ("GET", "/admin/config/scan-paths") | ("GET", "admin/config/scan-paths") => {
            sync_source_configs(&conn)?;
            list_sources(&conn).map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/admin/config/llm") | ("GET", "admin/config/llm") => Ok(json!(ApiResponse::ok(json!({
            "enabled": get_config_bool(&conn, "llm_enabled", false)?,
            "provider": get_config(&conn, "llm_provider")?.unwrap_or_else(|| "openai".to_string()),
            "base_url": get_config(&conn, "llm_base_url")?.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            "model": get_config(&conn, "llm_model")?.unwrap_or_else(|| "gpt-5.2".to_string()),
            "has_api_key": get_config(&conn, "llm_api_key")?.is_some()
        })))),
        ("GET", "/export/skills") | ("GET", "export/skills") => {
            export_skills(&conn, query).map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/export/agents") | ("GET", "export/agents") => {
            export_agents(&conn).map(|value| json!(ApiResponse::ok(value)))
        }
        ("PUT", "/admin/config/llm") | ("PUT", "admin/config/llm") => {
            save_llm_config(&conn, body).map(|value| json!(ApiResponse::ok(value)))
        }
        ("POST", "/admin/login") | ("POST", "admin/login") => Ok(json!(ApiResponse::ok(json!({
            "token": "tauri-local",
            "username": body
                .as_ref()
                .and_then(|v| v.get("username"))
                .and_then(Value::as_str)
                .unwrap_or("local"),
            "user": { "id": 1, "username": "local" }
        })))),
        ("GET", "/admin/users") | ("GET", "admin/users") => {
            list_admin_users(&conn).map(|value| json!(ApiResponse::ok(value)))
        }
        ("POST", "/admin/users") | ("POST", "admin/users") => {
            create_admin_user(&conn, body).map(|value| json!(ApiResponse::ok(value)))
        }
        ("PUT", p) if p.starts_with("/admin/users/") || p.starts_with("admin/users/") => {
            let id = p
                .trim_start_matches('/')
                .trim_start_matches("admin/users/")
                .parse::<i64>()?;
            update_admin_user(&conn, id, body).map(|value| json!(ApiResponse::ok(value)))
        }
        ("DELETE", p) if p.starts_with("/admin/users/") || p.starts_with("admin/users/") => {
            let id = p
                .trim_start_matches('/')
                .trim_start_matches("admin/users/")
                .parse::<i64>()?;
            delete_admin_user(&conn, id)?;
            Ok(json!(ApiResponse::ok(json!({ "id": id }))))
        }
        // Workflow clustering (now real)
        ("GET", "/workflows") | ("GET", "workflows") => {
            list_workflows(&conn, query).map(|value| json!(ApiResponse::ok(value)))
        }
        ("POST", "/workflows/cluster") | ("POST", "workflows/cluster") => {
            let clusters = services::workflow_service::cluster_workflows(&conn);
            services::workflow_service::save_clusters(&conn, &clusters)?;
            services::workflow_service::generate_skill_drafts(&conn, &clusters);
            list_workflows(&conn, query).map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/workflows/install-targets") | ("GET", "workflows/install-targets") => {
            list_skill_install_targets(&conn).map(|value| json!(ApiResponse::ok(value)))
        }
        ("PUT", p) if p.starts_with("/workflows/") || p.starts_with("workflows/") => {
            let id_text = p.trim_start_matches('/').trim_start_matches("workflows/");
            let id = id_text.parse::<i64>()?;
            update_workflow_draft(&conn, id, body).map(|value| json!(ApiResponse::ok(value)))
        }
        ("DELETE", p) if p.starts_with("/workflows/") || p.starts_with("workflows/") => {
            let id_text = p.trim_start_matches('/').trim_start_matches("workflows/");
            let id = id_text.parse::<i64>()?;
            delete_workflow_draft(&conn, id)?;
            Ok(json!(ApiResponse::ok(json!({ "id": id }))))
        }
        ("POST", p) if p.starts_with("/workflows/") && p.ends_with("/install") || p.starts_with("workflows/") && p.ends_with("/install") => {
            let id_text = p
                .trim_start_matches('/')
                .trim_start_matches("workflows/")
                .trim_end_matches("/install");
            let id = id_text.parse::<i64>()?;
            install_workflow_skill(&conn, id, body).map(|value| json!(ApiResponse::ok(value)))
        }
        // Evolution pipeline
        ("POST", "/evolution/start") | ("POST", "evolution/start") => {
            sync_source_configs(&conn)?;
            services::evolution_service::cleanup_stale_jobs(&conn);
            if services::evolution_service::has_incomplete_run(&conn) {
                return Ok(json!(ApiResponse::<Value> {
                    success: false,
                    data: json!(null),
                    error: Some("进化管道正在运行中，请等待完成后再启动新的进化。如果确认已中断，请点击「加载状态」后使用「重置卡住的管道」。".to_string()),
                }));
            }
            let scope = parse_scope_payload(body)?;
            let result = services::evolution_service::start_evolution_pipeline(
                &conn,
                app,
                &state.db_path,
                state.scan_lock.clone(),
                scope.agent_ids,
            );
            Ok(json!(ApiResponse::ok(result)))
        }
        ("GET", "/evolution/status") | ("GET", "evolution/status") => {
            Ok(json!(ApiResponse::ok(services::evolution_service::get_evolution_status(&conn))))
        }
        ("POST", "/evolution/reset") | ("POST", "evolution/reset") => {
            Ok(json!(ApiResponse::ok(services::evolution_service::reset_stuck_evolution(&conn))))
        }
        ("GET", "/evolution/history") | ("GET", "evolution/history") => {
            services::evolution_service::cleanup_stale_jobs(&conn);
            Ok(json!(ApiResponse::ok(services::evolution_service::list_evolution_history(&conn, &query))))
        }
        // System monitoring
        ("GET", "/system/monitor") | ("GET", "system/monitor") => {
            Ok(json!(ApiResponse::ok(services::system_service::get_system_monitor())))
        }
        ("GET", "/system/database") | ("GET", "system/database") => {
            Ok(json!(ApiResponse::ok(services::system_service::get_database_info(&state.db_path, &conn))))
        }
        ("POST", "/system/clear-data") | ("POST", "system/clear-data") => {
            Ok(json!(ApiResponse::ok(services::system_service::clear_all_data(&conn))))
        }
        ("POST", "/system/clear-logs") | ("POST", "system/clear-logs") => {
            Ok(json!(ApiResponse::ok(services::system_service::clear_logs(&conn))))
        }
        ("POST", "/system/initialize-database") | ("POST", "system/initialize-database") => {
            let value = services::system_service::initialize_database(&conn);
            sync_source_configs(&conn)?;
            Ok(json!(ApiResponse::ok(value)))
        }
        // Community skills
        ("POST", "/community/install") | ("POST", "community/install") => unreachable!("handled before opening DB connection"),
        ("GET", "/community/installed") | ("GET", "community/installed") => {
            services::community_service::get_installed_skills(&conn)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        _ => Err(anyhow!("unsupported local Tauri API route: {} {}", method, path)),
    }
}

fn init_db(db_path: &Path) -> Result<()> {
    db::init_db(db_path)
}

fn parse_scope_payload(body: Option<Value>) -> Result<ScopePayload> {
    match body {
        None | Some(Value::Null) => Ok(ScopePayload::default()),
        Some(value) => serde_json::from_value(value).map_err(Into::into),
    }
}

fn list_workflows(conn: &Connection, query: PageQuery) -> Result<Value> {
    let page = query.page.unwrap_or(1).max(1);
    let size = query.size.unwrap_or(50).clamp(1, 200);
    let total: i64 = conn.query_row(
        "SELECT COUNT(id) FROM workflow_clusters",
        [],
        |row| row.get(0),
    )?;
    let mut stmt = conn.prepare(
        "SELECT id, name, description, frequency, source_agents, estimated_time_saved,
                can_generate_skill, skill_score, status, draft_body, sample_tasks,
                recommendation_source, confidence, reasoning, source_skills, similar_skills,
                review_score, review_summary, review_feedback
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
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "items": items, "total": total, "page": page, "size": size }))
}

fn list_skill_install_targets(conn: &Connection) -> Result<Value> {
    let mut targets = Vec::new();
    for source in list_sources(conn)? {
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

fn create_manual_skill_draft(conn: &Connection, skill_name: &str) -> Result<Value> {
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
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        )
        .optional()?
        .ok_or_else(|| anyhow!("Skill not found: {}", skill_name))?;

    let draft_name = format!("{}-manual-evolution", sanitize_file_name(&name));
    let description_text = description.clone().unwrap_or_else(|| format!("Manual evolution draft based on existing Skill {}", name));
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
    let sample_tasks = serde_json::to_string(&vec![format!("Manual evolution from existing Skill: {}", name)])?;
    let source_agents = serde_json::to_string(&agent_source.clone().map(|value| vec![value]).unwrap_or_default())?;
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
    get_workflow_by_id(conn, id)
}
fn get_workflow_by_id(conn: &Connection, id: i64) -> Result<Value> {
    let value = list_workflows(conn, PageQuery { page: Some(1), size: Some(200), ..Default::default() })?;
    value
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| items.iter().find(|item| item.get("id").and_then(Value::as_i64) == Some(id)).cloned())
        .ok_or_else(|| anyhow!("workflow draft not found: {}", id))
}

fn update_workflow_draft(conn: &Connection, id: i64, body: Option<Value>) -> Result<Value> {
    let body = body.unwrap_or(Value::Null);
    let draft_body = body
        .get("draft_body")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("draft_body is required"))?;
    let description = body.get("description").and_then(Value::as_str);
    conn.execute(
        "UPDATE workflow_clusters
         SET draft_body = ?2,
             description = COALESCE(?3, description),
             status = CASE WHEN status = 'installed' THEN status ELSE 'edited' END,
             updated_at = ?4
         WHERE id = ?1",
        params![id, draft_body, description, now_string()],
    )?;
    get_workflow_by_id(conn, id)
}

fn delete_workflow_draft(conn: &Connection, id: i64) -> Result<()> {
    let affected = conn.execute("DELETE FROM workflow_clusters WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(anyhow!("workflow draft not found: {}", id));
    }
    Ok(())
}

fn install_workflow_skill(conn: &Connection, workflow_id: i64, body: Option<Value>) -> Result<Value> {
    let agent_id = body
        .as_ref()
        .and_then(|value| value.get("agent_id"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("agent_id is required"))?;

    let (name, draft_body): (String, String) = conn
        .query_row(
            "SELECT name, draft_body FROM workflow_clusters WHERE id = ?1",
            [workflow_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                ))
            },
        )
        .optional()?
        .ok_or_else(|| anyhow!("workflow draft not found"))?;

    if draft_body.trim().is_empty() {
        return Err(anyhow!("workflow draft has no installable Skill content"));
    }

    let target = list_sources(conn)?
        .into_iter()
        .find(|source| source.agent_id == agent_id)
        .ok_or_else(|| anyhow!("agent not found: {}", agent_id))?;
    let install_dir = target
        .paths
        .get("skills_path")
        .and_then(Clone::clone)
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("{} has no skills_path configured", target.agent_name))?;

    fs::create_dir_all(&install_dir)?;
    let skill_dir = install_dir.join(sanitize_file_name(&name));
    fs::create_dir_all(&skill_dir)?;
    let skill_path = skill_dir.join("SKILL.md");
    fs::write(&skill_path, draft_body)?;

    conn.execute(
        "UPDATE workflow_clusters SET status = 'installed', updated_at = ?2 WHERE id = ?1",
        params![workflow_id, now_string()],
    )?;

    Ok(json!({
        "path": path_to_string(&skill_path),
        "agent_id": target.agent_id,
        "agent_name": target.agent_name
    }))
}
async fn install_community_skill(db_path: &Path, body: Option<Value>) -> Result<Value> {
    let body = body.unwrap_or(Value::Null);
    let skill_id = body
        .get("id")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .ok_or_else(|| anyhow!("community skill id is required"))?;
    let agent_id = body
        .get("agent_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("agent_id is required"))?
        .to_string();

    let (name, repo_full_name, repo_url, description, file_url): (String, String, String, Option<String>, Option<String>) = {
        let conn = open_conn(db_path)?;
        conn.query_row(
            "SELECT name, repo_full_name, repo_url, description, file_url FROM community_skills WHERE id = ?1",
            [skill_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?
        .ok_or_else(|| anyhow!("community skill not found"))?
    };

    let raw_url = community_skill_raw_url(&repo_full_name, file_url.as_deref())
        .ok_or_else(|| anyhow!("cannot resolve GitHub raw SKILL.md URL"))?;
    let skill_content = fetch_community_skill_markdown(&raw_url).await?;
    if skill_content.trim().is_empty() {
        return Err(anyhow!("downloaded SKILL.md is empty"));
    }

    let conn = open_conn(db_path)?;
    sync_source_configs(&conn)?;
    let target = list_sources(&conn)?
        .into_iter()
        .find(|source| source.agent_id == agent_id)
        .ok_or_else(|| anyhow!("agent not found: {}", agent_id))?;
    let install_dir = target
        .paths
        .get("skills_path")
        .and_then(Clone::clone)
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("{} has no skills_path configured", target.agent_name))?;

    fs::create_dir_all(&install_dir)?;
    let skill_dir = install_dir.join(sanitize_file_name(&name));
    fs::create_dir_all(&skill_dir)?;
    let skill_path = skill_dir.join("SKILL.md");
    fs::write(&skill_path, &skill_content)?;

    if let Some(info) = parse_skill_file(&skill_path, "community", &target.agent_id, None) {
        upsert_skills(&conn, &[info])?;
    }

    conn.execute(
        "UPDATE community_skills
         SET installed = 1, skill_md_content = ?2, fetched_at = ?3
         WHERE id = ?1",
        params![skill_id, skill_content, now_string()],
    )?;

    Ok(json!({
        "status": "installed",
        "id": skill_id,
        "name": name,
        "repo": repo_full_name,
        "repo_url": repo_url,
        "description": description,
        "source": "GitHub",
        "raw_url": raw_url,
        "path": path_to_string(&skill_path),
        "agent_id": target.agent_id,
        "agent_name": target.agent_name
    }))
}

fn community_skill_raw_url(repo_full_name: &str, file_url: Option<&str>) -> Option<String> {
    if let Some(file_url) = file_url {
        if let Some(rest) = file_url.strip_prefix("https://github.com/") {
            let parts = rest.split('/').collect::<Vec<_>>();
            if parts.len() >= 5 && parts[2] == "blob" {
                let repo = format!("{}/{}", parts[0], parts[1]);
                let branch = parts[3];
                let file_path = parts[4..].join("/");
                return Some(format!("https://raw.githubusercontent.com/{repo}/{branch}/{file_path}"));
            }
        }
        if file_url.starts_with("https://raw.githubusercontent.com/") {
            return Some(file_url.to_string());
        }
    }
    if repo_full_name.contains('/') {
        Some(format!("https://raw.githubusercontent.com/{repo_full_name}/main/SKILL.md"))
    } else {
        None
    }
}

async fn fetch_community_skill_markdown(raw_url: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let response = client
        .get(raw_url)
        .header(reqwest::header::USER_AGENT, "Self-Evolving-Skills")
        .send()
        .await
        .map_err(|err| anyhow!("failed to download community skill, check network: {}", err))?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!(
            "failed to download community skill: HTTP {}. The repository may not have SKILL.md at root, or GitHub is unreachable.",
            status.as_u16()
        ));
    }
    Ok(text)
}

fn import_skills(conn: &Connection, body: Option<Value>) -> Result<Value> {
    sync_source_configs(conn)?;
    let payload: ImportSkillPayload = serde_json::from_value(body.unwrap_or(Value::Null))?;
    let agent_id = payload.agent_id.trim();
    if agent_id.is_empty() {
        return Err(anyhow!("agent_id is required"));
    }
    if payload.content.trim().is_empty() {
        return Err(anyhow!("import content is required"));
    }

    let target = list_sources(conn)?
        .into_iter()
        .find(|source| source.agent_id == agent_id)
        .ok_or_else(|| anyhow!("agent not found: {}", agent_id))?;
    let install_dir = target
        .paths
        .get("skills_path")
        .and_then(Clone::clone)
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("{} has no skills_path configured", target.agent_name))?;

    fs::create_dir_all(&install_dir)?;
    if looks_like_zip(&payload.filename) {
        let encoded = payload
            .content_base64
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow!("zip import requires base64 binary content"))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .context("failed to decode zip base64 content")?;
        let (imported, skipped) = import_skills_from_zip(conn, &install_dir, &target.agent_id, &bytes)?;
        return Ok(json!({
            "agent_id": target.agent_id,
            "agent_name": target.agent_name,
            "skills_path": path_to_string(&install_dir),
            "imported": imported,
            "skipped": skipped,
            "count": imported.len()
        }));
    }
    let imported = if looks_like_json(&payload.filename, &payload.content) {
        import_skills_from_json(conn, &install_dir, &target.agent_id, &payload.content)?
    } else {
        let name = skill_name_from_markdown(&payload.content)
            .or_else(|| payload.filename.as_deref().map(file_stem_name))
            .unwrap_or_else(|| "imported-skill".to_string());
        let path = write_skill_markdown(&install_dir, &name, &payload.content)?;
        upsert_imported_skill(conn, &path, &target.agent_id)?;
        vec![json!({ "name": name, "path": path_to_string(&path) })]
    };

    Ok(json!({
        "agent_id": target.agent_id,
        "agent_name": target.agent_name,
        "skills_path": path_to_string(&install_dir),
        "imported": imported,
        "count": imported.len()
    }))
}
fn import_skills_from_json(conn: &Connection, install_dir: &Path, agent_id: &str, content: &str) -> Result<Vec<Value>> {
    let value: Value = serde_json::from_str(content).context("invalid JSON; cannot import Skills")?;
    let items = match value {
        Value::Array(items) => items,
        Value::Object(map) => vec![Value::Object(map)],
        _ => return Err(anyhow!("JSON must be a Skill object or an array of Skill objects")),
    };
    if items.is_empty() {
        return Err(anyhow!("JSON does not contain any importable Skill"));
    }

    let mut imported = Vec::new();
    for item in items {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow!("JSON Skill is missing required field: name"))?;
        let body = item
            .get("content")
            .or_else(|| item.get("body_text"))
            .or_else(|| item.get("body"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let markdown = if body.trim_start().starts_with("---") {
            body.to_string()
        } else {
            let description = item.get("description").and_then(Value::as_str).unwrap_or("");
            format!(
                "---\nname: {}\ndescription: {}\n---\n\n{}",
                yaml_scalar(name),
                yaml_scalar(description),
                body
            )
        };
        let path = write_skill_markdown(install_dir, name, &markdown)?;
        upsert_imported_skill(conn, &path, agent_id)?;
        imported.push(json!({ "name": name, "path": path_to_string(&path) }));
    }
    Ok(imported)
}
fn import_skills_from_zip(conn: &Connection, install_dir: &Path, agent_id: &str, bytes: &[u8]) -> Result<(Vec<Value>, Vec<Value>)> {
    const MAX_ZIP_BYTES: usize = 100 * 1024 * 1024;
    const MAX_FILES: usize = 2_000;
    const MAX_TOTAL_UNCOMPRESSED: u64 = 300 * 1024 * 1024;
    const MAX_SINGLE_FILE: u64 = 50 * 1024 * 1024;

    if bytes.len() > MAX_ZIP_BYTES {
        return Err(anyhow!("zip file is too large; maximum supported size is 100MB"));
    }

    let reader = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader).context("invalid or corrupted zip file")?;
    if archive.len() > MAX_FILES {
        return Err(anyhow!("zip contains too many files; maximum supported file count is {}", MAX_FILES));
    }

    let mut safe_entries = Vec::new();
    let mut skill_roots: HashSet<PathBuf> = HashSet::new();
    let mut skipped = Vec::new();
    let mut total_uncompressed = 0u64;

    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        let raw_name = file.name().to_string();
        if file.is_dir() {
            continue;
        }
        let Some(relative_path) = safe_zip_relative_path(&raw_name) else {
            skipped.push(json!({ "path": raw_name, "reason": "unsafe path skipped" }));
            continue;
        };
        let size = file.size();
        if size > MAX_SINGLE_FILE {
            skipped.push(json!({ "path": raw_name, "reason": "single file exceeds 50MB" }));
            continue;
        }
        total_uncompressed = total_uncompressed.saturating_add(size);
        if total_uncompressed > MAX_TOTAL_UNCOMPRESSED {
            return Err(anyhow!("zip uncompressed content is too large; maximum supported size is 300MB"));
        }
        if relative_path.file_name().and_then(|name| name.to_str()) == Some("SKILL.md") {
            if let Some(root) = skill_root_from_skill_md(&relative_path) {
                skill_roots.insert(root);
            }
        }
        safe_entries.push((index, raw_name, relative_path, size));
    }

    if skill_roots.is_empty() {
        return Err(anyhow!("zip does not contain any SKILL.md file"));
    }

    let mut imported_paths: HashMap<PathBuf, PathBuf> = HashMap::new();
    for (index, raw_name, relative_path, _) in safe_entries {
        let Some(root) = matching_skill_root(&relative_path, &skill_roots) else {
            skipped.push(json!({ "path": raw_name, "reason": "not under a directory containing SKILL.md" }));
            continue;
        };
        let target_root = install_dir.join(sanitize_file_name(&skill_root_name(root)));
        let inside_skill = if root.as_os_str().is_empty() {
            relative_path.as_path()
        } else {
            relative_path.strip_prefix(root).unwrap_or(&relative_path)
        };
        let target_path = target_root.join(inside_skill);
        ensure_child_path(install_dir, &target_path)?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = archive.by_index(index)?;
        let mut out = fs::File::create(&target_path)?;
        std::io::copy(&mut file, &mut out)?;
        if inside_skill.file_name().and_then(|name| name.to_str()) == Some("SKILL.md") {
            imported_paths.insert(root.clone(), target_path);
        }
    }

    let mut imported = Vec::new();
    for (root, skill_path) in imported_paths {
        upsert_imported_skill(conn, &skill_path, agent_id)?;
        imported.push(json!({
            "name": skill_root_name(&root),
            "path": path_to_string(&skill_path)
        }));
    }

    if imported.is_empty() {
        return Err(anyhow!("zip was extracted but no Skill was imported"));
    }
    Ok((imported, skipped))
}
fn safe_zip_relative_path(raw_name: &str) -> Option<PathBuf> {
    if raw_name.trim().is_empty() || raw_name.contains('\0') || raw_name.contains(':') {
        return None;
    }
    let normalized = raw_name.replace('\\', "/");
    let path = Path::new(&normalized);
    if path.is_absolute() {
        return None;
    }
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

fn skill_root_from_skill_md(path: &Path) -> Option<PathBuf> {
    path.parent().map(Path::to_path_buf)
}

fn matching_skill_root<'a>(path: &Path, roots: &'a HashSet<PathBuf>) -> Option<&'a PathBuf> {
    roots
        .iter()
        .filter(|root| root.as_os_str().is_empty() || path.starts_with(root.as_path()))
        .max_by_key(|root| root.components().count())
}

fn skill_root_name(root: &Path) -> String {
    root.file_name()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "imported-skill".to_string())
}

fn ensure_child_path(parent: &Path, child: &Path) -> Result<()> {
    let parent = parent.canonicalize().unwrap_or_else(|_| parent.to_path_buf());
    let child_parent = child
        .parent()
        .ok_or_else(|| anyhow!("invalid target path: {}", child.display()))?;
    let child_parent = child_parent.canonicalize().unwrap_or_else(|_| child_parent.to_path_buf());
    if !child_parent.starts_with(&parent) {
        return Err(anyhow!("zip path traversal rejected: {}", child.display()));
    }
    Ok(())
}

fn write_skill_markdown(install_dir: &Path, name: &str, content: &str) -> Result<PathBuf> {
    let skill_dir = install_dir.join(sanitize_file_name(name));
    fs::create_dir_all(&skill_dir)?;
    let skill_path = skill_dir.join("SKILL.md");
    fs::write(&skill_path, content)?;
    Ok(skill_path)
}

fn upsert_imported_skill(conn: &Connection, path: &Path, agent_id: &str) -> Result<()> {
    let skill = parse_skill_file(path, "user", agent_id, None)
        .ok_or_else(|| anyhow!("imported file is not a valid SKILL.md: {}", path.display()))?;
    upsert_skills(conn, &[skill])?;
    Ok(())
}

fn looks_like_zip(filename: &Option<String>) -> bool {
    filename
        .as_deref()
        .and_then(|name| Path::new(name).extension())
        .and_then(|value| value.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

fn looks_like_json(filename: &Option<String>, content: &str) -> bool {
    filename
        .as_deref()
        .and_then(|name| Path::new(name).extension())
        .and_then(|value| value.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("json"))
        .unwrap_or_else(|| content.trim_start().starts_with('{') || content.trim_start().starts_with('['))
}

fn skill_name_from_markdown(content: &str) -> Option<String> {
    let (_, parsed, _) = parse_frontmatter(content);
    parsed
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn file_stem_name(filename: &str) -> String {
    Path::new(filename)
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "imported-skill".to_string())
}

fn yaml_scalar(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn sanitize_file_name(name: &str) -> String {
    let cleaned = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else if ch.is_whitespace() {
                '-'
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

fn open_conn(db_path: &Path) -> Result<Connection> {
    Connection::open(db_path).with_context(|| format!("open sqlite db {}", db_path.display()))
}

fn agent_sources() -> Vec<AgentSource> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let config_dir = dirs::config_dir().unwrap_or_default();
    vec![
        AgentSource {
            id: "hermes",
            name: "Hermes",
            skills_path: Some(home.join(".hermes").join("skills")),
            agents_path: None,
            sessions_path: Some(home.join(".hermes").join("runs")),
            projects_path: None,
            plugins_path: None,
            memory_path: None,
            default_enabled: true,
        },
        AgentSource {
            id: "claude-code",
            name: "Claude Code",
            skills_path: Some(home.join(".claude").join("skills")),
            agents_path: Some(home.join(".claude").join("agents")),
            sessions_path: Some(home.join(".claude").join("sessions")),
            projects_path: Some(home.join(".claude").join("projects")),
            plugins_path: Some(
                home.join(".claude")
                    .join("plugins")
                    .join("cache"),
            ),
            memory_path: Some(home.join(".claude").join("projects")),
            default_enabled: true,
        },
        AgentSource {
            id: "codex",
            name: "Codex",
            skills_path: Some(home.join(".codex").join("skills")),
            agents_path: None,
            sessions_path: Some(home.join(".codex").join("sessions")),
            projects_path: Some(home.join(".codex").join("projects")),
            plugins_path: None,
            memory_path: None,
            default_enabled: true,
        },
        AgentSource {
            id: "vscode",
            name: "VSCode",
            skills_path: None,
            agents_path: None,
            sessions_path: Some(
                config_dir
                    .join("Code")
                    .join("User")
                    .join("globalStorage")
                    .join("saoudrizwan.claude-dev"),
            ),
            projects_path: None,
            plugins_path: None,
            memory_path: None,
            default_enabled: true,
        },
        AgentSource {
            id: "cursor",
            name: "Cursor",
            skills_path: None,
            agents_path: None,
            sessions_path: Some(config_dir.join("Cursor").join("User")),
            projects_path: None,
            plugins_path: None,
            memory_path: None,
            default_enabled: true,
        },
        AgentSource {
            id: "codebuddy",
            name: "CodeBuddy",
            skills_path: Some(home.join(".codebuddy").join("skills")),
            agents_path: None,
            sessions_path: Some(home.join(".codebuddy").join("sessions")),
            projects_path: Some(home.join(".codebuddy").join("projects")),
            plugins_path: None,
            memory_path: None,
            default_enabled: false,
        },
        AgentSource {
            id: "trae",
            name: "TRAE",
            skills_path: Some(home.join(".trae").join("skills")),
            agents_path: None,
            sessions_path: Some(home.join(".trae").join("sessions")),
            projects_path: None,
            plugins_path: None,
            memory_path: None,
            default_enabled: false,
        },
        AgentSource {
            id: "zeelinclaw",
            name: "ZeeLinClaw",
            skills_path: Some(home.join(".zeelinclaw").join("skills")),
            agents_path: None,
            sessions_path: Some(home.join(".zeelinclaw").join("sessions")),
            projects_path: None,
            plugins_path: None,
            memory_path: None,
            default_enabled: false,
        },
    ]
}

fn source_default_paths(src: &AgentSource) -> HashMap<String, Option<String>> {
    let mut paths = HashMap::from([
        ("skills_path".into(), src.skills_path.as_ref().map(path_to_string)),
        ("agents_path".into(), src.agents_path.as_ref().map(path_to_string)),
        ("sessions_path".into(), src.sessions_path.as_ref().map(path_to_string)),
        ("projects_path".into(), src.projects_path.as_ref().map(path_to_string)),
        ("plugins_path".into(), src.plugins_path.as_ref().map(path_to_string)),
        ("memory_path".into(), src.memory_path.as_ref().map(path_to_string)),
    ]);
    if src.id == "claude-code" {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let agent_skills_path = home.join(".claude").join(".agents").join("skills");
        paths.insert("extra_skills_path".into(), Some(path_to_string(&agent_skills_path)));
        paths.insert("extra_agents_path".into(), Some(path_to_string(&agent_skills_path)));
    }
    paths
}

#[allow(clippy::ptr_arg)]
fn path_to_string(path: &PathBuf) -> String {
    path.to_string_lossy().to_string()
}

pub(crate) fn sync_source_configs(conn: &Connection) -> Result<()> {
    for src in agent_sources() {
        let mut paths = source_default_paths(&src);
        if let Some(custom) = existing_custom_paths(conn, src.id)? {
            for (key, value) in custom {
                if value.is_some() {
                    paths.insert(key, value);
                }
            }
        }
        let (available, detected_path, record_count, last_activity) = detect_source_status(&paths)?;
        let exists: Option<i64> = conn
            .query_row(
                "SELECT id FROM source_configs WHERE agent_id = ?1",
                [src.id],
                |row| row.get(0),
            )
            .optional()?;

        if exists.is_some() {
            conn.execute(
                "UPDATE source_configs
                 SET agent_name = ?2, detected_path = ?3, is_available = ?4,
                     record_count = ?5, last_activity = ?6, updated_at = ?7
                 WHERE agent_id = ?1",
                params![
                    src.id,
                    src.name,
                    detected_path,
                    available as i64,
                    record_count,
                    last_activity,
                    now_string()
                ],
            )?;
        } else {
            conn.execute(
                "INSERT INTO source_configs
                 (agent_id, agent_name, detected_path, is_enabled, is_available, record_count, last_activity, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                params![
                    src.id,
                    src.name,
                    detected_path,
                    (src.default_enabled && available) as i64,
                    available as i64,
                    record_count,
                    last_activity,
                    now_string()
                ],
            )?;
        }
    }
    Ok(())
}

fn existing_custom_paths(conn: &Connection, agent_id: &str) -> Result<Option<HashMap<String, Option<String>>>> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT custom_paths FROM source_configs WHERE agent_id = ?1",
            [agent_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    Ok(raw.and_then(|text| serde_json::from_str(&text).ok()))
}

fn detect_source_status(
    paths: &HashMap<String, Option<String>>,
) -> Result<(bool, Option<String>, i64, Option<String>)> {
    let mut available = false;
    let mut detected_path = None;
    let mut count = 0;
    let mut latest: Option<std::time::SystemTime> = None;

    for key in ["sessions_path", "projects_path", "skills_path", "extra_skills_path", "agents_path", "extra_agents_path"] {
        if let Some(Some(path)) = paths.get(key) {
            let p = Path::new(path);
            if !p.exists() {
                continue;
            }
            available = true;
            detected_path.get_or_insert_with(|| path.clone());
            let (c, mtime) = count_records(p, key);
            count += c;
            if let Some(mtime) = mtime {
                latest = Some(latest.map_or(mtime, |cur| cur.max(mtime)));
            }
        }
    }

    Ok((available, detected_path, count, latest.map(format_system_time)))
}

fn count_records(path: &Path, key: &str) -> (i64, Option<std::time::SystemTime>) {
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
            "skills_path" | "extra_skills_path" => file_type.is_file() && entry.file_name() == "SKILL.md",
            "agents_path" | "extra_agents_path" => file_type.is_file() && is_agent_definition_file(entry.path()),
            "projects_path" => file_type.is_file() && entry.path().extension().and_then(|s| s.to_str()) == Some("jsonl"),
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
                    latest = Some(latest.map_or(modified, |cur: std::time::SystemTime| cur.max(modified)));
                }
            }
        }
    }
    (count, latest)
}

fn list_sources(conn: &Connection) -> Result<Vec<SourcePayload>> {
    let defaults = agent_sources()
        .into_iter()
        .map(|src| (src.id.to_string(), source_default_paths(&src)))
        .collect::<HashMap<_, _>>();

    let mut stmt = conn.prepare(
        "SELECT agent_id, agent_name, detected_path, custom_paths, is_enabled, is_available,
                record_count, last_activity, last_scan_at
         FROM source_configs ORDER BY agent_name",
    )?;
    let rows = stmt.query_map([], |row| {
        let agent_id: String = row.get(0)?;
        let mut paths = defaults.get(&agent_id).cloned().unwrap_or_default();
        let custom_paths: Option<String> = row.get(3)?;
        if let Some(custom_paths) = custom_paths {
            if let Ok(custom) = serde_json::from_str::<HashMap<String, Option<String>>>(&custom_paths) {
                for (key, value) in custom {
                    paths.insert(key, value);
                }
            }
        }
        Ok(SourcePayload {
            agent_id,
            agent_name: row.get(1)?,
            detected_path: row.get(2)?,
            paths,
            is_enabled: row.get::<_, i64>(4)? != 0,
            is_available: row.get::<_, i64>(5)? != 0,
            record_count: row.get(6)?,
            last_activity: row.get(7)?,
            last_scan_at: row.get(8)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

fn update_source(conn: &Connection, agent_id: &str, payload: SourceUpdate) -> Result<Value> {
    let mut current_custom = existing_custom_paths(conn, agent_id)?.unwrap_or_default();
    if let Some(paths) = payload.paths {
        current_custom = paths
            .into_iter()
            .filter(|(_, value)| value.as_ref().is_some_and(|v| !v.trim().is_empty()))
            .collect();
        conn.execute(
            "UPDATE source_configs SET custom_paths = ?2, updated_at = ?3 WHERE agent_id = ?1",
            params![agent_id, serde_json::to_string(&current_custom)?, now_string()],
        )?;
    }
    if let Some(path) = payload.detected_path {
        current_custom.insert("sessions_path".into(), Some(path.clone()));
        conn.execute(
            "UPDATE source_configs SET custom_paths = ?2, detected_path = ?3, updated_at = ?4 WHERE agent_id = ?1",
            params![agent_id, serde_json::to_string(&current_custom)?, path, now_string()],
        )?;
    }
    if let Some(is_enabled) = payload.is_enabled {
        conn.execute(
            "UPDATE source_configs SET is_enabled = ?2, updated_at = ?3 WHERE agent_id = ?1",
            params![agent_id, is_enabled as i64, now_string()],
        )?;
    }
    sync_source_configs(conn)?;
    let source = list_sources(conn)?
        .into_iter()
        .find(|s| s.agent_id == agent_id)
        .ok_or_else(|| anyhow!("source not found: {agent_id}"))?;
    Ok(serde_json::to_value(source)?)
}

fn reset_source(conn: &Connection, agent_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE source_configs
         SET custom_paths = NULL, is_available = 0, is_enabled = 0, record_count = 0,
             last_activity = NULL, updated_at = ?2
         WHERE agent_id = ?1",
        params![agent_id, now_string()],
    )?;
    Ok(())
}

pub(crate) fn enabled_source_paths(conn: &Connection) -> Result<Vec<SourcePaths>> {
    Ok(list_sources(conn)?
        .into_iter()
        .filter(|src| src.is_enabled)
        .map(|src| SourcePaths {
            agent_id: src.agent_id,
            agent_name: src.agent_name,
            paths: src.paths,
        })
        .collect())
}

fn run_full_scan(conn: &Connection, app: &AppHandle, agent_ids: Option<&[String]>) -> Result<Value> {
    sync_source_configs(conn)?;
    let selected = agent_ids.unwrap_or(&[]);
    let sources = enabled_source_paths(conn)?
        .into_iter()
        .filter(|source| selected.is_empty() || selected.iter().any(|id| id == &source.agent_id))
        .collect::<Vec<_>>();
    let started_at = now_string();
    let config_snapshot = serde_json::to_string(&sources.iter().map(|s| &s.agent_id).collect::<Vec<_>>())?;

    conn.execute(
        "INSERT INTO scan_jobs (scan_type, status, started_at, sources_scanned, config_snapshot, created_at)
         VALUES ('full', 'running', ?1, ?2, ?3, ?1)",
        params![started_at, sources.len() as i64, config_snapshot],
    )?;
    let job_id = conn.last_insert_rowid();
    emit_scan_progress(app, job_id, "running", 5, "Scanning local sources");

    let mut errors = Vec::new();
    let skills = scan_skills(&sources);
    emit_scan_progress(app, job_id, "running", 30, "Writing skills");
    let skills_found = upsert_skills(conn, &skills).unwrap_or_else(|e| {
        errors.push(format!("skills: {e}"));
        0
    });

    let agents = scan_agents(&sources);
    emit_scan_progress(app, job_id, "running", 52, "Writing agents");
    let agents_found = upsert_agents(conn, &agents).unwrap_or_else(|e| {
        errors.push(format!("agents: {e}"));
        0
    });

    let sessions = scan_sessions(&sources);
    emit_scan_progress(app, job_id, "running", 78, "Writing sessions");
    let sessions_found = upsert_sessions(conn, &sessions).unwrap_or_else(|e| {
        errors.push(format!("sessions: {e}"));
        0
    });

    let completed_at = now_string();
    for source in &sources {
        conn.execute(
            "UPDATE source_configs SET last_scan_at = ?2 WHERE agent_id = ?1",
            params![source.agent_id, completed_at],
        )?;
    }

    let errors_json = if errors.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&errors)?)
    };

    conn.execute(
        "UPDATE scan_jobs
         SET status = 'completed', completed_at = ?2, skills_found = ?3, agents_found = ?4,
             sessions_found = ?5, conversations_analyzed = 0, memories_found = 0,
             sources_scanned = ?6, errors = ?7
         WHERE id = ?1",
        params![
            job_id,
            completed_at,
            skills_found,
            agents_found,
            sessions_found,
            sources.len() as i64,
            errors_json
        ],
    )?;
    emit_scan_progress(app, job_id, "completed", 100, "Scan completed");

    Ok(json!({
        "id": job_id,
        "status": "completed",
        "scan_type": "full",
        "started_at": started_at,
        "completed_at": completed_at,
        "skills_found": skills_found,
        "agents_found": agents_found,
        "sessions_found": sessions_found,
        "conversations_analyzed": 0,
        "memories_found": 0,
        "sources_scanned": sources.len()
    }))
}

fn emit_scan_progress(app: &AppHandle, job_id: i64, status: &str, progress: i64, message: &str) {
    let _ = app.emit(
        "scan-progress",
        json!({
            "job_id": job_id,
            "status": status,
            "progress": progress,
            "message": message
        }),
    );
}

pub(crate) fn scan_skills(sources: &[SourcePaths]) -> Vec<SkillInfo> {
    let mut results = Vec::new();
    for source in sources {
        for key in ["skills_path", "extra_skills_path"] {
        if let Some(Some(path)) = source.paths.get(key) {
            let base = Path::new(path);
            if base.exists() {
                let max_depth = if key == "extra_skills_path" { 4 } else { 3 };
                for entry in WalkDir::new(base).min_depth(1).max_depth(max_depth).into_iter().flatten() {
                    if entry.file_type().is_file() && entry.file_name() == "SKILL.md" {
                        if let Some(info) = parse_skill_file(entry.path(), "user", &source.agent_id, None) {
                            results.push(info);
                        }
                    }
                }
            }
        }
        }
        if let Some(Some(path)) = source.paths.get("plugins_path") {
            let base = Path::new(path);
            if base.exists() {
                for entry in WalkDir::new(base).min_depth(1).max_depth(6).into_iter().flatten() {
                    if entry.file_type().is_file() && entry.file_name() == "SKILL.md" {
                        let plugin_name = entry
                            .path()
                            .ancestors()
                            .nth(3)
                            .and_then(|p| p.file_name())
                            .map(|s| s.to_string_lossy().to_string());
                        if let Some(info) = parse_skill_file(entry.path(), "plugin", &source.agent_id, plugin_name) {
                            results.push(info);
                        }
                    }
                }
            }
        }
    }
    results
}

pub(crate) fn scan_agents(sources: &[SourcePaths]) -> Vec<AgentInfo> {
    let mut results = Vec::new();
    for source in sources {
        for key in ["agents_path", "extra_agents_path"] {
        if let Some(Some(path)) = source.paths.get(key) {
            let base = Path::new(path);
            if !base.exists() {
                continue;
            }
            let max_depth = if key == "extra_agents_path" { 4 } else { 2 };
            for entry in WalkDir::new(base).min_depth(1).max_depth(max_depth).into_iter().flatten() {
                let path = entry.path();
                if entry.file_type().is_file() && is_agent_definition_file(path) {
                    if let Some(info) = parse_agent_file(path, &source.agent_id) {
                        results.push(info);
                    }
                }
            }
        }
        }
    }
    results
}

fn is_agent_definition_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|s| s.to_str()).map(str::to_ascii_lowercase).as_deref(),
        Some("md") | Some("yaml") | Some("yml")
    )
}

pub(crate) fn scan_sessions(sources: &[SourcePaths]) -> Vec<SessionInfo> {
    let mut results = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for source in sources {
        for key in ["sessions_path", "projects_path"] {
            let Some(Some(path)) = source.paths.get(key) else {
                continue;
            };
            let base = Path::new(path);
            if !base.exists() {
                continue;
            }
            let max_depth = if key == "projects_path" { 8 } else { 5 };
            for entry in WalkDir::new(base).max_depth(max_depth).into_iter().flatten() {
                if !entry.file_type().is_file() {
                    continue;
                }
                let ext = entry.path().extension().and_then(|s| s.to_str());
                let info = match ext {
                    Some("json") => parse_session_file(entry.path(), &source.agent_id, sources),
                    Some("jsonl") => parse_session_jsonl_file(entry.path(), &source.agent_id),
                    _ => None,
                };
                if let Some(info) = info {
                    if seen.insert(info.session_id.clone()) {
                        results.push(info);
                    }
                }
            }
        }
    }
    results
}

fn parse_skill_file(path: &Path, source_type: &str, agent_source: &str, plugin_name: Option<String>) -> Option<SkillInfo> {
    let text = fs::read_to_string(path).ok()?;
    let meta = fs::metadata(path).ok()?;
    let (yaml_raw, parsed, body) = parse_frontmatter(&text);
    let name = parsed
        .get("name")
        .and_then(Value::as_str)
        .map(String::from)
        .or_else(|| path.parent()?.file_name().map(|s| s.to_string_lossy().to_string()))?;
    let description = parsed.get("description").and_then(Value::as_str).map(String::from);
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

fn parse_agent_file(path: &Path, agent_source: &str) -> Option<AgentInfo> {
    let text = fs::read_to_string(path).ok()?;
    let meta = fs::metadata(path).ok()?;
    let is_yaml = matches!(
        path.extension().and_then(|s| s.to_str()).map(str::to_ascii_lowercase).as_deref(),
        Some("yaml") | Some("yml")
    );
    let (yaml_raw, parsed, body) = if is_yaml {
        let parsed = YamlLoader::load_from_str(&text)
            .ok()
            .and_then(|docs| docs.into_iter().next())
            .and_then(yaml_to_json)
            .unwrap_or_else(|| json!({}));
        (Some(text.clone()), parsed, text.clone())
    } else {
        let (yaml, map, body) = parse_frontmatter(&text);
        (yaml, serde_json::to_value(map).unwrap_or_else(|_| json!({})), body)
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
        model: parsed.get("model").or_else(|| parsed.get("default_model")).and_then(Value::as_str).map(String::from),
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

fn parse_session_file(path: &Path, agent_source: &str, sources: &[SourcePaths]) -> Option<SessionInfo> {
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
                let candidate = Path::new(projects_path).join(project_name).join(format!("{session_id}.jsonl"));
                if candidate.exists() {
                    jsonl_size = fs::metadata(&candidate).map(|m| m.len() as i64).unwrap_or(0);
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
        .and_then(|millis| Local.timestamp_millis_opt(millis).single())
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string());

    Some(SessionInfo {
        session_id,
        pid: data.get("pid").and_then(Value::as_i64),
        cwd,
        project_name,
        agent_source: agent_source.into(),
        entrypoint: data.get("entrypoint").and_then(Value::as_str).map(String::from),
        version: data.get("version").and_then(Value::as_str).map(String::from),
        kind: data.get("kind").and_then(Value::as_str).map(String::from),
        started_at,
        message_count,
        compressed_summary: jsonl_path
            .as_deref()
            .and_then(|path| compress_session_jsonl(Path::new(path), first_prompt.as_deref())),
        first_prompt,
        jsonl_path,
        jsonl_size,
    })
}

fn parse_session_jsonl_file(path: &Path, agent_source: &str) -> Option<SessionInfo> {
    let meta = fs::metadata(path).ok()?;
    let session_id = path.file_stem()?.to_string_lossy().to_string();
    if session_id.is_empty() {
        return None;
    }
    let first_value = first_jsonl_value(path);
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
    })
}

fn first_jsonl_value(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    text.lines()
        .find_map(|line| serde_json::from_str::<Value>(line).ok())
}

fn extract_started_at(value: &Value) -> Option<String> {
    if let Some(millis) = value
        .get("startedAt")
        .or_else(|| value.get("timestamp"))
        .and_then(Value::as_i64)
    {
        return Local
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

fn extract_first_user_prompt(path: &Path) -> Option<String> {
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

fn compress_session_jsonl(path: &Path, first_prompt: Option<&str>) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let mut user_goals = Vec::new();
    let mut tool_names = std::collections::HashSet::new();
    let mut errors = Vec::new();
    let mut outcomes = Vec::new();

    for line in text.lines().take(800) {
        let value: Value = serde_json::from_str(line).ok()?;
        if is_user_message(&value) {
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

fn is_user_message(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("user")
        || value.get("role").and_then(Value::as_str) == Some("user")
        || value
            .get("message")
            .and_then(|message| message.get("role"))
            .and_then(Value::as_str)
            == Some("user")
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

fn parse_frontmatter(text: &str) -> (Option<String>, HashMap<String, Value>, String) {
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

fn yaml_to_json(yaml: yaml_rust2::Yaml) -> Option<Value> {
    use yaml_rust2::Yaml;
    match yaml {
        Yaml::Real(s) => s.parse::<f64>().ok().map(Value::from),
        Yaml::Integer(i) => Some(Value::from(i)),
        Yaml::String(s) => Some(Value::String(s)),
        Yaml::Boolean(b) => Some(Value::Bool(b)),
        Yaml::Array(values) => Some(Value::Array(values.into_iter().filter_map(yaml_to_json).collect())),
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

pub(crate) fn upsert_skills(conn: &Connection, skills: &[SkillInfo]) -> Result<i64> {
    for skill in skills {
        conn.execute(
            "INSERT INTO skills
             (name, description, origin, source_type, agent_source, plugin_name, file_path, yaml_raw,
              body_text, body_size, line_count, file_mtime, file_size, file_hash, category, category_tags, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?17)
             ON CONFLICT(name, source_type) DO UPDATE SET
              description = excluded.description,
              origin = excluded.origin,
              agent_source = excluded.agent_source,
              plugin_name = excluded.plugin_name,
              file_path = excluded.file_path,
              yaml_raw = excluded.yaml_raw,
              body_text = excluded.body_text,
              body_size = excluded.body_size,
              line_count = excluded.line_count,
              file_mtime = excluded.file_mtime,
              file_size = excluded.file_size,
              file_hash = excluded.file_hash,
              category = excluded.category,
              category_tags = excluded.category_tags,
              updated_at = excluded.updated_at",
            params![
                skill.name,
                skill.description,
                skill.origin,
                skill.source_type,
                skill.agent_source,
                skill.plugin_name,
                skill.file_path,
                skill.yaml_raw,
                skill.body_text,
                skill.body_size,
                skill.line_count,
                skill.file_mtime,
                skill.file_size,
                skill.file_hash,
                skill.category,
                serde_json::to_string(&skill.category_tags)?,
                now_string()
            ],
        )?;
    }
    Ok(skills.len() as i64)
}

pub(crate) fn upsert_agents(conn: &Connection, agents: &[AgentInfo]) -> Result<i64> {
    for agent in agents {
        conn.execute(
            "INSERT INTO agents
             (name, description, tools, model, agent_source, file_path, yaml_raw, body_text,
              body_size, line_count, file_mtime, file_size, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)
             ON CONFLICT(name) DO UPDATE SET
              description = excluded.description,
              tools = excluded.tools,
              model = excluded.model,
              agent_source = excluded.agent_source,
              file_path = excluded.file_path,
              yaml_raw = excluded.yaml_raw,
              body_text = excluded.body_text,
              body_size = excluded.body_size,
              line_count = excluded.line_count,
              file_mtime = excluded.file_mtime,
              file_size = excluded.file_size,
              updated_at = excluded.updated_at",
            params![
                agent.name,
                agent.description,
                agent.tools.as_ref().map(Value::to_string),
                agent.model,
                agent.agent_source,
                agent.file_path,
                agent.yaml_raw,
                agent.body_text,
                agent.body_size,
                agent.line_count,
                agent.file_mtime,
                agent.file_size,
                now_string()
            ],
        )?;
    }
    Ok(agents.len() as i64)
}

pub(crate) fn upsert_sessions(conn: &Connection, sessions: &[SessionInfo]) -> Result<i64> {
    for session in sessions {
        conn.execute(
            "INSERT INTO sessions
             (session_id, pid, cwd, project_name, agent_source, entrypoint, version, kind,
              started_at, message_count, first_prompt, compressed_summary, jsonl_path, jsonl_size, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)
             ON CONFLICT(session_id) DO UPDATE SET
              pid = excluded.pid,
              cwd = excluded.cwd,
              project_name = excluded.project_name,
              agent_source = excluded.agent_source,
              entrypoint = excluded.entrypoint,
              version = excluded.version,
              kind = excluded.kind,
              started_at = excluded.started_at,
              message_count = excluded.message_count,
              first_prompt = excluded.first_prompt,
              compressed_summary = excluded.compressed_summary,
              jsonl_path = excluded.jsonl_path,
              jsonl_size = excluded.jsonl_size,
              updated_at = excluded.updated_at",
            params![
                session.session_id,
                session.pid,
                session.cwd,
                session.project_name,
                session.agent_source,
                session.entrypoint,
                session.version,
                session.kind,
                session.started_at,
                session.message_count,
                session.first_prompt,
                session.compressed_summary,
                session.jsonl_path,
                session.jsonl_size,
                now_string()
            ],
        )?;
    }
    refresh_skill_usage_from_sessions(conn)?;
    Ok(sessions.len() as i64)
}

fn refresh_skill_usage_from_sessions(conn: &Connection) -> Result<i64> {
    let skills = load_skill_usage_candidates(conn)?;
    conn.execute("DELETE FROM skill_usage", [])?;
    conn.execute("UPDATE skills SET usage_count = 0, session_count = 0", [])?;
    if skills.is_empty() {
        return Ok(0);
    }

    let mut stmt = conn.prepare(
        "SELECT session_id, agent_source, first_prompt, compressed_summary, jsonl_path, started_at
         FROM sessions",
    )?;
    let sessions = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut inserted = 0;
    for (session_id, agent_source, first_prompt, compressed_summary, jsonl_path, started_at) in sessions {
        let mut haystack = String::new();
        if let Some(value) = first_prompt {
            haystack.push_str(&value);
            haystack.push('\n');
        }
        if let Some(value) = compressed_summary {
            haystack.push_str(&value);
            haystack.push('\n');
        }
        if let Some(path) = &jsonl_path {
            if let Ok(text) = fs::read_to_string(path) {
                haystack.push_str(&truncate_chars(&text, 200_000));
            }
        }
        if haystack.trim().is_empty() {
            continue;
        }

        for skill in &skills {
            if let Some(session_agent) = &agent_source {
                if let Some(skill_agent) = &skill.agent_source {
                    if session_agent != skill_agent {
                        continue;
                    }
                }
            }
            let mention_count = count_skill_mentions(&haystack, &skill.name);
            if mention_count == 0 {
                continue;
            }
            conn.execute(
                "INSERT INTO skill_usage
                 (skill_name, skill_source_type, session_id, usage_type, mention_count, first_used_at, jsonl_path, created_at)
                 VALUES (?1, ?2, ?3, 'session-mention', ?4, ?5, ?6, ?7)",
                params![
                    skill.name,
                    skill.source_type,
                    session_id,
                    mention_count as i64,
                    started_at,
                    jsonl_path,
                    now_string()
                ],
            )?;
            inserted += 1;
        }
    }

    conn.execute(
        "UPDATE skills
         SET usage_count = COALESCE((
             SELECT SUM(mention_count) FROM skill_usage
             WHERE skill_usage.skill_name = skills.name
               AND skill_usage.skill_source_type = skills.source_type
         ), 0),
         session_count = COALESCE((
             SELECT COUNT(DISTINCT session_id) FROM skill_usage
             WHERE skill_usage.skill_name = skills.name
               AND skill_usage.skill_source_type = skills.source_type
         ), 0)",
        [],
    )?;
    Ok(inserted)
}

#[derive(Debug)]
struct SkillUsageCandidate {
    name: String,
    source_type: String,
    agent_source: Option<String>,
}

fn load_skill_usage_candidates(conn: &Connection) -> Result<Vec<SkillUsageCandidate>> {
    let mut stmt = conn.prepare(
        "SELECT name, source_type, agent_source FROM skills
         WHERE name IS NOT NULL AND name != ''
         ORDER BY LENGTH(name) DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(SkillUsageCandidate {
            name: row.get(0)?,
            source_type: row.get(1)?,
            agent_source: row.get(2)?,
        })
    })?;
    let items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(items)
}

fn count_skill_mentions(text: &str, skill_name: &str) -> usize {
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

fn list_scan_history(conn: &Connection, query: PageQuery) -> Result<Value> {
    let page = query.page.unwrap_or(1).max(1);
    let size = query.size.unwrap_or(20).clamp(1, 100);
    let total: i64 = conn.query_row("SELECT COUNT(id) FROM scan_jobs", [], |row| row.get(0))?;
    let mut stmt = conn.prepare(
        "SELECT id, scan_type, status, started_at, completed_at, skills_found, agents_found,
                sessions_found, conversations_analyzed, memories_found, sources_scanned, errors
         FROM scan_jobs ORDER BY started_at DESC LIMIT ?1 OFFSET ?2",
    )?;
    let items = stmt
        .query_map(params![size, (page - 1) * size], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "scan_type": row.get::<_, String>(1)?,
                "status": row.get::<_, String>(2)?,
                "started_at": row.get::<_, Option<String>>(3)?,
                "completed_at": row.get::<_, Option<String>>(4)?,
                "skills_found": row.get::<_, i64>(5)?,
                "agents_found": row.get::<_, i64>(6)?,
                "sessions_found": row.get::<_, i64>(7)?,
                "conversations_analyzed": row.get::<_, i64>(8)?,
                "memories_found": row.get::<_, i64>(9)?,
                "sources_scanned": row.get::<_, i64>(10)?,
                "errors": row.get::<_, Option<String>>(11)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "items": items, "total": total, "page": page, "size": size }))
}

fn list_skills(conn: &Connection, query: PageQuery) -> Result<Value> {
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

fn list_categories(conn: &Connection) -> Result<Value> {
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

fn get_skill(conn: &Connection, name: &str) -> Result<Value> {
    conn.query_row(
        "SELECT id, name, description, category, category_tags, source_type, plugin_name, origin,
                file_path, yaml_raw, body_text, body_size, line_count, file_mtime, file_size,
                usage_count, session_count, created_at, updated_at
         FROM skills WHERE name = ?1 LIMIT 1",
        [name],
        |row| {
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

fn update_skill(conn: &Connection, name: &str, body: &Value) -> Result<Value> {
    let existing = conn.query_row(
        "SELECT file_path, source_type FROM skills WHERE name = ?1 LIMIT 1",
        [name],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    ).optional()?.ok_or_else(|| anyhow!("Skill not found"))?;

    let new_name = body.get("name").and_then(|v| v.as_str()).unwrap_or(name);
    let new_desc = body.get("description").and_then(|v| v.as_str());
    let new_category = body.get("category").and_then(|v| v.as_str());

    conn.execute(
        "UPDATE skills SET name = ?1, description = ?2, category = ?3, updated_at = CURRENT_TIMESTAMP WHERE name = ?4",
        params![new_name, new_desc, new_category, name],
    )?;

    // Rename file on disk if name changed
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

    get_skill(conn, new_name)
}

fn delete_skill(conn: &Connection, name: &str) -> Result<Value> {
    let row = conn.query_row(
        "SELECT file_path, source_type FROM skills WHERE name = ?1 LIMIT 1",
        [name],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    ).optional()?.ok_or_else(|| anyhow!("Skill not found"))?;

    let file_path = std::path::Path::new(&row.0);
    if file_path.exists() {
        // On Windows, clear read-only attribute before attempting deletion
        if let Ok(metadata) = std::fs::metadata(file_path) {
            let mut perms = metadata.permissions();
            if perms.readonly() {
                perms.set_readonly(false);
                std::fs::set_permissions(file_path, perms)
                    .with_context(|| format!("无法修改文件权限: {}", file_path.display()))?;
            }
        }
        std::fs::remove_file(file_path)
            .with_context(|| format!("无法删除文件 {}，可能被其他进程占用", file_path.display()))?;
    }

    // Try to remove the parent skill directory if empty
    if let Some(parent) = file_path.parent() {
        let _ = std::fs::remove_dir(parent);
    }

    conn.execute("DELETE FROM skills WHERE name = ?1", [name])?;
    Ok(json!({ "deleted": true, "name": name, "file_path": row.0 }))
}

fn list_agents(conn: &Connection, query: PageQuery) -> Result<Value> {
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
        .query_map(params![query.agent_source, search, size, (page - 1) * size], |row| {
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
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "items": items, "total": total, "page": page, "size": size }))
}

fn get_agent(conn: &Connection, name: &str) -> Result<Value> {
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

fn stats_summary(conn: &Connection) -> Result<Value> {
    Ok(json!({
        "total_skills": count_table(conn, "skills")?,
        "total_agents": count_table(conn, "agents")?,
        "total_sessions": count_table(conn, "sessions")?,
        "total_memories": count_table(conn, "memories")?,
        "total_skill_usages": count_table(conn, "skill_usage")?,
        "enabled_sources": count_where(conn, "source_configs", "is_enabled = 1")?,
        "available_sources": count_where(conn, "source_configs", "is_available = 1")?,
    }))
}

fn top_skills(conn: &Connection, limit: i64) -> Result<Value> {
    let mut stmt = conn.prepare(
        "SELECT name, category, agent_source, source_type, description, usage_count, session_count
         FROM skills ORDER BY usage_count DESC, name ASC LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit.clamp(1, 100)], |row| {
            Ok(json!({
                "name": row.get::<_, String>(0)?,
                "category": row.get::<_, Option<String>>(1)?,
                "agent_source": row.get::<_, Option<String>>(2)?,
                "source_type": row.get::<_, String>(3)?,
                "description": row.get::<_, Option<String>>(4)?,
                "usage_count": row.get::<_, i64>(5)?,
                "session_count": row.get::<_, i64>(6)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!(items))
}

fn skills_by_category(conn: &Connection) -> Result<Value> {
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

fn list_sessions(conn: &Connection, query: PageQuery) -> Result<Value> {
    let page = query.page.unwrap_or(1).max(1);
    let size = query.size.unwrap_or(50).clamp(1, 200);
    let total: i64 = conn.query_row(
        "SELECT COUNT(id) FROM sessions WHERE (?1 IS NULL OR project_name = ?1)",
        params![query.project],
        |row| row.get(0),
    )?;
    let mut stmt = conn.prepare(
        "SELECT id, session_id, project_name, entrypoint, version, started_at, message_count, cwd,
                agent_source, first_prompt, compressed_summary, jsonl_path, jsonl_size
         FROM sessions WHERE (?1 IS NULL OR project_name = ?1)
         ORDER BY started_at DESC LIMIT ?2 OFFSET ?3",
    )?;
    let items = stmt
        .query_map(params![query.project, size, (page - 1) * size], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "session_id": row.get::<_, String>(1)?,
                "project_name": row.get::<_, Option<String>>(2)?,
                "entrypoint": row.get::<_, Option<String>>(3)?,
                "version": row.get::<_, Option<String>>(4)?,
                "started_at": row.get::<_, Option<String>>(5)?,
                "message_count": row.get::<_, i64>(6)?,
                "cwd": row.get::<_, Option<String>>(7)?,
                "agent_source": row.get::<_, Option<String>>(8)?,
                "first_prompt": row.get::<_, Option<String>>(9)?,
                "compressed_summary": row.get::<_, Option<String>>(10)?,
                "jsonl_path": row.get::<_, Option<String>>(11)?,
                "jsonl_size": row.get::<_, i64>(12)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "items": items, "total": total, "page": page, "size": size }))
}

fn get_session(conn: &Connection, session_id: &str) -> Result<Value> {
    conn.query_row(
        "SELECT id, session_id, pid, cwd, project_name, agent_source, entrypoint, version, kind,
                started_at, message_count, first_prompt, compressed_summary, jsonl_path, jsonl_size
         FROM sessions WHERE session_id = ?1",
        [session_id],
        |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "session_id": row.get::<_, String>(1)?,
                "pid": row.get::<_, Option<i64>>(2)?,
                "cwd": row.get::<_, Option<String>>(3)?,
                "project_name": row.get::<_, Option<String>>(4)?,
                "agent_source": row.get::<_, Option<String>>(5)?,
                "entrypoint": row.get::<_, Option<String>>(6)?,
                "version": row.get::<_, Option<String>>(7)?,
                "kind": row.get::<_, Option<String>>(8)?,
                "started_at": row.get::<_, Option<String>>(9)?,
                "message_count": row.get::<_, i64>(10)?,
                "first_prompt": row.get::<_, Option<String>>(11)?,
                "compressed_summary": row.get::<_, Option<String>>(12)?,
                "jsonl_path": row.get::<_, Option<String>>(13)?,
                "jsonl_size": row.get::<_, i64>(14)?,
                "skill_usage": [],
            }))
        },
    )
    .optional()?
    .ok_or_else(|| anyhow!("Session not found"))
}

fn system_info(conn: &Connection) -> Result<Value> {
    let total_skills: i64 = count_table(conn, "skills")?;
    let total_agents: i64 = count_table(conn, "agents")?;
    let total_sessions: i64 = count_table(conn, "sessions")?;
    let first_launch = total_skills == 0 && total_agents == 0 && total_sessions == 0;
    Ok(json!({
        "runtime": "tauri-rust",
        "database": "sqlite",
        "total_skills": total_skills,
        "total_agents": total_agents,
        "total_sessions": total_sessions,
        "first_launch": first_launch,
        "sources": list_sources(conn)?,
    }))
}

fn get_config(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_config WHERE key = ?1",
        [key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(Into::into)
}

fn get_config_bool(conn: &Connection, key: &str, default: bool) -> Result<bool> {
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

fn save_llm_config(conn: &Connection, body: Option<Value>) -> Result<Value> {
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
        .unwrap_or("gpt-5.2")
        .trim();

    if base_url.is_empty() {
        return Err(anyhow!("base_url is required"));
    }
    if model.is_empty() {
        return Err(anyhow!("model is required"));
    }

    set_config(conn, "llm_enabled", &enabled.to_string())?;
    set_config(conn, "llm_provider", provider)?;
    set_config(conn, "llm_base_url", base_url)?;
    set_config(conn, "llm_model", model)?;

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

async fn test_llm_connection(body: Option<Value>) -> Result<Value> {
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

    if base_url.is_empty() {
        return Err(anyhow!("base_url is required"));
    }
    if model.is_empty() {
        return Err(anyhow!("model is required"));
    }
    if api_key.is_empty() {
        return Err(anyhow!("api_key is required"));
    }

    let url = format!("{}/chat/completions", base_url);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let response = client
        .post(url)
        .bearer_auth(api_key)
        .json(&json!({
            "model": model,
            "messages": [
                { "role": "user", "content": "ping" }
            ],
            "max_tokens": 1,
            "temperature": 0
        }))
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if status.is_success() {
        Ok(json!({ "message": "大模型连接测试成功" }))
    } else {
        let message = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|value| {
                value
                    .get("error")
                    .and_then(|error| error.get("message").or(Some(error)))
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .unwrap_or_else(|| text.chars().take(300).collect::<String>());
        Err(anyhow!("大模型连接测试失败：HTTP {} {}", status.as_u16(), message))
    }
}

fn export_skills(conn: &Connection, query: PageQuery) -> Result<Value> {
    let format = query.format.unwrap_or_else(|| "json".to_string());
    if format == "csv" {
        let mut stmt = conn.prepare(
            "SELECT name, description, category, source_type, plugin_name, origin,
                    usage_count, session_count, file_path, agent_source
             FROM skills WHERE (?1 IS NULL OR agent_source = ?1) ORDER BY name ASC",
        )?;
        let mut csv = String::from(
            "name,description,category,source_type,agent_source,plugin_name,origin,usage_count,session_count,file_path\n",
        );
        let rows = stmt.query_map([query.agent_source], |row| {
            Ok(vec![
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(9)?.unwrap_or_default(),
                row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                row.get::<_, i64>(6)?.to_string(),
                row.get::<_, i64>(7)?.to_string(),
                row.get::<_, String>(8)?,
            ])
        })?;
        for row in rows {
            csv.push_str(
                &row?
                    .into_iter()
                    .map(csv_escape)
                    .collect::<Vec<_>>()
                    .join(","),
            );
            csv.push('\n');
        }
        return Ok(json!({ "content": csv, "filename": "skills.csv", "content_type": "text/csv" }));
    }

    let mut stmt = conn.prepare(
        "SELECT name, description, category, source_type, agent_source, plugin_name, origin,
                usage_count, session_count, file_path, body_text
         FROM skills WHERE (?1 IS NULL OR agent_source = ?1) ORDER BY name ASC",
    )?;
    let items = stmt
        .query_map([query.agent_source], |row| {
            Ok(json!({
                "name": row.get::<_, String>(0)?,
                "description": row.get::<_, Option<String>>(1)?,
                "category": row.get::<_, Option<String>>(2)?,
                "source_type": row.get::<_, String>(3)?,
                "agent_source": row.get::<_, Option<String>>(4)?,
                "plugin_name": row.get::<_, Option<String>>(5)?,
                "origin": row.get::<_, Option<String>>(6)?,
                "usage_count": row.get::<_, i64>(7)?,
                "session_count": row.get::<_, i64>(8)?,
                "file_path": row.get::<_, String>(9)?,
                "body_text": row.get::<_, Option<String>>(10)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({
        "content": serde_json::to_string_pretty(&items)?,
        "filename": "skills.json",
        "content_type": "application/json"
    }))
}

fn export_agents(conn: &Connection) -> Result<Value> {
    let mut stmt = conn.prepare(
        "SELECT name, description, tools, model, file_path FROM agents ORDER BY name ASC",
    )?;
    let items = stmt
        .query_map([], |row| {
            let tools: Option<String> = row.get(2)?;
            Ok(json!({
                "name": row.get::<_, String>(0)?,
                "description": row.get::<_, Option<String>>(1)?,
                "tools": tools.and_then(|v| serde_json::from_str::<Value>(&v).ok()),
                "model": row.get::<_, Option<String>>(3)?,
                "file_path": row.get::<_, String>(4)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({
        "content": serde_json::to_string_pretty(&items)?,
        "filename": "agents.json",
        "content_type": "application/json"
    }))
}

fn list_admin_users(conn: &Connection) -> Result<Value> {
    let mut stmt = conn.prepare(
        "SELECT id, username, is_active, created_at FROM admin_users ORDER BY id ASC",
    )?;
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

fn create_admin_user(conn: &Connection, body: Option<Value>) -> Result<Value> {
    let body = body.unwrap_or(Value::Null);
    let username = body
        .get("username")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| anyhow!("username is required"))?;
    let password_hash = body
        .get("password")
        .and_then(Value::as_str)
        .map(local_password_hash)
        .unwrap_or_default();
    conn.execute(
        "INSERT INTO admin_users (username, password_hash, is_active, created_at)
         VALUES (?1, ?2, 1, ?3)",
        params![username, password_hash, now_string()],
    )?;
    Ok(json!({ "id": conn.last_insert_rowid(), "username": username, "is_active": true }))
}

fn update_admin_user(conn: &Connection, id: i64, body: Option<Value>) -> Result<Value> {
    let body = body.unwrap_or(Value::Null);
    if let Some(username) = body.get("username").and_then(Value::as_str).filter(|v| !v.trim().is_empty()) {
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
    if let Some(password) = body.get("password").and_then(Value::as_str).filter(|v| !v.is_empty()) {
        conn.execute(
            "UPDATE admin_users SET password_hash = ?2 WHERE id = ?1",
            params![id, local_password_hash(password)],
        )?;
    }
    Ok(json!({ "id": id }))
}

fn delete_admin_user(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM admin_users WHERE id = ?1 AND id != 1", [id])?;
    Ok(())
}

fn local_password_hash(password: &str) -> String {
    hash_bytes(password.as_bytes())
}

fn csv_escape(value: String) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}

fn count_table(conn: &Connection, table: &str) -> Result<i64> {
    conn.query_row(&format!("SELECT COUNT(id) FROM {table}"), [], |row| row.get(0))
        .map_err(Into::into)
}

fn count_where(conn: &Connection, table: &str, clause: &str) -> Result<i64> {
    conn.query_row(&format!("SELECT COUNT(id) FROM {table} WHERE {clause}"), [], |row| row.get(0))
        .map_err(Into::into)
}

fn derive_category(name: &str) -> String {
    let lower = name.to_lowercase();
    let prefix = lower.split('-').next().unwrap_or(&lower);
    let groups: [(&str, &[&str]); 10] = [
        ("python", &["python"]),
        ("golang", &["golang", "go"]),
        ("java", &["java", "kotlin", "springboot", "jpa", "android", "compose", "gradle"]),
        ("cpp", &["cpp"]),
        ("rust", &["rust"]),
        ("frontend", &["frontend", "vue", "react", "nextjs", "nuxt", "typescript", "javascript", "flutter", "swiftui", "swift"]),
        ("document", &["document", "docx", "pptx", "xlsx", "pdf", "internal-comms"]),
        ("workflow", &["tdd", "e2e", "verification", "code-review", "debugging", "build", "refactor"]),
        ("meta", &["colleague", "continuous-learning", "skill", "configure", "eval", "harness", "project", "rules", "sessions", "strategic", "brainstorm"]),
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
    let haystack = format!("{} {}", name.to_lowercase(), description.unwrap_or("").to_lowercase());
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

fn derive_project_name(cwd: Option<&str>, file_path: &Path) -> Option<String> {
    if let Some(cwd) = cwd {
        return Some(
            cwd.replace(":\\", "--")
                .replace(['\\', '/', ' '], "-"),
        );
    }
    let path = file_path.to_string_lossy().replace('\\', "/");
    path.split("/projects/")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .map(String::from)
}

fn count_jsonl_lines(path: &Path) -> i64 {
    fs::read_to_string(path)
        .map(|text| text.lines().filter(|line| !line.trim().is_empty()).count() as i64)
        .unwrap_or(0)
}

fn truncate_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn system_time_secs(time: std::time::SystemTime) -> f64 {
    time.duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or_default()
}

fn format_system_time(time: std::time::SystemTime) -> String {
    let dt: DateTime<Utc> = time.into();
    dt.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string()
}

fn now_string() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn url_decode(value: &str) -> String {
    let mut out = String::new();
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte as char);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}






