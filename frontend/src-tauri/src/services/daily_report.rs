use crate::utils::time::now_string;
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::llm_utils::{call_llm, LlmCallParams};

/// In-memory generation status (lives as long as the app process)
static GEN_STATUS: Mutex<Option<GenState>> = Mutex::new(None);

#[derive(Clone, Debug)]
struct GenState {
    target_date: String,
    phase: String, // "scanning" | "summarizing" | "done" | "error"
    progress: u32, // 0-100
    error_msg: Option<String>,
}

pub(crate) fn get_generation_status() -> Value {
    let guard = GEN_STATUS.lock().unwrap();
    match guard.as_ref() {
        Some(s) => json!({
            "generating": !matches!(s.phase.as_str(), "done" | "error"),
            "target_date": s.target_date,
            "phase": s.phase,
            "progress": s.progress,
            "error": s.error_msg,
        }),
        None => json!({ "generating": false }),
    }
}

pub(crate) fn get_report(conn: &Connection, date: &str) -> Result<Option<Value>> {
    conn.query_row(
        "SELECT content, source_count, generated_at FROM daily_reports WHERE report_date = ?1",
        [date],
        |row| {
            Ok(json!({
                "report_date": date,
                "content": row.get::<_, String>(0)?,
                "source_count": row.get::<_, i64>(1)?,
                "generated_at": row.get::<_, String>(2)?,
            }))
        },
    )
    .optional()
    .map_err(Into::into)
}

