use anyhow::Result;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::path::Path;

use crate::db;
use crate::utils::time::now_string;

use super::PHASES;

pub(crate) fn has_incomplete_run(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM evolution_jobs WHERE status IN ('running', 'pending')",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}

pub(crate) fn cleanup_stale_jobs(conn: &Connection) -> i64 {
    let now = now_string();
    let phase_timeouts: &[(&str, i64)] = &[
        ("discover", 5),
        ("reference_retrieval", 8),
        ("cluster", 8),
        ("draft_generate", 3),
        ("optimize", 20),
        ("qa_review", 25),
        ("diff_recommend", 5),
    ];
    let mut total = 0i64;
    for (phase, timeout_min) in phase_timeouts {
        let sql = format!(
            "UPDATE evolution_jobs
             SET status = 'failed', completed_at = ?1,
                 message = COALESCE(message, '') || ' [超时自动标记为失败]'
             WHERE phase = ?2 AND status IN ('running', 'pending')
               AND started_at IS NOT NULL
               AND datetime(started_at, '+{timeout_min} minutes') < datetime(?1)"
        );
        if let Ok(count) = conn.execute(&sql, params![now, phase]) {
            total += count as i64;
        }
    }
    total
}

pub(super) fn heartbeat_phase(db_path: &Path, run_id: i64, phase: &str, message: &str) {
    if let Ok(conn) = db::open_conn(db_path) {
        let now = now_string();
        let _ = conn.execute(
            "UPDATE evolution_jobs SET started_at = ?2, message = ?3
             WHERE run_id = ?1 AND phase = ?4 AND status = 'running'",
            params![run_id, now, message, phase],
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn record_skill_iteration(
    db_path: &Path,
    skill_name: &str,
    iteration_num: i64,
    previous_draft: Option<&str>,
    new_draft: &str,
    trigger_reason: &str,
    judge_result: Option<&str>,
    critic_result: Option<&str>,
    improvement_summary: Option<&str>,
    run_id: i64,
) {
    if let Ok(conn) = db::open_conn(db_path) {
        let now = now_string();
        let _ = conn.execute(
            "INSERT INTO skill_iterations (skill_name, iteration_num, previous_draft, new_draft,
                 status, trigger_reason, judge_result, critic_result, improvement_summary, run_id,
                 created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'completed', ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                skill_name,
                iteration_num,
                previous_draft,
                new_draft,
                trigger_reason,
                judge_result,
                critic_result,
                improvement_summary,
                run_id,
                now,
            ],
        );
    }
}

pub(crate) fn reset_stuck_evolution(conn: &Connection) -> Value {
    let now = now_string();
    let stuck: i64 = conn
        .execute(
            "UPDATE evolution_jobs
             SET status = 'failed', completed_at = ?1, message = COALESCE(message, '') || ' [已手动重置]'
             WHERE status IN ('running', 'pending')",
            params![now],
        )
        .unwrap_or(0) as i64;
    json!({ "reset": stuck, "message": format!("已重置 {} 个卡住的作业", stuck) })
}

pub(crate) fn get_evolution_status(conn: &Connection) -> Value {
    let auto_failed = cleanup_stale_jobs(conn);

    let steps: Vec<Value> = PHASES
        .iter()
        .map(|&(phase, label, start, end)| {
            json!({ "phase": phase, "label": label, "start": start, "end": end })
        })
        .collect();

    let latest_run: Option<i64> = conn
        .query_row("SELECT MAX(run_id) FROM evolution_jobs", [], |row| {
            row.get(0)
        })
        .ok();

    match latest_run {
        Some(run_id) => {
            let phases = get_run_phases(conn, run_id).unwrap_or_else(|e| {
                log::warn!("Failed to get run phases for run_id {run_id}: {e}");
                Vec::new()
            });
            let has_active = phases
                .iter()
                .any(|p| p["status"] == "running" || p["status"] == "pending");
            let current = phases.iter().find(|p| p["status"] == "running");
            let all_done = phases.iter().all(|p| p["status"] == "completed");
            let any_failed = phases.iter().any(|p| p["status"] == "failed");
            json!({
                "run_id": run_id,
                "phases": phases,
                "steps": steps,
                "running": has_active,
                "current_phase": current.map(|p| p["phase"].clone()).unwrap_or(Value::Null),
                "last_completed": if all_done { get_last_completed(conn) } else { None::<Value> },
                "any_failed": any_failed,
                "auto_failed": auto_failed,
            })
        }
        None => json!({
            "running": false,
            "steps": steps,
            "last_completed": Value::Null,
        }),
    }
}

fn get_run_phases(conn: &Connection, run_id: i64) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT id, phase, status, progress, message, started_at, completed_at
             FROM evolution_jobs WHERE run_id = ?1 ORDER BY id",
    )?;
    let rows: Vec<Value> = stmt
        .query_map(params![run_id], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "phase": row.get::<_, String>(1)?,
                "status": row.get::<_, String>(2)?,
                "progress": row.get::<_, i64>(3)?,
                "message": row.get::<_, Option<String>>(4)?,
                "started_at": row.get::<_, Option<String>>(5)?,
                "completed_at": row.get::<_, Option<String>>(6)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

fn get_last_completed(conn: &Connection) -> Option<Value> {
    conn.query_row(
        "SELECT run_id, MAX(completed_at) FROM evolution_jobs WHERE status = 'completed' GROUP BY run_id ORDER BY run_id DESC LIMIT 1",
        [],
        |row| {
            Ok(json!({
                "run_id": row.get::<_, i64>(0)?,
                "completed_at": row.get::<_, Option<String>>(1)?,
            }))
        },
    )
    .ok()
}

pub(crate) fn list_evolution_history(conn: &Connection, query: &crate::PageQuery) -> Result<Value> {
    let page = query.page.unwrap_or(1).max(1);
    let size = query.size.unwrap_or(20).clamp(1, 100);

    let total: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT run_id) FROM evolution_jobs",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let mut stmt = conn.prepare(
        "SELECT run_id FROM evolution_jobs GROUP BY run_id ORDER BY run_id DESC LIMIT ?1 OFFSET ?2",
    )?;
    let run_ids: Vec<i64> = stmt
        .query_map(params![size, (page - 1) * size], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    let items: Vec<Value> = run_ids
        .iter()
        .map(|&run_id| {
            let phases = get_run_phases(conn, run_id).unwrap_or_else(|e| {
                log::warn!("Failed to get run phases for run_id {run_id}: {e}");
                Vec::new()
            });
            let first = phases.first();
            let last = phases.last();
            let all_done = phases.iter().all(|p| p["status"] == "completed");
            let any_failed = phases.iter().any(|p| p["status"] == "failed");
            let run_status = if all_done {
                "completed"
            } else if any_failed {
                "failed"
            } else {
                "running"
            };
            json!({
                "run_id": run_id,
                "status": run_status,
                "phases": phases,
                "started_at": first.and_then(|p| p["started_at"].as_str().map(|s| s.to_string())),
                "completed_at": last.and_then(|p| p["completed_at"].as_str().map(|s| s.to_string())),
            })
        })
        .collect();

    Ok(json!({ "items": items, "total": total, "page": page, "size": size }))
}
