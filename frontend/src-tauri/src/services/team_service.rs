use crate::AppState;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Mutex;

const KEYRING_SERVICE: &str = "self-evolving-skills";
const KEYRING_TOKEN_KEY: &str = "team-jwt";
const KEYRING_REFRESH_KEY: &str = "team-refresh";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct TeamSession {
    pub(crate) server_url: String,
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
    pub(crate) username: String,
    pub(crate) user_id: i64,
    pub(crate) dept_name: String,
    pub(crate) dept_id: i64,
}

pub(crate) struct TeamState {
    pub(crate) session: Mutex<Option<TeamSession>>,
    pub(crate) http_client: reqwest::Client,
}

impl TeamState {
    pub(crate) fn new() -> Self {
        Self {
            session: Mutex::new(None),
            http_client: reqwest::Client::new(),
        }
    }

    fn save_tokens(&self, access: &str, refresh: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_TOKEN_KEY)
            .map_err(|e| format!("keyring error: {}", e))?;
        entry
            .set_password(access)
            .map_err(|e| format!("keyring set: {}", e))?;

        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_REFRESH_KEY)
            .map_err(|e| format!("keyring error: {}", e))?;
        entry
            .set_password(refresh)
            .map_err(|e| format!("keyring set: {}", e))?;
        Ok(())
    }

    pub(crate) fn delete_tokens(&self) -> Result<(), String> {
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_TOKEN_KEY) {
            let _ = entry.delete_credential();
        }
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_REFRESH_KEY) {
            let _ = entry.delete_credential();
        }
        Ok(())
    }

    async fn try_refresh_token(&self) -> Result<String, String> {
        let (url, token) = {
            let session = self.session.lock().map_err(|e| e.to_string())?;
            let session = session.as_ref().ok_or("未登录")?;
            (session.server_url.clone(), session.access_token.clone())
        };

        let resp = self
            .http_client
            .post(format!("{}/api/team/refresh", url))
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| format!("刷新请求失败: {}", e))?;

        if !resp.status().is_success() {
            return Err("Token 已过期，请重新登录".to_string());
        }

        let data: Value = resp.json().await.map_err(|e| format!("解析失败: {}", e))?;
        let new_token = data["data"]["token"]
            .as_str()
            .ok_or("刷新响应缺少 token")?
            .to_string();

        self.save_tokens(&new_token, "")?;
        {
            let mut session = self.session.lock().map_err(|e| e.to_string())?;
            if let Some(ref mut s) = *session {
                s.access_token = new_token.clone();
            }
        }
        Ok(new_token)
    }
}

#[tauri::command]
pub(crate) async fn login_team(
    state: tauri::State<'_, TeamState>,
    server_url: String,
    username: String,
    password: String,
) -> Result<Value, String> {
    let base = server_url.trim_end_matches('/');
    let client = &state.http_client;

    // Ping server
    let ping_url = format!("{}/api/team/ping", base);
    client
        .get(&ping_url)
        .send()
        .await
        .map_err(|e| format!("无法连接到服务器: {}", e))?;

    // Login
    let login_url = format!("{}/api/team/login", base);
    let resp = client
        .post(&login_url)
        .header(CONTENT_TYPE, "application/json")
        .json(&serde_json::json!({
            "username": username,
            "password": password,
            "device": "desktop"
        }))
        .send()
        .await
        .map_err(|e| format!("登录请求失败: {}", e))?;

    if !resp.status().is_success() {
        return Err("用户名或密码错误".to_string());
    }

    let data: Value = resp
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    // Check business status code in AjaxResult envelope
    if data["code"].as_i64() != Some(200) {
        let msg = data["msg"].as_str().unwrap_or("登录失败");
        return Err(msg.to_string());
    }

    let payload = &data["data"];
    let access_token = payload["token"].as_str().ok_or("token 缺失")?.to_string();
    let refresh_token = payload["refresh_token"].as_str().unwrap_or("").to_string();
    let username_str = payload["username"]
        .as_str()
        .unwrap_or(&username)
        .to_string();
    let user_id = payload["user_id"].as_i64().unwrap_or(0);
    let dept_name = payload["dept_name"].as_str().unwrap_or("").to_string();
    let dept_id = payload["dept_id"].as_i64().unwrap_or(0);

    state.save_tokens(&access_token, &refresh_token)?;

    let session = TeamSession {
        server_url: base.to_string(),
        access_token: access_token.clone(),
        refresh_token: refresh_token.clone(),
        username: username_str.clone(),
        user_id,
        dept_name: dept_name.clone(),
        dept_id,
    };
    *state.session.lock().map_err(|e| e.to_string())? = Some(session);

    Ok(serde_json::json!({
        "username": username_str,
        "user_id": user_id,
        "dept_name": dept_name,
        "dept_id": dept_id,
        "token": access_token
    }))
}