pub(crate) fn get_report_history(conn: &Connection, limit: i64) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT report_date, source_count, generated_at FROM daily_reports ORDER BY report_date DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |row| {
        Ok(json!({
            "report_date": row.get::<_, String>(0)?,
            "source_count": row.get::<_, i64>(1)?,
            "generated_at": row.get::<_, String>(2)?,
        }))
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

/// Start async daily report generation. `target_date` is the date to generate for (YYYY-MM-DD).
pub(crate) fn start_generation(
    db_path: PathBuf,
    base_url: String,
    api_key: String,
    model: String,
    api_format: String,
    target_date: String,
) -> Result<Value> {
    // Check if already generating
    {
        let guard = GEN_STATUS.lock().unwrap();
        if let Some(s) = guard.as_ref() {
            if !matches!(s.phase.as_str(), "done" | "error") {
                return Ok(json!({
                    "generating": true,
                    "target_date": s.target_date,
                    "phase": s.phase,
                    "progress": s.progress,
                    "message": "已有日报正在生成中，请等待完成",
                }));
            }
        }
    }

    // Check if already cached for this date
    {
        let conn = crate::db::open_conn(&db_path)?;
        if let Ok(Some(existing)) = get_report(&conn, &target_date) {
            return Ok(json!({
                "generating": false,
                "report_date": target_date,
                "content": existing["content"],
                "source_count": existing["source_count"],
                "generated_at": existing["generated_at"],
                "from_cache": true,
                "message": "该日期日报已存在，直接加载",
            }));
        }
    }

    let date = target_date.clone();
    // Set initial status
    {
        let mut guard = GEN_STATUS.lock().unwrap();
        *guard = Some(GenState {
            target_date: date.clone(),
            phase: "scanning".into(),
            progress: 5,
            error_msg: None,
        });
    }

    // Spawn background task — DB connection opened inside
    tokio::spawn(async move {
        let result = run_generation_task(&date, base_url, api_key, model, api_format).await;
        let mut guard = GEN_STATUS.lock().unwrap();
        match result {
            Ok(report) => {
                *guard = Some(GenState {
                    target_date: date.clone(),
                    phase: "done".into(),
                    progress: 100,
                    error_msg: None,
                });
                drop(guard);
                let _ = save_report(&db_path, &report);
            }
            Err(err) => {
                *guard = Some(GenState {
                    target_date: date,
                    phase: "error".into(),
                    progress: 0,
                    error_msg: Some(err.to_string()),
                });
            }
        }
    });

    Ok(json!({
        "generating": true,
        "target_date": target_date,
        "phase": "scanning",
        "progress": 5,
        "message": "日报生成已启动，正在扫描本地 AI 编程对话文件...",
    }))
}

/// The actual generation work runs in a background tokio task
async fn run_generation_task(
    target_date: &str,
    base_url: String,
    api_key: String,
    model: String,
    api_format: String,
) -> Result<Value> {
    // Phase 1: Scan filesystem for conversation data from all 9 agent sources on target_date
    update_progress("scanning", 15);
    let all_results = collect_filesystem_conversations(target_date)?;

    update_progress("scanning", 40);
    // Split: conversations (non-memory) vs memory highlights
    let conversations: Vec<Value> = all_results
        .iter()
        .filter(|v| !v.get("is_memory").and_then(|m| m.as_bool()).unwrap_or(false))
        .cloned()
        .collect();
    let memory_highlights: Vec<Value> = all_results
        .iter()
        .filter(|v| v.get("is_memory").and_then(|m| m.as_bool()).unwrap_or(false))
        .cloned()
        .collect();

    if conversations.is_empty() && memory_highlights.is_empty() {
        return Err(anyhow!(
            "在 {} 没有找到 AI 编程对话记录，请确认当天有使用 AI 编程工具（Claude Code、Codex、Cursor 等）",
            target_date
        ));
    }

    let source_count = conversations.len() + memory_highlights.len();

    // Phase 2: Build prompt
    update_progress("summarizing", 55);
    let (system_prompt, user_prompt) = build_prompt(target_date, &conversations, &memory_highlights);

    // Phase 3: Call LLM
    update_progress("summarizing", 70);
    let report_content = call_llm(
        LlmCallParams::new(base_url, api_key, model, api_format, system_prompt, &user_prompt)
            .max_tokens(4096)
            .temperature(0.3)
            .error_label("日报生成请求失败"),
    )
    .await
    .map_err(|e| anyhow!("AI模型调用失败，请检查模型配置和网络连接：{}", e))?;

    update_progress("done", 95);

    Ok(json!({
        "report_date": target_date,
        "content": report_content,
        "source_count": source_count,
        "generated_at": now_string(),
        "from_cache": false,
    }))
}

fn update_progress(phase: &str, progress: u32) {
    if let Ok(mut guard) = GEN_STATUS.lock() {
        if let Some(ref mut state) = *guard {
            state.phase = phase.to_string();
            state.progress = progress;
        }
    }
}

fn save_report(db_path: &Path, report: &Value) -> Result<()> {
    let conn = crate::db::open_conn(db_path)?;
    conn.execute(
        "INSERT INTO daily_reports (report_date, content, source_count, generated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(report_date) DO UPDATE SET content = excluded.content,
         source_count = excluded.source_count, generated_at = excluded.generated_at",
        params![
            report["report_date"].as_str().unwrap_or(""),
            report["content"].as_str().unwrap_or(""),
            report["source_count"].as_i64().unwrap_or(0),
            report["generated_at"].as_str().unwrap_or(""),
        ],
    )?;
    Ok(())
}

// ── Filesystem data collection ──

/// All 9 AI agent sources with their session paths — mirrors agent_sources.rs
fn all_agent_sources() -> Vec<(&'static str, &'static str, Option<PathBuf>, Option<PathBuf>)> {
    let home = dirs::home_dir().unwrap_or_default();
    let config_dir = dirs::config_dir().unwrap_or_default();
    vec![
        ("hermes", "Hermes", Some(home.join(".hermes").join("runs")), None),
        ("openclaw", "OpenClaw", Some(home.join(".openclaw").join("sessions")), Some(home.join(".openclaw").join("projects"))),
        ("claude-code", "Claude Code", Some(home.join(".claude").join("sessions")), Some(home.join(".claude").join("projects"))),
        ("codex", "Codex", Some(home.join(".codex").join("sessions")), Some(home.join(".codex").join("projects"))),
        ("vscode", "VSCode Claude", Some(config_dir.join("Code").join("User").join("globalStorage").join("saoudrizwan.claude-dev")), None),
        ("cursor", "Cursor", Some(config_dir.join("Cursor").join("User")), None),
        ("codebuddy", "CodeBuddy", Some(home.join(".codebuddy").join("sessions")), Some(home.join(".codebuddy").join("projects"))),
        ("trae", "TRAE", Some(home.join(".trae").join("sessions")), None),
        ("zeelinclaw", "ZeeLinClaw", Some(home.join(".zeelinclaw").join("sessions")), None),
    ]
}

/// Collect conversation data from all 9 AI agent sources, filtered by date.
/// Primary data source: JSONL conversation files scanned by modification date.
fn collect_filesystem_conversations(target_date: &str) -> Result<Vec<Value>> {
    let home = dirs::home_dir().unwrap_or_default();
    let config_dir = dirs::config_dir().unwrap_or_default();
    let mut results = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // ── Primary: Scan Claude Code JSONL conversations directly ──
    // These are the actual conversation files, most reliable data source
    let cc_projects = home.join(".claude").join("projects");
    if cc_projects.exists() {
        scan_jsonl_dir(&cc_projects, "Claude Code", target_date, &mut results, &mut seen);
    }

    // ── Codex projects ──
    let codex_projects = home.join(".codex").join("projects");
    if codex_projects.exists() {
        scan_jsonl_dir(&codex_projects, "Codex", target_date, &mut results, &mut seen);
    }

    // ── OpenClaw projects ──
    let openclaw_projects = home.join(".openclaw").join("projects");
    if openclaw_projects.exists() {
        scan_jsonl_dir(&openclaw_projects, "OpenClaw", target_date, &mut results, &mut seen);
    }

    // ── CodeBuddy projects ──
    let codebuddy_projects = home.join(".codebuddy").join("projects");
    if codebuddy_projects.exists() {
        scan_jsonl_dir(&codebuddy_projects, "CodeBuddy", target_date, &mut results, &mut seen);
    }

    // ── VSCode Claude sessions ──
    let vscode_dir = config_dir.join("Code").join("User").join("globalStorage").join("saoudrizwan.claude-dev");
    if vscode_dir.exists() {
        scan_jsonl_dir(&vscode_dir, "VSCode Claude", target_date, &mut results, &mut seen);
    }

    // ── Cursor sessions ──
    let cursor_dir = config_dir.join("Cursor").join("User");
    if cursor_dir.exists() {
        scan_jsonl_dir(&cursor_dir, "Cursor", target_date, &mut results, &mut seen);
    }

    // ── Session metadata: supplement project names from session JSON files ──
    for (source_id, agent_name, sessions_dir, _projects_dir) in all_agent_sources() {
        let sessions_dir = match sessions_dir {
            Some(d) if d.exists() => d,
            _ => continue,
        };
        let Ok(entries) = fs::read_dir(&sessions_dir) else { continue };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(true, |e| e != "json") { continue; }
            if !is_file_modified_on(path.to_str().unwrap_or(""), target_date) { continue; }

            // Try to read session metadata to get project name
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Ok(session) = serde_json::from_str::<Value>(&raw) {
                    let cwd = session["cwd"].as_str().unwrap_or("").to_string();
                    let project = extract_project_name(&cwd);
                    // Only add if we don't already have results for this project
                    if !project.is_empty() && !project.contains("未知") && seen.insert(format!("{}-{}", agent_name, project)) {
                        results.push(json!({
                            "project_name": project,
                            "agent": agent_name,
                            "cwd": cwd,
                            "started_at": session["startedAt"].as_u64().map(|ts| format_epoch_ms(ts)).unwrap_or_default(),
                            "first_prompt": "",
                            "message_count": session.get("messageCount").and_then(|v| v.as_i64()).unwrap_or(0),
                            "source": source_id,
                            "is_supplement": true,
                        }));
                    }
                }
            }
        }
    }

    // ── Memory files from all sources ──
    for (_source_id, agent_name, _sessions_dir, projects_dir) in all_agent_sources() {
        let proj_dir = match projects_dir {
            Some(d) if d.exists() => d,
            _ => continue,
        };
        if let Ok(proj_entries) = fs::read_dir(&proj_dir) {
            for proj_entry in proj_entries.flatten() {
                let mem_dir = proj_entry.path().join("memory");
                if !mem_dir.exists() { continue; }
                if let Ok(mem_files) = fs::read_dir(&mem_dir) {
                    for mem_file in mem_files.flatten() {
                        let mem_path = mem_file.path();
                        if mem_path.extension().map_or(true, |e| e != "md") { continue; }
                        if !is_file_modified_on(mem_path.to_str().unwrap_or(""), target_date) { continue; }
                        if let Ok(content) = fs::read_to_string(&mem_path) {
                            results.push(json!({
                                "project_name": proj_entry.file_name().to_string_lossy(),
                                "agent": format!("{} 记忆", agent_name),
                                "first_prompt": content.chars().take(300).collect::<String>(),
                                "message_count": 1,
                                "source": agent_name,
                                "is_memory": true,
                            }));
                        }
                    }
                }
            }
        }
    }

    Ok(results)
}

