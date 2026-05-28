use anyhow::anyhow;
use serde_json::json;

use crate::services;
use crate::{open_conn, parse_scope_payload, ApiResponse, SourceUpdate};

pub(crate) async fn dispatch_api(
    state: &crate::AppState,
    app: &tauri::AppHandle,
    method: &str,
    path: &str,
    params_value: Option<serde_json::Value>,
    body: Option<serde_json::Value>,
) -> anyhow::Result<serde_json::Value> {
    let clean_path = path.trim_start_matches("/api").trim_end_matches('/');
    let query: crate::PageQuery =
        serde_json::from_value(params_value.unwrap_or(serde_json::Value::Null)).unwrap_or_default();

    // LLM test: pre-fill stored API key if not provided in request
    if matches!(
        (method, clean_path),
        ("POST", "/admin/config/llm/test") | ("POST", "admin/config/llm/test")
    ) {
        let mut body = body.unwrap_or(serde_json::Value::Null);
        let api_key_empty = body
            .get("llm_api_key")
            .and_then(serde_json::Value::as_str)
            .map(|s| s.trim().is_empty())
            .unwrap_or(true);
        if api_key_empty {
            let conn = open_conn(&state.db_path)?;
            if let Ok(Some(stored_key)) = services::admin_service::get_config(&conn, "llm_api_key")
            {
                if let Some(obj) = body.as_object_mut() {
                    obj.insert(
                        "llm_api_key".to_string(),
                        serde_json::Value::String(stored_key),
                    );
                }
            }
        }
        return services::admin_service::test_llm_connection(Some(body))
            .await
            .map(|value| json!(ApiResponse::ok(value)));
    }

    // Scan requires the lock to be acquired before opening the connection
    if matches!((method, clean_path), ("POST", "/scan") | ("POST", "scan")) {
        let _guard = state.scan_lock.lock().await;
        let conn = open_conn(&state.db_path)?;
        let scope = parse_scope_payload(body)?;
        return services::scan::run_full_scan(&conn, app, scope.agent_ids.as_deref())
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
                let (cache, value) =
                    services::system::build_disk_usage_cache_with_progress(app_handle, scan_body);
                if let Ok(mut guard) = cache_ref.lock() {
                    *guard = Some(cache);
                }
                value
            })
            .await
            .map(|value| json!(ApiResponse::ok(value)))
            .map_err(|err| anyhow!("disk usage scan failed: {}", err));
        }
        return Ok(json!(ApiResponse::<serde_json::Value> {
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
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string);
        let filter = body
            .as_ref()
            .and_then(|value| value.get("filter"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("all")
            .to_string();
        let page = body
            .as_ref()
            .and_then(|value| value.get("page"))
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(1);
        let size = body
            .as_ref()
            .and_then(|value| value.get("size"))
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(200);
        let guard = state
            .disk_usage_cache
            .lock()
            .map_err(|err| anyhow!("disk usage cache lock failed: {}", err))?;
        let Some(cache) = guard.as_ref() else {
            return Ok(json!(ApiResponse::<serde_json::Value> {
                success: false,
                data: json!(null),
                error: Some("暂无空间明细，请先点击「分析磁盘」完成一次扫描。".to_string()),
            }));
        };
        let path = path.unwrap_or_else(|| cache.root_path.clone());
        return Ok(json!(ApiResponse::ok(
            services::system::disk_usage_cached_view_paged(cache, &path, &filter, page, size,)
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
            services::system::disk_usage_cache_status(guard.as_ref())
        )));
    }

    if matches!(
        (method, clean_path),
        ("POST", "/system/disk-usage/delete") | ("POST", "system/disk-usage/delete")
    ) {
        let delete_body = body.clone();
        return tokio::task::spawn_blocking(move || {
            services::system::delete_disk_usage_path(delete_body)
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
            services::system::reveal_disk_usage_path(reveal_body)
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
        return services::community::search_community_skills(&state.db_path, &q_str, page, size)
            .await
            .map(|value| json!(ApiResponse::ok(value)));
    }

    if matches!(
        (method, clean_path),
        ("POST", "/community/install") | ("POST", "community/install")
    ) {
        return super::helpers::install_community_skill(&state.db_path, body)
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
            super::helpers::list_scan_history(&conn, &query)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/scan/sources") | ("GET", "scan/sources") => {
            services::scan::sync_source_configs(&conn)?;
            services::scan::list_sources(&conn).map(|value| json!(ApiResponse::ok(value)))
        }
        ("POST", "/scan/sources/detect") | ("POST", "scan/sources/detect") => {
            services::scan::sync_source_configs(&conn)?;
            services::scan::list_sources(&conn).map(|value| json!(ApiResponse::ok(value)))
        }
        ("PUT", p) if p.starts_with("/scan/sources/") || p.starts_with("scan/sources/") => {
            let agent_id = p
                .trim_start_matches('/')
                .trim_start_matches("scan/sources/");
            let payload: SourceUpdate =
                serde_json::from_value(body.unwrap_or(serde_json::Value::Null))?;
            services::scan::update_source(&conn, agent_id, payload)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("DELETE", p) if p.starts_with("/scan/sources/") || p.starts_with("scan/sources/") => {
            let agent_id = p
                .trim_start_matches('/')
                .trim_start_matches("scan/sources/");
            services::scan::reset_source(&conn, agent_id)?;
            Ok(json!(ApiResponse::ok(
                json!({ "agent_id": agent_id, "message": "Source reset" })
            )))
        }
        ("GET", "/skills") | ("GET", "skills") => services::skills_crud::list_skills(&conn, &query)
            .map(|value| json!(ApiResponse::ok(value))),
        ("GET", "/skills/categories") | ("GET", "skills/categories") => {
            services::skills_crud::list_categories(&conn).map(|value| json!(ApiResponse::ok(value)))
        }
        ("POST", "/skills/import") | ("POST", "skills/import") => {
            services::import_export::import_skills(&conn, body)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("POST", p)
            if p.starts_with("/skills/") && p.ends_with("/evolve")
                || p.starts_with("skills/") && p.ends_with("/evolve") =>
        {
            let name = crate::url_decode(
                p.trim_start_matches('/')
                    .trim_start_matches("skills/")
                    .trim_end_matches("/evolve"),
            );
            services::skills_crud::create_manual_skill_draft(&conn, &name)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("PUT", p) if p.starts_with("/skills/") || p.starts_with("skills/") => {
            let name = crate::url_decode(p.trim_start_matches('/').trim_start_matches("skills/"));
            services::skills_crud::update_skill(
                &conn,
                &name,
                &body.unwrap_or(serde_json::Value::Null),
            )
            .map(|value| json!(ApiResponse::ok(value)))
        }
        ("DELETE", p) if p.starts_with("/skills/") || p.starts_with("skills/") => {
            let name = crate::url_decode(p.trim_start_matches('/').trim_start_matches("skills/"));
            services::skills_crud::delete_skill(&conn, &name)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", p) if p.starts_with("/skills/") || p.starts_with("skills/") => {
            let name = crate::url_decode(p.trim_start_matches('/').trim_start_matches("skills/"));
            services::skills_crud::get_skill(&conn, &name, query.source_type.as_deref())
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/agents") | ("GET", "agents") => services::agents_crud::list_agents(&conn, &query)
            .map(|value| json!(ApiResponse::ok(value))),
        ("PUT", p) if p.starts_with("/agents/") || p.starts_with("agents/") => {
            let name = crate::url_decode(p.trim_start_matches('/').trim_start_matches("agents/"));
            services::agents_crud::update_agent(
                &conn,
                &name,
                &body.unwrap_or(serde_json::Value::Null),
            )
            .map(|value| json!(ApiResponse::ok(value)))
        }
        ("DELETE", p) if p.starts_with("/agents/") || p.starts_with("agents/") => {
            let name = crate::url_decode(p.trim_start_matches('/').trim_start_matches("agents/"));
            services::agents_crud::delete_agent(&conn, &name)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("POST", p)
            if p.starts_with("/agents/") && p.ends_with("/evolve")
                || p.starts_with("agents/") && p.ends_with("/evolve") =>
        {
            let name = crate::url_decode(
                p.trim_start_matches('/')
                    .trim_start_matches("agents/")
                    .trim_end_matches("/evolve"),
            );
            services::agents_crud::create_agent_evolution_draft(&conn, &name)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", p) if p.starts_with("/agents/") || p.starts_with("agents/") => {
            let name = crate::url_decode(p.trim_start_matches('/').trim_start_matches("agents/"));
            services::agents_crud::get_agent(&conn, &name)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/stats/summary") | ("GET", "stats/summary") => {
            super::helpers::stats_summary(&conn).map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/stats/skills/top") | ("GET", "stats/skills/top") => {
            super::helpers::top_skills(&conn, query.limit.unwrap_or(20))
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/stats/skills/by-category") | ("GET", "stats/skills/by-category") => {
            super::helpers::skills_by_category(&conn).map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/sessions") | ("GET", "sessions") => {
            super::helpers::list_sessions(&conn, &query).map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", p) if p.starts_with("/sessions/") || p.starts_with("sessions/") => {
            let session_id =
                crate::url_decode(p.trim_start_matches('/').trim_start_matches("sessions/"));
            super::helpers::get_session(&conn, &session_id)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/admin/system") | ("GET", "admin/system") => {
            services::scan::sync_source_configs(&conn)?;
            services::admin_service::system_info(&conn).map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/admin/config/scan-paths") | ("GET", "admin/config/scan-paths") => {
            services::scan::sync_source_configs(&conn)?;
            services::scan::list_sources(&conn).map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/admin/config/llm") | ("GET", "admin/config/llm") => {
            Ok(json!(ApiResponse::ok(json!({
                "enabled": services::admin_service::get_config_bool(&conn, "llm_enabled", false)?,
                "provider": services::admin_service::get_config(&conn, "llm_provider")?.unwrap_or_else(|| "openai".to_string()),
                "base_url": services::admin_service::get_config(&conn, "llm_base_url")?.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
                "model": services::admin_service::get_config(&conn, "llm_model")?.unwrap_or_else(|| "gpt-5.5".to_string()),
                "api_format": services::admin_service::get_config(&conn, "llm_api_format")?.unwrap_or_else(|| "openai".to_string()),
                "has_api_key": services::admin_service::get_config(&conn, "llm_api_key")?.is_some()
            }))))
        }
        ("GET", "/export/skills") | ("GET", "export/skills") => {
            services::import_export::export_skills(&conn, &query)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/export/agents") | ("GET", "export/agents") => {
            services::import_export::export_agents(&conn).map(|value| json!(ApiResponse::ok(value)))
        }
        ("PUT", "/admin/config/llm") | ("PUT", "admin/config/llm") => {
            services::admin_service::save_llm_config(&conn, body)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("POST", "/admin/login") | ("POST", "admin/login") => Ok(json!(ApiResponse::ok(json!({
            "token": "tauri-local",
            "username": body
                .as_ref()
                .and_then(|v| v.get("username"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("local"),
            "user": { "id": 1, "username": "local" }
        })))),
        ("GET", "/admin/users") | ("GET", "admin/users") => {
            services::admin_service::list_admin_users(&conn)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("POST", "/admin/users") | ("POST", "admin/users") => {
            services::admin_service::create_admin_user(&conn, body)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("PUT", p) if p.starts_with("/admin/users/") || p.starts_with("admin/users/") => {
            let id = p
                .trim_start_matches('/')
                .trim_start_matches("admin/users/")
                .parse::<i64>()?;
            services::admin_service::update_admin_user(&conn, id, body)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("DELETE", p) if p.starts_with("/admin/users/") || p.starts_with("admin/users/") => {
            let id = p
                .trim_start_matches('/')
                .trim_start_matches("admin/users/")
                .parse::<i64>()?;
            services::admin_service::delete_admin_user(&conn, id)?;
            Ok(json!(ApiResponse::ok(json!({ "id": id }))))
        }
        ("GET", "/workflows") | ("GET", "workflows") => {
            services::skills_crud::list_workflows(&conn, &query)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("POST", "/workflows/cluster") | ("POST", "workflows/cluster") => {
            let clusters = services::workflow::cluster_workflows(&conn);
            services::workflow::save_clusters(&conn, &clusters)?;
            services::workflow::generate_skill_drafts(&conn, &clusters);
            services::skills_crud::list_workflows(&conn, &query)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/workflows/install-targets") | ("GET", "workflows/install-targets") => {
            services::skills_crud::list_skill_install_targets(&conn)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("GET", "/skill-variants") | ("GET", "skill-variants") => {
            services::skills_crud::list_skill_variants(&conn, &query)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("PUT", p) if p.starts_with("/workflows/") || p.starts_with("workflows/") => {
            let id_text = p.trim_start_matches('/').trim_start_matches("workflows/");
            let id = id_text.parse::<i64>()?;
            services::workflow_crud::update_workflow_draft(&conn, id, body)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("DELETE", p) if p.starts_with("/workflows/") || p.starts_with("workflows/") => {
            let id_text = p.trim_start_matches('/').trim_start_matches("workflows/");
            let id = id_text.parse::<i64>()?;
            let agent_id = body
                .as_ref()
                .and_then(|v| v.get("agent_id").and_then(serde_json::Value::as_str));
            services::workflow_crud::delete_workflow_with_uninstall(&conn, id, agent_id)?;
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
            services::workflow_crud::install_workflow_skill(&conn, id, body)
                .map(|value| json!(ApiResponse::ok(value)))
        }
        ("POST", "/evolution/start") | ("POST", "evolution/start") => {
            services::scan::sync_source_configs(&conn)?;
            services::evolution::cleanup_stale_jobs(&conn);
            if services::evolution::has_incomplete_run(&conn) {
                return Ok(json!(ApiResponse::<serde_json::Value> {
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
        ("GET", "/evolution/status") | ("GET", "evolution/status") => Ok(json!(ApiResponse::ok(
            services::evolution::get_evolution_status(&conn)
        ))),
        ("POST", "/evolution/reset") | ("POST", "evolution/reset") => Ok(json!(ApiResponse::ok(
            services::evolution::reset_stuck_evolution(&conn)
        ))),
        ("GET", "/evolution/history") | ("GET", "evolution/history") => {
            services::evolution::cleanup_stale_jobs(&conn);
            Ok(json!(ApiResponse::ok(
                services::evolution::list_evolution_history(&conn, &query)?
            )))
        }
        ("GET", "/system/monitor") | ("GET", "system/monitor") => Ok(json!(ApiResponse::ok(
            services::system::get_system_monitor()
        ))),
        ("GET", "/system/database") | ("GET", "system/database") => Ok(json!(ApiResponse::ok(
            services::system::get_database_info(&state.db_path, &conn)
        ))),
        ("GET", "/system/disk-cleanup/scan") | ("GET", "system/disk-cleanup/scan") => Ok(json!(
            ApiResponse::ok(services::system::scan_system_disk_cleanup())
        )),
        ("POST", "/system/disk-cleanup/clean") | ("POST", "system/disk-cleanup/clean") => {
            Ok(json!(ApiResponse::ok(
                services::system::clean_system_disk_items(body)
            )))
        }
        ("POST", "/system/clear-data") | ("POST", "system/clear-data") => Ok(json!(
            ApiResponse::ok(services::system::clear_all_data(&conn))
        )),
        ("POST", "/system/clear-logs") | ("POST", "system/clear-logs") => {
            Ok(json!(ApiResponse::ok(services::system::clear_logs(&conn))))
        }
        ("POST", "/system/initialize-database") | ("POST", "system/initialize-database") => {
            let value = services::system::initialize_database(&conn);
            services::scan::sync_source_configs(&conn)?;
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