#[tauri::command]
pub(crate) async fn logout_team(state: tauri::State<'_, TeamState>) -> Result<(), String> {
    // Call server logout endpoint (best effort — ignore errors if server unreachable)
    call_server_logout(&state).await;
    state.delete_tokens()?;
    *state.session.lock().map_err(|e| e.to_string())? = None;
    Ok(())
}

/// Call /api/team/logout to invalidate the server-side token.
/// Best-effort: failures are silently ignored.
pub(crate) async fn call_server_logout(state: &TeamState) {
    let (url, token) = match (|| -> Option<_> {
        let session = state.session.lock().ok()?;
        let session = session.as_ref()?;
        Some((session.server_url.clone(), session.access_token.clone()))
    })() {
        Some(v) => v,
        None => return,
    };
    let _ = state
        .http_client
        .post(format!("{}/api/team/logout", url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await;
}

#[tauri::command]
pub(crate) async fn get_team_session(
    state: tauri::State<'_, TeamState>,
) -> Result<Option<Value>, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;
    match session.as_ref() {
        Some(s) => Ok(Some(serde_json::json!({
            "username": s.username,
            "user_id": s.user_id,
            "dept_name": s.dept_name,
            "dept_id": s.dept_id,
            "server_url": s.server_url,
        }))),
        None => Ok(None),
    }
}

#[tauri::command]
pub(crate) async fn team_api_get(
    state: tauri::State<'_, TeamState>,
    path: String,
) -> Result<Value, String> {
    api_call_with_retry(&state, "GET", &path, None).await
}

#[tauri::command]
pub(crate) async fn team_api_post(
    state: tauri::State<'_, TeamState>,
    path: String,
    body: Value,
) -> Result<Value, String> {
    api_call_with_retry(&state, "POST", &path, Some(&body)).await
}

#[tauri::command]
pub(crate) async fn team_api_put(
    state: tauri::State<'_, TeamState>,
    path: String,
    body: Value,
) -> Result<Value, String> {
    api_call_with_retry(&state, "PUT", &path, Some(&body)).await
}

#[tauri::command]
pub(crate) async fn team_api_delete(
    state: tauri::State<'_, TeamState>,
    path: String,
) -> Result<Value, String> {
    api_call_with_retry(&state, "DELETE", &path, None).await
}

#[tauri::command]
pub(crate) async fn check_team_connection(
    state: tauri::State<'_, TeamState>,
) -> Result<Value, String> {
    let (url, token) = {
        let session = state.session.lock().map_err(|e| e.to_string())?;
        let session = session.as_ref().ok_or("未登录")?;
        (session.server_url.clone(), session.access_token.clone())
    };

    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        state
            .http_client
            .get(format!("{}/api/team/ping", url))
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .send(),
    )
    .await
    {
        Ok(Ok(resp)) if resp.status().is_success() => {
            // Also verify auth is valid by calling profile
            let profile_resp = state
                .http_client
                .get(format!("{}/api/team/profile", url))
                .header(AUTHORIZATION, format!("Bearer {}", token))
                .send()
                .await;

            match profile_resp {
                Ok(r) if r.status().is_success() => Ok(serde_json::json!({
                    "connected": true,
                    "authenticated": true
                })),
                Ok(r) if r.status().as_u16() == 401 => {
                    // Try token refresh
                    match state.try_refresh_token().await {
                        Ok(_) => Ok(serde_json::json!({
                            "connected": true,
                            "authenticated": true,
                            "refreshed": true
                        })),
                        Err(_) => Ok(serde_json::json!({
                            "connected": true,
                            "authenticated": false,
                            "error": "Token 已过期，请重新登录"
                        })),
                    }
                }
                _ => Ok(serde_json::json!({
                    "connected": true,
                    "authenticated": false
                })),
            }
        }
        Ok(Ok(_)) => Ok(serde_json::json!({
            "connected": true,
            "authenticated": false
        })),
        Ok(Err(_)) | Err(_) => Ok(serde_json::json!({
            "connected": false,
            "authenticated": false
        })),
    }
}

async fn api_call_with_retry(
    state: &TeamState,
    method: &str,
    path: &str,
    body: Option<&Value>,
) -> Result<Value, String> {
    let resp = do_api_call(state, method, path, body).await?;

    if resp.status().as_u16() == 401 {
        // Try token refresh and retry once
        state.try_refresh_token().await?;
        let retry_resp = do_api_call(state, method, path, body).await?;
        return parse_api_response(retry_resp).await;
    }

    parse_api_response(resp).await
}