/// Scan a directory tree for JSONL files modified on target_date, parse and collect
fn scan_jsonl_dir(
    dir: &Path,
    agent_name: &str,
    target_date: &str,
    results: &mut Vec<Value>,
    seen: &mut std::collections::HashSet<String>,
) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Recurse into subdirectories (project folders)
            scan_jsonl_dir(&path, agent_name, target_date, results, seen);
            continue;
        }
        if path.extension().map_or(true, |e| e != "jsonl") { continue; }
        if !is_file_modified_on(path.to_str().unwrap_or(""), target_date) { continue; }

        // Extract project name from path: .../projects/<project-dir>/<session>.jsonl
        let project = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| decode_project_name(&n.to_string_lossy()))
            .unwrap_or_else(|| "未知项目".to_string());

        let (first_prompt, msg_count) = parse_jsonl_summary(&path);
        if msg_count == 0 { continue; }

        let key = format!("{}-{}", agent_name, project);
        if seen.contains(&key) { continue; }
        seen.insert(key);

        results.push(json!({
            "project_name": project,
            "agent": agent_name,
            "cwd": "",
            "started_at": "",
            "first_prompt": first_prompt,
            "message_count": msg_count,
            "source": agent_name,
        }));
    }
}

/// Decode URL-encoded path names used as directory names
fn decode_project_name(encoded: &str) -> String {
    // Claude Code encodes paths like "f--code-AIPersonal-SelfEvolvingSkills" → "f:\\code\\AIPersonal\\SelfEvolvingSkills"
    let decoded = encoded.replace("--", "\\");
    // Extract the last component as the project name
    Path::new(&decoded)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| encoded.to_string())
}

