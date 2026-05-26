mod parsers;

pub(crate) use parsers::parse_skill_file;

use crate::{
    agent_sources, count_records, format_system_time, now_string, source_default_paths, AgentInfo,
    SessionInfo, SkillInfo, SourcePaths, SourcePayload, SourceUpdate,
};
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::{collections::HashMap, fs, path::Path};
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;

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

fn existing_custom_paths(
    conn: &Connection,
    agent_id: &str,
) -> Result<Option<HashMap<String, Option<String>>>> {
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

    for key in [
        "sessions_path",
        "projects_path",
        "skills_path",
        "extra_skills_path",
        "agents_path",
        "extra_agents_path",
    ] {
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

    Ok((
        available,
        detected_path,
        count,
        latest.map(format_system_time),
    ))
}

pub(crate) fn list_sources(conn: &Connection) -> Result<Vec<SourcePayload>> {
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
            if let Ok(custom) =
                serde_json::from_str::<HashMap<String, Option<String>>>(&custom_paths)
            {
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
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub(crate) fn update_source(
    conn: &Connection,
    agent_id: &str,
    payload: SourceUpdate,
) -> Result<Value> {
    let mut current_custom = existing_custom_paths(conn, agent_id)?.unwrap_or_default();
    if let Some(paths) = payload.paths {
        current_custom = paths
            .into_iter()
            .filter(|(_, value)| value.as_ref().is_some_and(|v| !v.trim().is_empty()))
            .collect();
        conn.execute(
            "UPDATE source_configs SET custom_paths = ?2, updated_at = ?3 WHERE agent_id = ?1",
            params![
                agent_id,
                serde_json::to_string(&current_custom)?,
                now_string()
            ],
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

pub(crate) fn reset_source(conn: &Connection, agent_id: &str) -> Result<()> {
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

pub(crate) fn run_full_scan(
    conn: &Connection,
    app: &AppHandle,
    agent_ids: Option<&[String]>,
) -> Result<Value> {
    sync_source_configs(conn)?;
    let selected = agent_ids.unwrap_or(&[]);
    let sources = enabled_source_paths(conn)?
        .into_iter()
        .filter(|source| selected.is_empty() || selected.iter().any(|id| id == &source.agent_id))
        .collect::<Vec<_>>();
    let started_at = now_string();
    let config_snapshot =
        serde_json::to_string(&sources.iter().map(|s| &s.agent_id).collect::<Vec<_>>())?;

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
                    for entry in WalkDir::new(base)
                        .min_depth(1)
                        .max_depth(max_depth)
                        .into_iter()
                        .flatten()
                    {
                        if entry.file_type().is_file() && entry.file_name() == "SKILL.md" {
                            if let Some(info) = parsers::parse_skill_file(
                                entry.path(),
                                "user",
                                &source.agent_id,
                                None,
                            ) {
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
                for entry in WalkDir::new(base)
                    .min_depth(1)
                    .max_depth(6)
                    .into_iter()
                    .flatten()
                {
                    if entry.file_type().is_file() && entry.file_name() == "SKILL.md" {
                        let plugin_name = entry
                            .path()
                            .ancestors()
                            .nth(3)
                            .and_then(|p| p.file_name())
                            .map(|s| s.to_string_lossy().to_string());
                        if let Some(info) = parsers::parse_skill_file(
                            entry.path(),
                            "plugin",
                            &source.agent_id,
                            plugin_name,
                        ) {
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
                for entry in WalkDir::new(base)
                    .min_depth(1)
                    .max_depth(max_depth)
                    .into_iter()
                    .flatten()
                {
                    let path = entry.path();
                    if entry.file_type().is_file() && parsers::is_agent_definition_file(path) {
                        if let Some(info) = parsers::parse_agent_file(path, &source.agent_id) {
                            results.push(info);
                        }
                    }
                }
            }
        }
    }
    results
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
            for entry in WalkDir::new(base)
                .max_depth(max_depth)
                .into_iter()
                .flatten()
            {
                if !entry.file_type().is_file() {
                    continue;
                }
                let ext = entry.path().extension().and_then(|s| s.to_str());
                let info = match ext {
                    Some("json") => {
                        parsers::parse_session_file(entry.path(), &source.agent_id, sources)
                    }
                    Some("jsonl") => {
                        parsers::parse_session_jsonl_file(entry.path(), &source.agent_id)
                    }
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
              started_at, message_count, first_prompt, compressed_summary, jsonl_path, jsonl_size,
              matched_skill, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?16)
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
              matched_skill = excluded.matched_skill,
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
                session.matched_skill,
                now_string()
            ],
        )?;
    }
    refresh_skill_usage_from_sessions(conn)?;
    Ok(sessions.len() as i64)
}

pub(crate) fn refresh_skill_usage_from_sessions(conn: &Connection) -> Result<i64> {
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
    for (session_id, agent_source, first_prompt, compressed_summary, jsonl_path, started_at) in
        sessions
    {
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
                haystack.push_str(&crate::truncate_chars(&text, 200_000));
            }
        }
        if haystack.trim().is_empty() {
            continue;
        }

        let mut best_skill: Option<(&str, usize)> = None;
        for skill in &skills {
            if let Some(session_agent) = &agent_source {
                if let Some(skill_agent) = &skill.agent_source {
                    if session_agent != skill_agent {
                        continue;
                    }
                }
            }
            let mention_count = crate::count_skill_mentions(&haystack, &skill.name);
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
            if mention_count > best_skill.map(|(_, c)| c).unwrap_or(0) {
                best_skill = Some((&skill.name, mention_count));
            }
        }
        if let Some((skill_name, _)) = best_skill {
            let _ = conn.execute(
                "UPDATE sessions SET matched_skill = ?1 WHERE session_id = ?2",
                params![skill_name, session_id],
            );
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
         ), 0),
         effectiveness_score = COALESCE((
             SELECT AVG(mention_count) * 10.0 FROM skill_usage
             WHERE skill_usage.skill_name = skills.name
               AND skill_usage.skill_source_type = skills.source_type
         ), 0),
         last_evaluated_at = ?1",
        params![now_string()],
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