async fn parse_api_response(resp: reqwest::Response) -> Result<Value, String> {
    let status = resp.status();
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("服务端响应解析失败 (HTTP {}): {}", status.as_u16(), e))?;

    if !status.is_success() {
        let msg = body["msg"].as_str().unwrap_or("服务器内部错误");
        return Err(format!("[HTTP {}] {}", status.as_u16(), msg));
    }

    // Check business code in AjaxResult envelope
    if let Some(code) = body["code"].as_i64() {
        if code != 200 {
            let msg = body["msg"].as_str().unwrap_or("请求失败");
            return Err(msg.to_string());
        }
    }

    Ok(body)
}

async fn do_api_call(
    state: &TeamState,
    method: &str,
    path: &str,
    body: Option<&Value>,
) -> Result<reqwest::Response, String> {
    let (url, token) = {
        let session = state.session.lock().map_err(|e| e.to_string())?;
        let session = session.as_ref().ok_or("未登录")?;
        (
            format!("{}/api{}", session.server_url, path),
            format!("Bearer {}", session.access_token),
        )
    };

    let req = match method {
        "GET" => state.http_client.get(&url).header(AUTHORIZATION, &token),
        "DELETE" => state.http_client.delete(&url).header(AUTHORIZATION, &token),
        "POST" => {
            let mut r = state.http_client.post(&url).header(AUTHORIZATION, &token);
            if let Some(b) = body {
                r = r.header(CONTENT_TYPE, "application/json").json(b);
            }
            r
        }
        "PUT" => {
            let mut r = state.http_client.put(&url).header(AUTHORIZATION, &token);
            if let Some(b) = body {
                r = r.header(CONTENT_TYPE, "application/json").json(b);
            }
            r
        }
        _ => return Err("不支持的 HTTP 方法".to_string()),
    };

    req.send().await.map_err(|e| format!("API 请求失败: {}", e))
}

// ---- Offline cache commands ----

#[tauri::command]
pub(crate) fn cache_team_skills(
    app_state: tauri::State<'_, AppState>,
    skills: Vec<Value>,
) -> Result<Value, String> {
    let count = super::team_cache::cache_team_skills(&app_state.db_path, &skills)?;
    Ok(serde_json::json!({ "cached": count }))
}

#[tauri::command]
pub(crate) fn get_cached_team_skills(
    app_state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    super::team_cache::get_cached_skills(&app_state.db_path)
}

#[tauri::command]
pub(crate) fn get_team_cache_summary(
    app_state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    super::team_cache::get_cache_summary(&app_state.db_path)
}

#[tauri::command]
pub(crate) fn get_pending_operations(
    app_state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    super::team_cache::get_pending_operations(&app_state.db_path)
}

#[tauri::command]
pub(crate) async fn flush_pending_operations(
    app_state: tauri::State<'_, AppState>,
    team_state: tauri::State<'_, TeamState>,
) -> Result<Value, String> {
    let ops_data = super::team_cache::get_pending_operations(&app_state.db_path)?;
    let ops = ops_data["ops"].as_array().cloned().unwrap_or_default();

    if ops.is_empty() {
        return Ok(serde_json::json!({ "synced": 0, "failed": 0 }));
    }

    let mut synced = 0;
    let mut failed = 0;

    for op in &ops {
        let id = op["id"].as_i64().unwrap_or(0);
        let method = op["method"].as_str().unwrap_or("GET");
        let path = op["path"].as_str().unwrap_or("");
        let body: Option<Value> = op["body"]
            .as_str()
            .and_then(|s| serde_json::from_str(s).ok());

        super::team_cache::increment_retry_count(&app_state.db_path, id)?;

        match api_call_with_retry(&team_state, method, path, body.as_ref()).await {
            Ok(_) => {
                super::team_cache::update_operation_status(&app_state.db_path, id, "synced", None)?;
                synced += 1;
            }
            Err(e) => {
                super::team_cache::update_operation_status(
                    &app_state.db_path,
                    id,
                    "failed",
                    Some(&e),
                )?;
                failed += 1;
            }
        }
    }

    // Clean up old synced ops
    let _ = super::team_cache::cleanup_completed_ops(&app_state.db_path, 20);

    Ok(serde_json::json!({ "synced": synced, "failed": failed }))
}

#[tauri::command]
pub(crate) fn queue_team_operation(
    app_state: tauri::State<'_, AppState>,
    op_type: String,
    path: String,
    method: String,
    body: Option<Value>,
) -> Result<Value, String> {
    let id = super::team_cache::queue_operation(
        &app_state.db_path,
        &op_type,
        &path,
        &method,
        body.as_ref(),
    )?;
    Ok(serde_json::json!({ "id": id }))
}
