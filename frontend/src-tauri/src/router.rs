use std::{fs, path::Path};

use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::{
    count_table, count_where, now_string, open_conn, parse_scope_payload, path_to_string,
    sanitize_file_name, url_decode, ApiResponse, PageQuery, SourceUpdate,
};
use services::admin_service::{
    create_admin_user, delete_admin_user, get_config, get_config_bool, list_admin_users,
    save_llm_config, system_info, test_llm_connection, update_admin_user,
};
use services::agents_crud::{
    create_agent_evolution_draft, delete_agent, get_agent, list_agents, update_agent,
};
use services::import_export::{export_agents, export_skills, import_skills};
use services::scan::{
    list_sources, reset_source, run_full_scan, sync_source_configs, update_source,
};
use services::skills_crud::{
    create_manual_skill_draft, delete_skill, get_skill, list_categories,
    list_skill_install_targets, list_skill_variants, list_skills, list_workflows, update_skill,
};
use services::workflow_crud::{
    delete_workflow_with_uninstall, install_workflow_skill, update_workflow_draft,
};

use super::services;

pub(crate) use dispatch_api::dispatch_api;

mod dispatch_api {
    use super::*;

    pub(crate) async fn dispatch_api(
        state: &crate::AppState,
        app: &AppHandle,
        method: &str,
        path: &str,
        params_value: Option<Value>,
        body: Option<Value>,
    ) -> Result<Value> {
        let clean_path = path.trim_start_matches("/api").trim_end_matches('/');
        let query: PageQuery =
            serde_json::from_value(params_value.unwrap_or(Value::Null)).unwrap_or_default();

        // LLM test: pre-fill stored API key if not provided in request
        if matches!(
            (method, clean_path),
            ("POST", "/admin/config/llm/test") | ("POST", "admin/config/llm/test")
        ) {
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
            return test_llm_connection(Some(body))
                .await
                .map(|value| json!(ApiResponse::ok(value)));
        }

        // Scan requires the lock to be acquired before opening the connection
        if matches!((method, clean_path), ("POST", "/scan") | ("POST", "scan")) {
            let _guard = state.scan_lock.lock().await;
            let conn = open_conn(&state.db_path)?;
            let scope = parse_scope_payload(body)?;
            return run_full_scan(&conn, app, scope.agent_ids.as_deref())
                .map(|value| json!(ApiResponse::ok(value)));
        }

        // Disk usage walks can take a long time. Keep them out of the DB-backed
        // routing path so system monitor/config APIs continue to respond.
        if matches!(
            (method, clean_path),
            ("POST", "/system/disk-usage/scan") | ("POST", "system/disk-usage/scan")
        ) {
            if let Ok(_guard) = state.disk_usage_lock.try_lock() {
                let scan_body = body.clone();
                let cache_ref = state.disk_usage_cache.clone();
                let app_handle = app.clone();
                return tokio::task::spawn_blocking(move || {
                    let (cache, value) = services::system_service::build_disk_usage_cache_with_progress(app_handle, scan_body);
                    if let Ok(mut guard) = cache_ref.lock() {
                        *guard = Some(cache);
                    }
                    value
                })
                .await
                .map(|value| json!(ApiResponse::ok(value)))
                .map_err(|err| anyhow!("disk usage scan failed: {}", err));
            }
            return Ok(json!(ApiResponse::<Value> {
                success: false,
                data: json!(null),
                error: Some("磁盘扫描正在进行，请等待本次扫描结束。".to_string()),
            }));
        }

        if matches!(
            (method, clean_path),
            ("POST", "/system/disk-usage/list") | ("POST", "system/disk-usage/list")
        ) {
            let path = body
                .as_ref()
                .and_then(|value| value.get("path"))
                .and_then(Value::as_str)
                .map(ToString::to_string);
            let filter = body
                .as_ref()
                .and_then(|value| value.get("filter"))
                .and_then(Value::as_str)
                .unwrap_or("all")
                .to_string();
            let page = body
                .as_ref()
                .and_then(|value| value.get("page"))
                .and_then(Value::as_i64)
                .unwrap_or(1);
            let size = body
                .as_ref()
                .and_then(|value| value.get("size"))
                .and_then(Value::as_i64)
                .unwrap_or(200);
            let guard = state
                .disk_usage_cache
                .lock()
                .map_err(|err| anyhow!("disk usage cache lock failed: {}", err))?;
            let Some(cache) = guard.as_ref() else {
                return Ok(json!(ApiResponse::<Value> {
                    success: false,
                    data: json!(null),
                    error: Some("暂无空间明细，请先点击“分析磁盘”完成一次扫描。".to_string()),
                }));
            };
            let path = path.unwrap_or_else(|| cache.root_path.clone());
            return Ok(json!(ApiResponse::ok(
                services::system_service::disk_usage_cached_view_paged(cache, &path, &filter, page, size)
            )));
        }

        if matches!(
            (method, clean_path),
            ("GET", "/system/disk-usage/status") | ("GET", "system/disk-usage/status")
        ) {
            let guard = state
                .disk_usage_cache
                .lock()
                .map_err(|err| anyhow!("disk usage cache lock failed: {}", err))?;
            return Ok(json!(ApiResponse::ok(
                services::system_service::disk_usage_cache_status(guard.as_ref())
            )));
        }

        if matches!(
            (method, clean_path),
            ("POST", "/system/disk-usage/delete") | ("POST", "system/disk-usage/delete")
        ) {
            let delete_body = body.clone();
            return tokio::task::spawn_blocking(move || {
                services::system_service::delete_disk_usage_path(delete_body)
            })
            .await
            .map(|value| json!(ApiResponse::ok(value)))
            .map_err(|err| anyhow!("disk usage delete failed: {}", err));
        }

        if matches!(
            (method, clean_path),
            ("POST", "/system/disk-usage/reveal") | ("POST", "system/disk-usage/reveal")
        ) {
            let reveal_body = body.clone();
            return tokio::task::spawn_blocking(move || {
                services::system_service::reveal_disk_usage_path(reveal_body)
            })
            .await
            .map(|value| json!(ApiResponse::ok(value)))
            .map_err(|err| anyhow!("disk usage reveal failed: {}", err));
        }

        // Community search uses async HTTP and manages its own connection
        if matches!(
            (method, clean_path),
            ("GET", "/community/search") | ("GET", "community/search")
        ) {
            let q_str = query.search.unwrap_or_default();
            let page = query.page.unwrap_or(1);
            let size = query.size.unwrap_or(20);
            return services::community::search_community_skills(
                &state.db_path,
                &q_str,
                page,
                size,
            )
            .await
            .map(|value| json!(ApiResponse::ok(value)));
        }

        if matches!(
            (method, clean_path),
            ("POST", "/community/install") | ("POST", "community/install")
        ) {
            return super::install_community_skill(&state.db_path, body)
                .await
                .map(|value| json!(ApiResponse::ok(value)));
        }

        if matches!(
            (method, clean_path),
            ("POST", "/community/compare") | ("POST", "community/compare")
        ) {
            return services::community::compare_with_llm(&state.db_path, body)
                .await
                .map(|value| json!(ApiResponse::ok(value)));
        }

        let conn = open_conn(&state.db_path)?;

        match (method, clean_path) {
            ("GET", "/health") | ("GET", "health") => {
                Ok(json!(ApiResponse::ok(json!({ "status": "ok" }))))
            }
            ("GET", "/scan/history") | ("GET", "scan/history") => {
                super::list_scan_history(&conn, &query).map(|value| json!(ApiResponse::ok(value)))
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
                let agent_id = p
                    .trim_start_matches('/')
                    .trim_start_matches("scan/sources/");
                let payload: SourceUpdate = serde_json::from_value(body.unwrap_or(Value::Null))?;
                update_source(&conn, agent_id, payload).map(|value| json!(ApiResponse::ok(value)))
            }
            ("DELETE", p) if p.starts_with("/scan/sources/") || p.starts_with("scan/sources/") => {
                let agent_id = p
                    .trim_start_matches('/')
                    .trim_start_matches("scan/sources/");
                reset_source(&conn, agent_id)?;
                Ok(json!(ApiResponse::ok(
                    json!({ "agent_id": agent_id, "message": "Source reset" })
                )))
            }
            ("GET", "/skills") | ("GET", "skills") => {
                list_skills(&conn, &query).map(|value| json!(ApiResponse::ok(value)))
            }
            ("GET", "/skills/categories") | ("GET", "skills/categories") => {
                list_categories(&conn).map(|value| json!(ApiResponse::ok(value)))
            }
            ("POST", "/skills/import") | ("POST", "skills/import") => {
                import_skills(&conn, body).map(|value| json!(ApiResponse::ok(value)))
            }
            ("POST", p)
                if p.starts_with("/skills/") && p.ends_with("/evolve")
                    || p.starts_with("skills/") && p.ends_with("/evolve") =>
            {
                let name = url_decode(
                    p.trim_start_matches('/')
                        .trim_start_matches("skills/")
                        .trim_end_matches("/evolve"),
                );
                create_manual_skill_draft(&conn, &name)
                    .map(|value| json!(ApiResponse::ok(value)))
            }
            ("PUT", p) if p.starts_with("/skills/") || p.starts_with("skills/") => {
                let name = url_decode(p.trim_start_matches('/').trim_start_matches("skills/"));
                update_skill(&conn, &name, &body.unwrap_or(Value::Null))
                    .map(|value| json!(ApiResponse::ok(value)))
            }
            ("DELETE", p) if p.starts_with("/skills/") || p.starts_with("skills/") => {
                let name = url_decode(p.trim_start_matches('/').trim_start_matches("skills/"));
                delete_skill(&conn, &name).map(|value| json!(ApiResponse::ok(value)))
            }
            ("GET", p) if p.starts_with("/skills/") || p.starts_with("skills/") => {
                let name = url_decode(p.trim_start_matches('/').trim_start_matches("skills/"));
                get_skill(&conn, &name, query.source_type.as_deref())
                    .map(|value| json!(ApiResponse::ok(value)))
            }
            ("GET", "/agents") | ("GET", "agents") => {
                list_agents(&conn, &query).map(|value| json!(ApiResponse::ok(value)))
            }
            ("PUT", p) if p.starts_with("/agents/") || p.starts_with("agents/") => {
                let name = url_decode(p.trim_start_matches('/').trim_start_matches("agents/"));
                update_agent(&conn, &name, &body.unwrap_or(Value::Null))
                    .map(|value| json!(ApiResponse::ok(value)))
            }
            ("DELETE", p) if p.starts_with("/agents/") || p.starts_with("agents/") => {
                let name = url_decode(p.trim_start_matches('/').trim_start_matches("agents/"));
                delete_agent(&conn, &name).map(|value| json!(ApiResponse::ok(value)))
            }
            ("POST", p)
                if p.starts_with("/agents/") && p.ends_with("/evolve")
                    || p.starts_with("agents/") && p.ends_with("/evolve") =>
            {
                let name = url_decode(
                    p.trim_start_matches('/')
                        .trim_start_matches("agents/")
                        .trim_end_matches("/evolve"),
                );
                create_agent_evolution_draft(&conn, &name)
                    .map(|value| json!(ApiResponse::ok(value)))
            }
            ("GET", p) if p.starts_with("/agents/") || p.starts_with("agents/") => {
                let name = url_decode(p.trim_start_matches('/').trim_start_matches("agents/"));
                get_agent(&conn, &name).map(|value| json!(ApiResponse::ok(value)))
            }
            ("GET", "/stats/summary") | ("GET", "stats/summary") => {
                super::stats_summary(&conn).map(|value| json!(ApiResponse::ok(value)))
            }
            ("GET", "/stats/skills/top") | ("GET", "stats/skills/top") => {
                super::top_skills(&conn, query.limit.unwrap_or(20))
                    .map(|value| json!(ApiResponse::ok(value)))
            }
            ("GET", "/stats/skills/by-category") | ("GET", "stats/skills/by-category") => {
                super::skills_by_category(&conn).map(|value| json!(ApiResponse::ok(value)))
            }
            ("GET", "/sessions") | ("GET", "sessions") => {
                super::list_sessions(&conn, &query).map(|value| json!(ApiResponse::ok(value)))
            }
            ("GET", p) if p.starts_with("/sessions/") || p.starts_with("sessions/") => {
                let session_id =
                    url_decode(p.trim_start_matches('/').trim_start_matches("sessions/"));
                super::get_session(&conn, &session_id)
                    .map(|value| json!(ApiResponse::ok(value)))
            }
            ("GET", "/admin/system") | ("GET", "admin/system") => {
                sync_source_configs(&conn)?;
                system_info(&conn).map(|value| json!(ApiResponse::ok(value)))
            }
            ("GET", "/admin/config/scan-paths") | ("GET", "admin/config/scan-paths") => {
                sync_source_configs(&conn)?;
                list_sources(&conn).map(|value| json!(ApiResponse::ok(value)))
            }
            ("GET", "/admin/config/llm") | ("GET", "admin/config/llm") => {
                Ok(json!(ApiResponse::ok(json!({
                    "enabled": get_config_bool(&conn, "llm_enabled", false)?,
                    "provider": get_config(&conn, "llm_provider")?.unwrap_or_else(|| "openai".to_string()),
                    "base_url": get_config(&conn, "llm_base_url")?.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
                    "model": get_config(&conn, "llm_model")?.unwrap_or_else(|| "gpt-5.5".to_string()),
                    "api_format": get_config(&conn, "llm_api_format")?.unwrap_or_else(|| "openai".to_string()),
                    "has_api_key": get_config(&conn, "llm_api_key")?.is_some()
                }))))
            }
            ("GET", "/export/skills") | ("GET", "export/skills") => {
                export_skills(&conn, &query).map(|value| json!(ApiResponse::ok(value)))
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
            ("GET", "/workflows") | ("GET", "workflows") => {
                list_workflows(&conn, &query).map(|value| json!(ApiResponse::ok(value)))
            }
            ("POST", "/workflows/cluster") | ("POST", "workflows/cluster") => {
                let clusters = services::workflow::cluster_workflows(&conn);
                services::workflow::save_clusters(&conn, &clusters)?;
                services::workflow::generate_skill_drafts(&conn, &clusters);
                list_workflows(&conn, &query).map(|value| json!(ApiResponse::ok(value)))
            }
            ("GET", "/workflows/install-targets") | ("GET", "workflows/install-targets") => {
                list_skill_install_targets(&conn).map(|value| json!(ApiResponse::ok(value)))
            }
            ("GET", "/skill-variants") | ("GET", "skill-variants") => {
                list_skill_variants(&conn, &query).map(|value| json!(ApiResponse::ok(value)))
            }
            ("PUT", p) if p.starts_with("/workflows/") || p.starts_with("workflows/") => {
                let id_text = p.trim_start_matches('/').trim_start_matches("workflows/");
                let id = id_text.parse::<i64>()?;
                update_workflow_draft(&conn, id, body).map(|value| json!(ApiResponse::ok(value)))
            }
            ("DELETE", p) if p.starts_with("/workflows/") || p.starts_with("workflows/") => {
                let id_text = p.trim_start_matches('/').trim_start_matches("workflows/");
                let id = id_text.parse::<i64>()?;
                let agent_id = body
                    .as_ref()
                    .and_then(|v| v.get("agent_id").and_then(Value::as_str));
                delete_workflow_with_uninstall(&conn, id, agent_id)?;
                Ok(json!(ApiResponse::ok(json!({ "id": id }))))
            }
            ("POST", p)
                if p.starts_with("/workflows/") && p.ends_with("/install")
                    || p.starts_with("workflows/") && p.ends_with("/install") =>
            {
                let id_text = p
                    .trim_start_matches('/')
                    .trim_start_matches("workflows/")
                    .trim_end_matches("/install");
                let id = id_text.parse::<i64>()?;
                install_workflow_skill(&conn, id, body)
                    .map(|value| json!(ApiResponse::ok(value)))
            }
            ("POST", "/evolution/start") | ("POST", "evolution/start") => {
                sync_source_configs(&conn)?;
                services::evolution::cleanup_stale_jobs(&conn);
                if services::evolution::has_incomplete_run(&conn) {
                    return Ok(json!(ApiResponse::<Value> {
                        success: false,
                        data: json!(null),
                        error: Some("进化管道正在运行中，请等待完成后再启动新的进化。如果确认已中断，请点击「加载状态」后使用「重置卡住的管道」。".to_string()),
                    }));
                }
                let scope = parse_scope_payload(body)?;
                let result = services::evolution::start_evolution_pipeline(
                    &conn,
                    app,
                    &state.db_path,
                    state.scan_lock.clone(),
                    scope.agent_ids,
                )?;
                Ok(json!(ApiResponse::ok(result)))
            }
            ("GET", "/evolution/status") | ("GET", "evolution/status") => {
                Ok(json!(ApiResponse::ok(
                    services::evolution::get_evolution_status(&conn)
                )))
            }
            ("POST", "/evolution/reset") | ("POST", "evolution/reset") => {
                Ok(json!(ApiResponse::ok(
                    services::evolution::reset_stuck_evolution(&conn)
                )))
            }
            ("GET", "/evolution/history") | ("GET", "evolution/history") => {
                services::evolution::cleanup_stale_jobs(&conn);
                Ok(json!(ApiResponse::ok(
                    services::evolution::list_evolution_history(&conn, &query)?
                )))
            }
            ("GET", "/system/monitor") | ("GET", "system/monitor") => {
                Ok(json!(ApiResponse::ok(
                    services::system_service::get_system_monitor()
                )))
            }
            ("GET", "/system/database") | ("GET", "system/database") => {
                Ok(json!(ApiResponse::ok(
                    services::system_service::get_database_info(&state.db_path, &conn)
                )))
            }
            ("GET", "/system/disk-cleanup/scan") | ("GET", "system/disk-cleanup/scan") => {
                Ok(json!(ApiResponse::ok(
                    services::system_service::scan_system_disk_cleanup()
                )))
            }
            ("POST", "/system/disk-cleanup/clean") | ("POST", "system/disk-cleanup/clean") => {
                Ok(json!(ApiResponse::ok(
                    services::system_service::clean_system_disk_items(body)
                )))
            }
            ("POST", "/system/clear-data") | ("POST", "system/clear-data") => Ok(json!(
                ApiResponse::ok(services::system_service::clear_all_data(&conn))
            )),
            ("POST", "/system/clear-logs") | ("POST", "system/clear-logs") => Ok(json!(
                ApiResponse::ok(services::system_service::clear_logs(&conn))
            )),
            ("POST", "/system/initialize-database")
            | ("POST", "system/initialize-database") => {
                let value = services::system_service::initialize_database(&conn);
                sync_source_configs(&conn)?;
                Ok(json!(ApiResponse::ok(value)))
            }
            ("POST", "/community/install") | ("POST", "community/install") => {
                unreachable!("handled before opening DB connection")
            }
            ("GET", "/community/installed") | ("GET", "community/installed") => {
                services::community::get_installed_skills(&conn)
                    .map(|value| json!(ApiResponse::ok(value)))
            }
            ("GET", p) if p.starts_with("/community/") || p.starts_with("community/") => {
                let id_text = p.trim_start_matches('/').trim_start_matches("community/");
                let id = id_text.parse::<i64>()?;
                services::community::get_community_skill_detail(&conn, id)
                    .map(|value| json!(ApiResponse::ok(value)))
            }
            _ => Err(anyhow!(
                "unsupported local Tauri API route: {} {}",
                method,
                path
            )),
        }
    }
}

// ---- Community skill helpers ----

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