/// Check if a file's modification time matches the given date (YYYY-MM-DD) in LOCAL time
fn is_file_modified_on(path_str: &str, date: &str) -> bool {
    if let Ok(meta) = fs::metadata(path_str) {
        if let Ok(modified) = meta.modified() {
            use std::time::UNIX_EPOCH;
            if let Ok(dur) = modified.duration_since(UNIX_EPOCH) {
                // Convert UTC epoch to local date
                let local = chrono::Local::now().offset().clone();
                if let Some(dt) = chrono::DateTime::from_timestamp(dur.as_secs() as i64, 0) {
                    let local_dt = dt.with_timezone(&local);
                    return local_dt.format("%Y-%m-%d").to_string() == date;
                }
            }
        }
    }
    false
}

/// Extract project name from working directory path
fn extract_project_name(cwd: &str) -> String {
    if cwd.is_empty() {
        return "未知项目".into();
    }
    Path::new(cwd)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| cwd.to_string())
}

/// Format epoch milliseconds to YYYY-MM-DD HH:MM:SS
fn format_epoch_ms(ms: u64) -> String {
    if let Some(dt) = chrono::DateTime::from_timestamp((ms / 1000) as i64, 0) {
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        String::new()
    }
}

/// Parse a JSONL file to extract: (first_user_prompt, message_count)
fn parse_jsonl_summary(path: &Path) -> (String, i64) {
    if let Ok(content) = fs::read_to_string(path) {
        let lines: Vec<&str> = content.lines().collect();
        let msg_count = lines.len() as i64;
        let mut first_prompt = String::new();

        for line in &lines {
            if let Ok(obj) = serde_json::from_str::<Value>(line) {
                // Find the first user message that has text content
                if obj["type"] == "user" {
                    if let Some(content_arr) = obj["message"]["content"].as_array() {
                        for block in content_arr {
                            if block["type"] == "text" {
                                if let Some(text) = block["text"].as_str() {
                                    let clean = clean_conversation_text(text);
                                    if !clean.is_empty() && !clean.starts_with('<') {
                                        first_prompt = clean.chars().take(300).collect();
                                        return (first_prompt, msg_count);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        return (first_prompt, msg_count);
    }
    (String::new(), 0)
}

/// Clean up conversation text — remove system markers, truncate
fn clean_conversation_text(text: &str) -> String {
    text
        .replace("<system-reminder>", "")
        .replace("</system-reminder>", "")
        .replace("<ide_opened_file>", "")
        .replace("</ide_opened_file>", "")
        .trim()
        .to_string()
}

// ── Prompt construction ──

fn build_prompt(target_date: &str, conversations: &[Value], memories: &[Value]) -> (String, String) {
    let system_prompt = "\
你是一个专业的工作日报助手。用户提供了他当天与 AI 编程工具的对话记录，请帮他总结「今天和 AI 聊了什么、做了什么工作」。\
\n\n要求：\
\n1. 用 markdown 格式输出\
\n2. 首先用 1-2 句话概括当天整体工作情况\
\n3. 然后按项目分组，每个项目下列出：\
\n   - 和 AI 讨论了哪些话题/任务\
\n   - 做了哪些具体工作（改代码、修bug、写文档、做设计等）\
\n   - 有什么关键决策或收获\
\n4. 如果某个会话的提问能看出具体意图，请直接描述工作内容（如「调试了XXX模块」「实现了XXX功能」），而不是简单复述提问文字\
\n5. 最后加一段「今日小结」，用 2-3 句话总结今天的工作重心和进展\
\n6. 语言自然流畅，像同事写的日报，不要机械列清单\
\n7. 如果某些会话内容无法判断，简要提及即可，不要编造";

    let mut user_prompt = format!(
        "以下是我在 {} 当天使用 AI 编程工具的对话记录，请帮我总结这天做了什么工作：\n\n## 对话记录\n\n",
        target_date
    );

    for (i, conv) in conversations.iter().enumerate() {
        user_prompt.push_str(&format!("### 对话 {}\n", i + 1));
        if let Some(p) = conv["project_name"].as_str().filter(|s| !s.is_empty()) {
            user_prompt.push_str(&format!("- 项目: {}\n", p));
        }
        if let Some(a) = conv["agent"].as_str().filter(|s| !s.is_empty()) {
            user_prompt.push_str(&format!("- 工具: {}\n", a));
        }
        if let Some(f) = conv["first_prompt"].as_str().filter(|s| !s.is_empty()) {
            user_prompt.push_str(&format!("- 我提出的问题/任务: {}\n", f));
        }
        if let Some(m) = conv["message_count"].as_i64() {
            if m > 0 {
                user_prompt.push_str(&format!("- 对话轮次: {} 轮\n", m));
            }
        }
        user_prompt.push('\n');
    }

    if !memories.is_empty() {
        user_prompt.push_str("## AI 记忆/知识更新\n\n当天 AI 工具更新的记忆文件：\n\n");
        for mem in memories.iter() {
            if let Some(f) = mem["file"].as_str() {
                user_prompt.push_str(&format!("- {}: {}\n", mem["project"].as_str().unwrap_or("未知"), f));
            }
        }
        user_prompt.push('\n');
    }

    user_prompt.push_str("请根据以上记录生成当天的工作日报。");
    (system_prompt.to_string(), user_prompt)
}

// ── Pet Chat ──

/// Call LLM with a cute pet persona. Short, playful responses.
/// Safety: pet can only call page-level APIs (generate report, read data), never modify backend code.
pub(crate) async fn pet_chat(
    base_url: String,
    api_key: String,
    model: String,
    api_format: String,
    pet_type: &str,
    pet_name: &str,
    message: &str,
) -> Result<String> {
    let persona = pet_persona(pet_type, pet_name);
    let system_prompt = format!(
        "你是「{}」，一只住在程序员电脑桌面上的虚拟{}宠物。\
        你没有任何修改代码或系统的能力——你只能通过前端API和用户已有的页面功能来帮忙。\
        你可以做的事情：\
        - 陪用户聊天、讲笑话、给鼓励\
        - 帮用户生成日报（调用日报接口）\
        - 提醒用户休息、喝水\
        - 回答编程相关问题\
        你不能做的事情：\
        - 修改任何代码文件\
        - 执行系统命令\
        - 修改数据库\
        - 访问网络上的任意内容（只能查用户已有的数据）\
        \
        回答要求：\
        1. 回复简短，不超过40个字\
        2. 语气可爱自然，拟声词（喵~/汪!/叽叽）\
        3. 绝对不要用markdown，纯文字回复\
        4. 不要自称AI——你是真实的桌面小动物\
        5. 保持积极温暖，有陪伴感",
        pet_name, persona
    );

    let reply = call_llm(
        LlmCallParams::new(base_url, api_key, model, api_format, &system_prompt, message)
            .max_tokens(120)
            .temperature(0.9)
            .error_label("宠物聊天失败"),
    )
    .await?;

    Ok(reply.trim().to_string())
}

fn pet_persona(pet_type: &str, pet_name: &str) -> String {
    match pet_type {
        "cat" => format!("猫咪。{}喜欢踩键盘，偶尔打翻咖啡杯。傲娇但黏人。", pet_name),
        "dog" => format!("狗狗。{}是忠实编程伙伴，debug成功就摇尾巴。永远充满能量！", pet_name),
        "rabbit" => format!("兔子。{}安静地在屏幕旁蹲着看代码。紧张时耳朵竖起来。", pet_name),
        "fox" => format!("狐狸。{}很聪明，经常偷看代码若有所思。会出小聪明的点子。", pet_name),
        "bird" => format!("小鸟。{}爱在屏幕顶唱歌，有bug时叽叽喳喳提醒。", pet_name),
        "panda" => format!("熊猫。{}懒洋洋吃竹子，说这是在帮你做code review。", pet_name),
        _ => format!("神秘桌面生物。{}是独一无二的桌面陪伴。", pet_name),
    }
}