    let (name, repo_full_name, repo_url, description, file_url): (
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    ) = {
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
        .map(std::path::PathBuf::from)
        .ok_or_else(|| anyhow!("{} has no skills_path configured", target.agent_name))?;

    fs::create_dir_all(&install_dir)?;
    let skill_dir = install_dir.join(sanitize_file_name(&name));
    fs::create_dir_all(&skill_dir)?;
    let skill_path = skill_dir.join("SKILL.md");
    fs::write(&skill_path, &skill_content)?;

    if let Some(info) =
        services::scan::parse_skill_file(&skill_path, "community", &target.agent_id, None)
    {
        services::scan::upsert_skills(&conn, &[info])?;
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

pub(crate) fn community_skill_raw_url(repo_full_name: &str, file_url: Option<&str>) -> Option<String> {
    if let Some(file_url) = file_url {
        if let Some(rest) = file_url.strip_prefix("https://github.com/") {
            let parts = rest.split('/').collect::<Vec<_>>();
            if parts.len() >= 5 && parts[2] == "blob" {
                let repo = format!("{}/{}", parts[0], parts[1]);
                let branch = parts[3];
                let file_path = parts[4..].join("/");
                return Some(format!(
                    "https://raw.githubusercontent.com/{repo}/{branch}/{file_path}"
                ));
            }
        }
        if file_url.starts_with("https://raw.githubusercontent.com/") {
            return Some(file_url.to_string());
        }
    }
    if repo_full_name.contains('/') {
        Some(format!(
            "https://raw.githubusercontent.com/{repo_full_name}/main/SKILL.md"
        ))
    } else {
        None
    }
}

pub(crate) async fn fetch_community_skill_markdown(raw_url: &str) -> Result<String> {
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
    let text = response.text().await.unwrap_or_else(|e| {
        format!("[Failed to read response body: {e}]")
    });
    if !status.is_success() {
        return Err(anyhow!(
            "failed to download community skill: HTTP {}. The repository may not have SKILL.md at root, or GitHub is unreachable.",
            status.as_u16()
        ));
    }
    Ok(text)
}

// ---- Session CRUD ----

fn list_sessions(conn: &Connection, query: &PageQuery) -> Result<Value> {
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
        .query_map(
            params![query.project, size, (page - 1) * size],
            |row| {
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
            },
        )?
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

// ---- Stats ----

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

// ---- Scan history ----

fn list_scan_history(conn: &Connection, query: &PageQuery) -> Result<Value> {
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
