use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use tauri::{AppHandle, Emitter};

use crate::db;
use crate::services::community_service;
use crate::services::workflow_service::{self, WorkflowCluster};
use crate::utils::time::now_string;

#[derive(Debug, Clone)]
struct LlmConfig {
    base_url: String,
    api_key: String,
    model: String,
    api_format: String,
}

#[derive(Debug, Deserialize)]
struct LlmClusterResponse {
    clusters: Vec<LlmClusterDecision>,
}

#[derive(Debug, Deserialize)]
struct LlmClusterDecision {
    name: String,
    new_name: Option<String>,
    description: Option<String>,
    skill_score: Option<i64>,
    can_generate_skill: Option<bool>,
    merge_into: Option<String>,
    reasoning: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SkillReviewResponse {
    name: Option<String>,
    description: Option<String>,
    draft_body: Option<String>,
    score: Option<i64>,
    verdict: Option<String>,
    summary: Option<String>,
    safety: Option<Vec<String>>,
    performance: Option<Vec<String>>,
    functionality: Option<Vec<String>>,
    writing: Option<Vec<String>>,
    improvements: Option<Vec<String>>,
}

const PHASES: &[(&str, &str, i64, i64)] = &[
    ("discover", "扫描发现", 0, 20),
    ("reference_retrieval", "参考检索", 20, 32),
    ("cluster", "聚类分析", 32, 50),
    ("draft_generate", "生成草稿", 50, 62),
    ("optimize", "智能优化", 62, 78),
    ("qa_review", "质量评审", 78, 90),
    ("diff_recommend", "差异推荐", 90, 100),
];

pub(crate) fn has_incomplete_run(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM evolution_jobs WHERE status IN ('running', 'pending')",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}

/// Auto-fail jobs that have been running or pending for more than 10 minutes.
pub(crate) fn cleanup_stale_jobs(conn: &Connection) -> i64 {
    let now = now_string();
    conn.execute(
        "UPDATE evolution_jobs
         SET status = 'failed', completed_at = ?1,
             message = COALESCE(message, '') || ' [超时自动标记为失败]'
         WHERE status IN ('running', 'pending')
           AND started_at IS NOT NULL
           AND datetime(started_at, '+10 minutes') < datetime(?1)",
        params![now],
    )
    .unwrap_or(0) as i64
}

/// Mark all running/pending jobs in the latest run as 'failed'.
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
    // Auto-detect stale runs: running for > 10 minutes without completion
    let auto_failed = cleanup_stale_jobs(conn);

    let steps: Vec<Value> = PHASES
        .iter()
        .map(|&(phase, label, start, end)| {
            json!({ "phase": phase, "label": label, "start": start, "end": end })
        })
        .collect();

    // Always look for the latest run (any status), so the UI can show phase
    // state even after the pipeline completes or fails.
    let latest_run: Option<i64> = conn
        .query_row("SELECT MAX(run_id) FROM evolution_jobs", [], |row| {
            row.get(0)
        })
        .ok();

    match latest_run {
        Some(run_id) => {
            let phases = get_run_phases(conn, run_id).unwrap_or_default();
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
    let mut stmt = conn
        .prepare(
            "SELECT id, phase, status, progress, message, started_at, completed_at
             FROM evolution_jobs WHERE run_id = ?1 ORDER BY id",
        )?;
    let rows: Vec<Value> = stmt.query_map(params![run_id], |row| {
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

    let mut stmt = conn
        .prepare(
            "SELECT run_id FROM evolution_jobs GROUP BY run_id ORDER BY run_id DESC LIMIT ?1 OFFSET ?2",
        )?;
    let run_ids: Vec<i64> = stmt
        .query_map(params![size, (page - 1) * size], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    let items: Vec<Value> = run_ids
        .iter()
        .map(|&run_id| {
            let phases = get_run_phases(conn, run_id).unwrap_or_default();
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

pub(crate) fn start_evolution_pipeline(
    conn: &Connection,
    app: &AppHandle,
    db_path: &Path,
    scan_lock: std::sync::Arc<tokio::sync::Mutex<()>>,
    agent_ids: Option<Vec<String>>,
) -> Result<Value> {
    let started_at = now_string();
    let next_run_id: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(run_id), 0) + 1 FROM evolution_jobs",
            [],
            |row| row.get(0),
        )
        .unwrap_or(1);

    for &(phase, _label, progress, _end) in PHASES {
        conn.execute(
            "INSERT INTO evolution_jobs (run_id, phase, status, progress, message, started_at, created_at)
             VALUES (?1, ?2, 'pending', ?3, '', ?4, ?4)",
            params![next_run_id, phase, progress, started_at],
        )?;
    }

    // Insert evolution_runs and evolution_steps for FK tracing
    let selected_agent_ids = agent_ids.unwrap_or_default();
    conn.execute(
        "INSERT INTO evolution_runs (status, agent_ids, started_at) VALUES ('running', ?1, ?2)",
        params![
            serde_json::to_string(&selected_agent_ids).unwrap_or_default(),
            started_at
        ],
    )?;
    let run_row_id = conn.last_insert_rowid();

    let mut step_ids: std::collections::HashMap<&str, i64> = std::collections::HashMap::new();
    for &(phase, _label, _progress, _end) in PHASES {
        conn.execute(
            "INSERT INTO evolution_steps (run_id, phase, status, started_at) VALUES (?1, ?2, 'pending', ?3)",
            params![run_row_id, phase, started_at],
        )?;
        step_ids.insert(phase, conn.last_insert_rowid());
    }

    // 清理上一次进化管道的推荐内容，但保留已安装和手动创建的
    let _ = conn.execute("DELETE FROM workflow_clusters WHERE status != 'installed' AND status != 'manual-draft' AND recommendation_source != 'manual-existing-skill'", []);
    let _ = conn.execute("DELETE FROM community_skills", []);

    let app_handle = app.clone();
    let db_path_clone = db_path.to_path_buf();
    let scan_lock = scan_lock.clone();

    tokio::spawn(async move {
        let mut llm_error: Option<String> = None;

        // =====================================================================
        // Phase 1: discover (0-20%) — 合并旧 detect + scan，单次 DB 连接
        // =====================================================================
        {
            phase_update(
                &app_handle,
                &db_path_clone,
                next_run_id,
                "discover",
                3,
                "正在检测已安装的 AI 编程助手...",
            );
            let conn2 = match db::open_conn(&db_path_clone) {
                Ok(c) => c,
                Err(e) => {
                    phase_update(
                        &app_handle,
                        &db_path_clone,
                        next_run_id,
                        "discover",
                        20,
                        &format!("数据库打开失败：{e}"),
                    );
                    emit_only(
                        &app_handle,
                        next_run_id,
                        "discover",
                        100,
                        &format!("进化管道中止：数据库错误 — {e}"),
                    );
                    return;
                }
            };
            if let Err(e) = crate::sync_source_configs(&conn2) {
                phase_update(
                    &app_handle,
                    &db_path_clone,
                    next_run_id,
                    "discover",
                    6,
                    &format!("检测失败：{}", e),
                );
            }
            let _guard = scan_lock.lock().await;
            let sources = filter_sources(
                crate::enabled_source_paths(&conn2).unwrap_or_default(),
                &selected_agent_ids,
            );
            phase_update(
                &app_handle,
                &db_path_clone,
                next_run_id,
                "discover",
                10,
                &format!("检测完成，本次将扫描 {} 个 AI 助手", sources.len()),
            );

            let skills = crate::scan_skills(&sources);
            phase_update(
                &app_handle,
                &db_path_clone,
                next_run_id,
                "discover",
                14,
                &format!("发现 {} 个 Skills", skills.len()),
            );
            let _ = crate::upsert_skills(&conn2, &skills);

            let agents = crate::scan_agents(&sources);
            phase_update(
                &app_handle,
                &db_path_clone,
                next_run_id,
                "discover",
                17,
                &format!("发现 {} 个 Agents", agents.len()),
            );
            let _ = crate::upsert_agents(&conn2, &agents);

            let sessions = crate::scan_sessions(&sources);
            phase_update(
                &app_handle,
                &db_path_clone,
                next_run_id,
                "discover",
                20,
                &format!("发现 {} 条历史会话，已生成本地压缩摘要", sessions.len()),
            );
            let _ = crate::upsert_sessions(&conn2, &sessions);
            drop(_guard);
            drop(conn2);
        }

        // =====================================================================
        // Phase 2: reference_retrieval (20-32%) — 社区检索前置到聚类前
        // =====================================================================
        phase_update(
            &app_handle,
            &db_path_clone,
            next_run_id,
            "reference_retrieval",
            22,
            "正在检索社区参考 Skills（GitHub / Web / 本地）...",
        );
        let community_error =
            match community_service::fetch_top_community_skills(&db_path_clone, &[]).await {
                Ok(count) => {
                    phase_update(
                        &app_handle,
                        &db_path_clone,
                        next_run_id,
                        "reference_retrieval",
                        32,
                        &format!("社区检索完成，新增 {} 条参考结果", count),
                    );
                    None
                }
                Err(err) => {
                    let message = format!("社区检索失败：{}。管道将继续执行。", err);
                    phase_update(
                        &app_handle,
                        &db_path_clone,
                        next_run_id,
                        "reference_retrieval",
                        32,
                        &message,
                    );
                    Some(message)
                }
            };

        // =====================================================================
        // Phase 3: cluster (32-50%) — 聚类 + LLM 复核 + 保存
        // =====================================================================
        let conn3 = match db::open_conn(&db_path_clone) {
            Ok(c) => c,
            Err(e) => {
                phase_update(
                    &app_handle,
                    &db_path_clone,
                    next_run_id,
                    "cluster",
                    50,
                    &format!("数据库打开失败：{e}"),
                );
                emit_only(
                    &app_handle,
                    next_run_id,
                    "cluster",
                    100,
                    &format!("进化管道中止：数据库错误 — {e}"),
                );
                return;
            }
        };
        phase_update(
            &app_handle,
            &db_path_clone,
            next_run_id,
            "cluster",
            34,
            "正在用本地算法聚类重复工作流...",
        );
        let mut clusters = workflow_service::cluster_workflows(&conn3);
        drop(conn3);

        phase_update(
            &app_handle,
            &db_path_clone,
            next_run_id,
            "cluster",
            42,
            &format!(
                "本地聚类完成，识别到 {} 个候选工作流，正在大模型复核...",
                clusters.len()
            ),
        );
        match refine_clusters_with_llm(&db_path_clone, &mut clusters).await {
            Ok(true) => phase_update(
                &app_handle,
                &db_path_clone,
                next_run_id,
                "cluster",
                48,
                "大模型已完成工作流复核与重排序",
            ),
            Ok(false) => phase_update(
                &app_handle,
                &db_path_clone,
                next_run_id,
                "cluster",
                48,
                "未启用大模型，使用本地聚类结果",
            ),
            Err(err) => {
                let message = format!("大模型复核失败：{}。已回退使用本地聚类结果。", err);
                llm_error = Some(message.clone());
                phase_update(
                    &app_handle,
                    &db_path_clone,
                    next_run_id,
                    "cluster",
                    48,
                    &message,
                );
            }
        }
        workflow_service::make_cluster_names_unique(&mut clusters);

        let conn4 = match db::open_conn(&db_path_clone) {
            Ok(c) => c,
            Err(e) => {
                phase_update(
                    &app_handle,
                    &db_path_clone,
                    next_run_id,
                    "cluster",
                    50,
                    &format!("数据库打开失败：{e}"),
                );
                emit_only(
                    &app_handle,
                    next_run_id,
                    "cluster",
                    100,
                    &format!("进化管道中止：数据库错误 — {e}"),
                );
                return;
            }
        };
        let _ = workflow_service::save_clusters(&conn4, &clusters);
        phase_update(
            &app_handle,
            &db_path_clone,
            next_run_id,
            "cluster",
            50,
            "聚类结果已保存",
        );

        // =====================================================================
        // Phase 4: draft_generate (50-62%) — 纯本地模板生成，不做 LLM 调用
        // =====================================================================
        phase_update(
            &app_handle,
            &db_path_clone,
            next_run_id,
            "draft_generate",
            52,
            "正在基于模板生成 Skill 草稿（纯本地）...",
        );
        let drafts = workflow_service::generate_skill_drafts(&conn4, &clusters);
        drop(conn4);
        phase_update(
            &app_handle,
            &db_path_clone,
            next_run_id,
            "draft_generate",
            62,
            &format!("已生成 {} 个本地 Skill 草稿", drafts),
        );

        // =====================================================================
        // Phase 5: optimize (62-78%) — LLM 逐条优化草稿，注入社区参考
        // =====================================================================
        phase_update(
            &app_handle,
            &db_path_clone,
            next_run_id,
            "optimize",
            64,
            "正在用大模型逐条优化草稿...",
        );
        let optimized = match optimize_drafts_with_llm(&db_path_clone, &clusters).await {
            Ok(count) => {
                if count > 0 {
                    phase_update(
                        &app_handle,
                        &db_path_clone,
                        next_run_id,
                        "optimize",
                        78,
                        &format!("大模型已优化 {} 个 Skill 草稿", count),
                    );
                } else {
                    phase_update(
                        &app_handle,
                        &db_path_clone,
                        next_run_id,
                        "optimize",
                        78,
                        "未启用大模型，使用本地草稿",
                    );
                }
                count
            }
            Err(err) => {
                let message = format!("大模型草稿优化失败：{}。已保留本地草稿。", err);
                llm_error = Some(match llm_error {
                    Some(existing) => format!("{existing}；{message}"),
                    None => message.clone(),
                });
                phase_update(
                    &app_handle,
                    &db_path_clone,
                    next_run_id,
                    "optimize",
                    78,
                    &message,
                );
                0
            }
        };

        // ---- A/B Variant Generation (within optimize phase) ----
        let ab_count = match generate_ab_variants(&db_path_clone, &clusters).await {
            Ok(count) => {
                if count > 0 {
                    phase_update(
                        &app_handle,
                        &db_path_clone,
                        next_run_id,
                        "optimize",
                        77,
                        &format!("已生成 {} 个 A/B 变体", count),
                    );
                }
                count
            }
            Err(_) => 0,
        };

        // =====================================================================
        // Phase 6: qa_review (78-90%) — 独立 QA Agent + guardrails
        // =====================================================================
        phase_update(
            &app_handle,
            &db_path_clone,
            next_run_id,
            "qa_review",
            80,
            "Skill Review Agent 正在评审草稿...",
        );
        let reviewed = match qa_drafts_with_llm(&db_path_clone, &mut clusters).await {
            Ok(count) => {
                if count > 0 {
                    phase_update(
                        &app_handle,
                        &db_path_clone,
                        next_run_id,
                        "qa_review",
                        90,
                        &format!("Skill Review Agent 已评审 {} 个草稿", count),
                    );
                } else {
                    phase_update(
                        &app_handle,
                        &db_path_clone,
                        next_run_id,
                        "qa_review",
                        90,
                        "未启用大模型，跳过 QA 评审",
                    );
                }
                count
            }
            Err(err) => {
                let message = format!(
                    "Skill Review Agent 评审失败：{}。已保留草稿等待人工审核。",
                    err
                );
                llm_error = Some(match llm_error {
                    Some(existing) => format!("{existing}；{message}"),
                    None => message.clone(),
                });
                phase_update(
                    &app_handle,
                    &db_path_clone,
                    next_run_id,
                    "qa_review",
                    90,
                    &message,
                );
                0
            }
        };

        // =====================================================================
        // Phase 7: diff_recommend (90-100%) — 差异对比 + 可操作建议
        // =====================================================================
        let conn5 = match db::open_conn(&db_path_clone) {
            Ok(c) => c,
            Err(e) => {
                phase_update(
                    &app_handle,
                    &db_path_clone,
                    next_run_id,
                    "diff_recommend",
                    100,
                    &format!("数据库打开失败：{e}"),
                );
                emit_only(
                    &app_handle,
                    next_run_id,
                    "diff_recommend",
                    100,
                    &format!("进化管道中止：数据库错误 — {e}"),
                );
                return;
            }
        };
        phase_update(
            &app_handle,
            &db_path_clone,
            next_run_id,
            "diff_recommend",
            92,
            "正在对比本地草稿与社区 Skills...",
        );
        let comparisons = community_service::compare_with_community(&conn5, &clusters);
        phase_update(
            &app_handle,
            &db_path_clone,
            next_run_id,
            "diff_recommend",
            96,
            &format!("差异对比完成，{} 个工作流有社区参考", comparisons),
        );

        // 生成简单的推荐建议（P0: 规则启发式，P1 升级为 LLM Agent）
        let diff_step_id = step_ids["diff_recommend"];
        // Mark the evolution_step as running
        let _ = conn5.execute(
            "UPDATE evolution_steps SET status = 'running' WHERE id = ?1",
            params![diff_step_id],
        );
        generate_recommendations(&conn5, diff_step_id, &clusters);
        // Mark the evolution_step as completed
        let _ = conn5.execute(
            "UPDATE evolution_steps SET status = 'completed', completed_at = ?2 WHERE id = ?1",
            params![diff_step_id, now_string()],
        );
        phase_update(
            &app_handle,
            &db_path_clone,
            next_run_id,
            "diff_recommend",
            100,
            "推荐建议已生成",
        );

        let completed_at = now_string();
        let _ = conn5.execute(
            "UPDATE evolution_jobs SET status = 'completed', progress = 100, message = '进化流程完成', completed_at = ?2, data = ?3 WHERE run_id = ?1 AND phase = 'diff_recommend'",
            params![next_run_id, &completed_at, serde_json::to_string(&json!({
                "clusters": clusters.len(),
                "drafts": drafts,
                "comparisons": comparisons,
                "optimized": optimized,
                "reviewed": reviewed,
                "ab_variants": ab_count,
                "agent_ids": selected_agent_ids,
                "llm_error": llm_error,
                "community_error": community_error,
            })).unwrap_or_default()],
        );
        // Mark all evolution_steps as completed and close the evolution_run
        let _ = conn5.execute(
            "UPDATE evolution_steps SET status = 'completed', completed_at = ?2 WHERE run_id = ?1",
            params![run_row_id, &completed_at],
        );
        let _ = conn5.execute(
            "UPDATE evolution_runs SET status = 'completed', completed_at = ?2, summary = ?3 WHERE id = ?1",
            params![run_row_id, &completed_at, serde_json::to_string(&json!({
                "clusters": clusters.len(),
                "drafts": drafts,
                "comparisons": comparisons,
                "optimized": optimized,
                "reviewed": reviewed,
                "ab_variants": ab_count,
            })).unwrap_or_default()],
        );

        emit_only(
            &app_handle,
            next_run_id,
            "completed",
            100,
            "自进化流程完成，请在 Skills 工作台查看结果",
        );
    });

    Ok(json!({ "run_id": next_run_id, "status": "running", "started_at": started_at }))
}

fn filter_sources(
    sources: Vec<crate::SourcePaths>,
    selected_agent_ids: &[String],
) -> Vec<crate::SourcePaths> {
    if selected_agent_ids.is_empty() {
        return sources;
    }
    sources
        .into_iter()
        .filter(|source| selected_agent_ids.iter().any(|id| id == &source.agent_id))
        .collect()
}

fn phase_update(
    app: &AppHandle,
    db_path: &Path,
    run_id: i64,
    phase: &str,
    progress: i64,
    message: &str,
) {
    if let Ok(conn) = db::open_conn(db_path) {
        let now = now_string();
        let _ = conn.execute(
            "UPDATE evolution_jobs SET status = CASE WHEN status = 'pending' THEN 'running' ELSE status END,
             progress = ?3, message = ?4, started_at = COALESCE(started_at, ?5)
             WHERE run_id = ?1 AND phase = ?2",
            params![run_id, phase, progress, message, now],
        );
        let _ = conn.execute(
            "UPDATE evolution_jobs SET status = 'completed',
             progress = CASE phase
               WHEN 'discover' THEN 20
               WHEN 'reference_retrieval' THEN 32
               WHEN 'cluster' THEN 50
               WHEN 'draft_generate' THEN 62
               WHEN 'optimize' THEN 78
               WHEN 'qa_review' THEN 90
               WHEN 'diff_recommend' THEN 100
               ELSE progress
             END,
             completed_at = ?3
             WHERE run_id = ?1 AND id < (SELECT id FROM evolution_jobs WHERE run_id = ?1 AND phase = ?2)",
            params![run_id, phase, now],
        );
        if progress
            >= PHASES
                .iter()
                .find(|&&(p, ..)| p == phase)
                .map(|&(_, _, _, e)| e)
                .unwrap_or(progress)
        {
            let _ = conn.execute(
                "UPDATE evolution_jobs SET status = 'completed', completed_at = ?3
                 WHERE run_id = ?1 AND phase = ?2",
                params![run_id, phase, now],
            );
        }
    }
    let _ = app.emit(
        "evolution-progress",
        json!({
            "run_id": run_id,
            "phase": phase,
            "progress": progress,
            "message": message,
        }),
    );
}

fn emit_only(app: &AppHandle, run_id: i64, phase: &str, progress: i64, message: &str) {
    let _ = app.emit(
        "evolution-progress",
        json!({
            "run_id": run_id,
            "phase": phase,
            "progress": progress,
            "message": message,
        }),
    );
}

async fn refine_clusters_with_llm(
    db_path: &Path,
    clusters: &mut [WorkflowCluster],
) -> Result<bool> {
    if clusters.is_empty() {
        return Ok(false);
    }
    if llm_config(db_path)?.is_none() {
        return Ok(false);
    }

    let payload = clusters
        .iter()
        .take(12)
        .map(|cluster| {
            json!({
                "name": cluster.name,
                "description": cluster.description,
                "frequency": cluster.frequency,
                "source_agents": cluster.source_agents,
                "skill_score": cluster.skill_score,
                "sample_tasks": cluster.sample_tasks,
                "llm_naming_contract": {
                    "new_name": "optional concrete kebab-case object-action-output skill name",
                    "merge_into": "optional common reusable skill name when this cluster overlaps another one",
                    "avoid_names": ["frontend-ui", "code-review", "data-processing", "document-writing", "automation-workflow"]
                },
            })
        })
        .collect::<Vec<_>>();

    let content = call_llm(
        db_path,
        "你是一个 AI 编程 Agent 的 Skill 进化分析器。只基于用户提供的压缩摘要和候选聚类判断，不要编造未出现的会话内容。",
        &format!(
            "请复核这些本地聚类出来的重复工作流，判断哪些值得生成 Skill，并优化名称、描述和评分。\n\
             只返回 JSON，不要 Markdown。格式：{{\"clusters\":[{{\"name\":\"必须等于输入中的原 name\",\"description\":\"中文描述\",\"skill_score\":0-100,\"can_generate_skill\":true,\"reasoning\":\"简短中文理由\"}}]}}\n\
             候选：{}",
            serde_json::to_string(&payload)?
        ),
    )
    .await?;

    let parsed: LlmClusterResponse = serde_json::from_str(&extract_json_object(&content)?)?;
    for decision in parsed.clusters {
        if let Some(cluster) = clusters
            .iter_mut()
            .find(|cluster| cluster.name == decision.name)
        {
            if let Some(new_name) = decision.new_name.filter(|value| !value.trim().is_empty()) {
                cluster.name = sanitize_skill_name(&new_name);
            }
            if let Some(description) = decision
                .description
                .filter(|value| !value.trim().is_empty())
            {
                cluster.description = description;
            }
            if let Some(score) = decision.skill_score {
                cluster.skill_score = score.clamp(0, 100);
            }
            if let Some(can_generate) = decision.can_generate_skill {
                cluster.can_generate_skill = can_generate;
            }
            if let Some(merge_into) = decision.merge_into.filter(|value| !value.trim().is_empty()) {
                let target = sanitize_skill_name(&merge_into);
                if target != cluster.name {
                    cluster.can_generate_skill = false;
                    cluster
                        .source_skills
                        .push(format!("LLM merge target: {}", target));
                }
            }
            if let Some(reasoning) = decision.reasoning.filter(|value| !value.trim().is_empty()) {
                cluster.source_skills = vec![format!("LLM 复核：{}", reasoning)];
            }
        }
    }
    clusters.sort_by_key(|cluster| std::cmp::Reverse(cluster.skill_score));
    Ok(true)
}

async fn multi_agent_review_draft(
    db_path: &Path,
    cluster: &WorkflowCluster,
    draft_body: &str,
) -> Result<String> {
    let context = serde_json::to_string(&json!({
        "name": cluster.name,
        "description": cluster.description,
        "frequency": cluster.frequency,
        "source_agents": cluster.source_agents,
        "estimated_time_saved": cluster.estimated_time_saved,
        "sample_tasks": cluster.sample_tasks,
    }))?;

    // ---- Agent 1: Judge — 审查草稿，指出问题 ----
    let judge_prompt = format!(
        "请审查以下 Skill 草稿。\n\n工作流上下文：\n{}\n\n当前草稿：\n{}\n\n\
         返回格式：只返回一个 JSON 对象，格式为：\n\
         {{\"issues\":[\"问题1\",\"问题2\"],\"suggestions\":[\"建议1\",\"建议2\"],\"score\":0-100,\"verdict\":\"keep|revise|discard\"}}\n\
         \n审查维度：\n\
         1. 名称是否具体、唯一（不能是泛化大类名如 frontend-ui）\n\
         2. 描述是否清晰描述了触发场景和解决的问题\n\
         3. 步骤是否可执行、不空洞（不能写\"根据需求做X\"）\n\
         4. 是否缺少安全注意事项\n\
         5. 是否有虚假工具或危险默认操作\n\
         6. frontmatter 是否完整",
        context, draft_body
    );
    let judge_raw = call_llm(
        db_path,
        "你是严格的 Skill 评审专家。仔细审查草稿中的每个问题，不要遗漏。你的评审将被另一个 Agent 批判检查。",
        &judge_prompt,
    )
    .await?;
    let judge_json = extract_json_object(&judge_raw).unwrap_or_else(|_| judge_raw.clone());

    // ---- Agent 2: Critic — 批判 Judge 的分析 ----
    let critic_prompt = format!(
        "以下是一个 Skill 草稿及其评审结果。请批判这份评审。\n\n\
         草稿：\n{}\n\n\
         评审结果：\n{}\n\n\
         返回格式：只返回一个 JSON 对象，格式为：\n\
         {{\"agreements\":[\"同意的评审点\"],\"disagreements\":[\"不同意的评审点及原因\"],\"blind_spots\":[\"评审遗漏的问题\"],\"over_criticism\":[\"评审过度苛刻的地方\"]}}\n\
         \n注意：\n\
         - 不要因为评审全面就不提反对意见，强制找出至少1个可改进之处\n\
         - blind_spots 是评审没提到但你认为应该指出的问题",
        draft_body, judge_json
    );
    let critic_raw = call_llm(
        db_path,
        "你是严格的评审批判者。你的任务是找出评审中的漏洞、误判和遗漏。即使评审看起来不错，也必须找出可以改进的地方。",
        &critic_prompt,
    )
    .await?;
    let critic_json = extract_json_object(&critic_raw).unwrap_or_else(|_| critic_raw.clone());

    // ---- Agent 3: Fixer — 综合 Judge + Critic，生成最终草稿 ----
    let fixer_prompt = format!(
        "请根据以下信息修复并输出最终版 SKILL.md。\n\n\
         工作流上下文：\n{}\n\n\
         原草稿：\n{}\n\n\
         评审结果（Judge）：\n{}\n\n\
         批审判定（Critic）：\n{}\n\n\
         要求：\n\
         1. 直接输出完整的 SKILL.md（包含 YAML frontmatter），不要任何解释\n\
         2. 综合 Judge 和 Critic 的意见：采纳 Judge 的合理建议，修正 Critic 指出的盲点\n\
         3. 名称必须具体、kebab-case、不能是泛化大类名\n\
         4. 正文必须包含：概述、适用场景、输入信号、执行步骤、验证方式、安全注意事项\n\
         5. 不编造工具或流程，所有内容只能基于给定的工作流上下文",
        context, draft_body, judge_json, critic_json
    );
    let fixer_raw = call_llm(
        db_path,
        "你是资深 AI Agent Skill 修复专家。你的任务是综合评审和批判意见，生成最优质的 SKILL.md。直接输出 Markdown，不要解释。",
        &fixer_prompt,
    )
    .await?;

    let fixed = strip_markdown_fence(&fixer_raw);
    if fixed.trim().len() < 80 {
        return Err(anyhow!(
            "Multi-agent review produced too-short output ({} chars)",
            fixed.trim().len()
        ));
    }
    Ok(fixed)
}

async fn optimize_drafts_with_llm(db_path: &Path, clusters: &[WorkflowCluster]) -> Result<i64> {
    if llm_config(db_path)?.is_none() {
        return Ok(0);
    }

    let mut optimized = 0;
    for cluster in clusters
        .iter()
        .filter(|cluster| cluster.can_generate_skill && cluster.skill_score >= 60)
        .take(5)
    {
        let conn = db::open_conn(db_path)?;
        let id = workflow_service::workflow_id_for_cluster(&conn, cluster)?;
        let draft_body: Option<String> = conn
            .query_row(
                "SELECT draft_body FROM workflow_clusters WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .unwrap_or(None);
        let Some(draft_body) = draft_body.filter(|v| !v.trim().is_empty()) else {
            continue;
        };

        let fixed = match multi_agent_review_draft(db_path, cluster, &draft_body).await {
            Ok(f) => f,
            Err(_) => {
                // Fall back to original draft on multi-agent failure
                draft_body
            }
        };

        conn.execute(
            "UPDATE workflow_clusters
             SET draft_body = ?2, recommendation_source = 'multi-agent-review', confidence = 0.88,
                 reasoning = COALESCE(reasoning, '') || ?3, updated_at = ?4
             WHERE id = ?1",
            params![
                id,
                fixed,
                "\n三 Agent 审查（Judge → Critic → Fixer）已完成 Skill 草稿优化。",
                now_string(),
            ],
        )?;
        optimized += 1;
    }
    Ok(optimized)
}

async fn qa_drafts_with_llm(db_path: &Path, clusters: &mut [WorkflowCluster]) -> Result<i64> {
    if llm_config(db_path)?.is_none() {
        return Ok(0);
    }

    let mut fixed = 0;
    let mut seen_names = std::collections::HashSet::new();
    for cluster in clusters
        .iter_mut()
        .filter(|cluster| cluster.can_generate_skill && cluster.skill_score >= 60)
        .take(8)
    {
        let conn = db::open_conn(db_path)?;
        let id = workflow_service::workflow_id_for_cluster(&conn, cluster)?;
        let draft_body: Option<String> = conn
            .query_row(
                "SELECT draft_body FROM workflow_clusters WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .unwrap_or(None);
        let Some(draft_body) = draft_body.filter(|value| !value.trim().is_empty()) else {
            continue;
        };

        // Truncate draft_body if excessively long (> 8000 chars) to avoid token issues
        let draft_for_prompt = if draft_body.chars().count() > 8000 {
            let truncated: String = draft_body.chars().take(8000).collect();
            format!("{}\n\n[... 草稿过长，已截断 ...]", truncated)
        } else {
            draft_body.clone()
        };

        // Two-part response: JSON metadata then corrected markdown, separated by sentinel
        let content = call_llm(
            db_path,
            "你是一个严格的 Draft QA Agent。你必须在草稿进入审核前检查并修正 Skill 名称、描述和 SKILL.md 正文。",
            &format!(
                "请检查并修正下面的 SKILL.md 草稿。\n\
                 返回格式：先返回一行 JSON 元数据，然后一个 ===DRAFT=== 分隔行，最后是完整的修正后 SKILL.md。\n\
                 JSON 格式：{{\"name\":\"kebab-case-skill-name\",\"description\":\"一句清晰描述\",\"score\":0-100,\"verdict\":\"install|revise|merge|discard\",\"summary\":\"一句中文总结\",\"safety\":[\"...\"],\"performance\":[\"...\"],\"functionality\":[\"...\"],\"writing\":[\"...\"],\"improvements\":[\"...\"]}}\n\
                 \n\
                 注意：draft_body 不要放在 JSON 里，放在 ===DRAFT=== 分隔行之后，直接写 Markdown 即可。\n\
                 \n\
                 修正要求：\n\
                 1. name 必须具体、唯一、kebab-case，不能只是大类名。\n\
                 2. draft_body 必须包含 YAML frontmatter，frontmatter 的 name/description 要和 JSON 字段一致。\n\
                 3. 正文必须包含适用场景、输入信号、执行步骤、验证方式、安全注意事项、不要使用时机。\n\
                 4. 删除虚假工具、危险默认操作、过泛步骤。\n\
                 5. 如果草稿应该合并到已有 Skill，verdict 返回 merge。\n\
                 \n\
                 工作流上下文：{}\n\
                 原草稿：\n{}",
                serde_json::to_string(&json!({
                    "name": cluster.name,
                    "description": cluster.description,
                    "frequency": cluster.frequency,
                    "source_agents": cluster.source_agents,
                    "sample_tasks": cluster.sample_tasks,
                }))?,
                draft_for_prompt
            ),
        )
        .await?;

        // Parse the two-part response: JSON metadata before ===DRAFT===, markdown after
        let (meta_json, fixed_body) = parse_qa_response(&content, &draft_body);
        let parsed: SkillReviewResponse = match extract_json_object(&meta_json) {
            Ok(json_str) => match serde_json::from_str(&json_str) {
                Ok(p) => p,
                Err(e) => {
                    // Fallback: try to parse the entire response as JSON (old format)
                    match extract_json_object(&content)
                        .ok()
                        .and_then(|c| serde_json::from_str::<SkillReviewResponse>(&c).ok())
                    {
                        Some(p) => p,
                        None => {
                            return Err(anyhow!(
                                "Skill Review Agent 返回格式解析失败：{}。原始响应前 300 字符：{}",
                                e,
                                content.chars().take(300).collect::<String>()
                            ));
                        }
                    }
                }
            },
            Err(e) => {
                return Err(anyhow!(
                    "Skill Review Agent 返回 JSON 提取失败：{}。原始响应：{}",
                    e,
                    content.chars().take(300).collect::<String>()
                ));
            }
        };

        let mut fixed_name = parsed
            .name
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| cluster.name.clone());
        fixed_name = sanitize_skill_name(&fixed_name);
        if !seen_names.insert(fixed_name.clone()) {
            fixed_name = format!("{}-{}", fixed_name, id);
            seen_names.insert(fixed_name.clone());
        }
        let fixed_description = parsed
            .description
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| cluster.description.clone());
        let feedback = json!({
            "verdict": parsed.verdict.unwrap_or_else(|| "revise".to_string()),
            "safety": parsed.safety.unwrap_or_default(),
            "performance": parsed.performance.unwrap_or_default(),
            "functionality": parsed.functionality.unwrap_or_default(),
            "writing": parsed.writing.unwrap_or_default(),
            "improvements": parsed.improvements.unwrap_or_default(),
        });

        db::open_conn(db_path)?.execute(
            "UPDATE workflow_clusters
             SET name = ?2, description = ?3, draft_body = ?4,
                 review_score = ?5, review_summary = ?6, review_feedback = ?7,
                 recommendation_source = CASE WHEN recommendation_source = 'llm' THEN recommendation_source ELSE 'qa-agent' END,
                 updated_at = ?8
             WHERE id = ?1",
            params![
                id,
                fixed_name,
                fixed_description,
                strip_markdown_fence(&fixed_body),
                parsed.score.unwrap_or(0).clamp(0, 100),
                parsed.summary.unwrap_or_else(|| "Draft QA Agent 已完成检测并修正草稿。".to_string()),
                serde_json::to_string(&feedback)?,
                now_string(),
            ],
        )?;
        cluster.name = fixed_name;
        cluster.description = fixed_description;
        fixed += 1;
    }
    Ok(fixed)
}

async fn generate_ab_variants(db_path: &Path, clusters: &[WorkflowCluster]) -> Result<i64> {
    if llm_config(db_path)?.is_none() {
        return Ok(0);
    }

    let variant_strategies: &[(&str, &str)] = &[
        (
            "A",
            "精简直白型：用最少的文字描述核心流程，适合有经验的开发者快速查阅。省略冗余解释，只保留关键步骤和注意事项。",
        ),
        (
            "B",
            "详尽指南型：包含完整的背景说明、分步教程、代码示例和故障排查，适合新手或复杂场景。",
        ),
        (
            "C",
            "安全保守型：强调边界条件、错误处理、安全注意事项和不使用时机。在每个步骤后标注潜在风险。",
        ),
    ];

    let mut generated = 0;
    for cluster in clusters
        .iter()
        .filter(|cluster| cluster.can_generate_skill && cluster.skill_score >= 70)
        .take(3)
    {
        let conn = db::open_conn(db_path)?;
        let id = workflow_service::workflow_id_for_cluster(&conn, cluster)?;
        let draft_body: Option<String> = conn
            .query_row(
                "SELECT draft_body FROM workflow_clusters WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .unwrap_or(None);
        let Some(ref draft_body) = draft_body.filter(|v| !v.trim().is_empty()) else {
            continue;
        };

        // Get the current run_id for tracking
        let run_id: Option<i64> = conn
            .query_row(
                "SELECT MAX(run_id) FROM evolution_jobs",
                [],
                |row| row.get(0),
            )
            .ok();

        for &(label, strategy) in variant_strategies {
            let variant_prompt = format!(
                "请为以下 Skill 生成一个变体版本。\n\n\
                 原始草稿：\n{}\n\n\
                 变体策略（{}）：{}\n\n\
                 要求：\n\
                 1. 遵守给定的策略风格\n\
                 2. 保持 Skill 的核心功能不变\n\
                 3. YAML frontmatter 中的 name 后加 -{variant} 后缀\n\
                 4. 直接输出完整的 SKILL.md，不要解释",
                draft_body, label, strategy,
                variant = label.to_lowercase()
            );

            let variant_raw = match call_llm(
                db_path,
                "你是资深 Skill 变体生成专家。根据指定策略生成不同风格的 Skill 版本。直接输出 Markdown。",
                &variant_prompt,
            )
            .await
            {
                Ok(content) => content,
                Err(_) => continue,
            };

            let variant_md = strip_markdown_fence(&variant_raw);
            if variant_md.trim().len() < 80 {
                continue;
            }

            let variant_name = format!("{}-{}", cluster.name, label.to_lowercase());
            let _ = conn.execute(
                "INSERT OR REPLACE INTO skill_variants
                 (skill_name, variant_label, draft_body, status, generation_run_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'active', ?4, ?5, ?5)",
                params![variant_name, label, variant_md, run_id, now_string()],
            );
            generated += 1;
        }
    }
    Ok(generated)
}

fn generate_recommendations(conn: &Connection, step_id: i64, clusters: &[WorkflowCluster]) {
    for cluster in clusters.iter().filter(|c| c.can_generate_skill) {
        let Ok(id) = workflow_service::workflow_id_for_cluster(conn, cluster) else {
            continue;
        };

        // Check for matching community skill (highest stars)
        let community_match: Option<(i64, String, i64)> = conn
            .query_row(
                "SELECT id, repo_full_name, stars FROM community_skills
                 WHERE name LIKE ?1 OR ?1 LIKE '%' || name || '%'
                 ORDER BY stars DESC LIMIT 1",
                params![cluster.name],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok();

        // Check for local skill with same name
        let local_match: Option<i64> = conn
            .query_row(
                "SELECT id FROM skills WHERE name = ?1 LIMIT 1",
                params![cluster.name],
                |row| row.get(0),
            )
            .ok();

        let (rec_type, target_type, target_id, confidence, reason) =
            match (&community_match, local_match) {
                (Some((cid, repo, stars)), Some(_)) if *stars > 500 => (
                    "replace",
                    "community",
                    Some(*cid),
                    0.85,
                    format!(
                        "社区 {} ({} stars) 与本地 Skill 同名，建议替换",
                        repo, stars
                    ),
                ),
                (Some((cid, repo, stars)), _) if *stars > 500 => (
                    "install",
                    "community",
                    Some(*cid),
                    0.80,
                    format!("社区 {} ({} stars) 质量较高，建议安装参考", repo, stars),
                ),
                (Some((cid, repo, stars)), _) if *stars > 100 => (
                    "merge",
                    "community",
                    Some(*cid),
                    0.65,
                    format!("社区 {} ({} stars) 有参考价值，建议合并", repo, stars),
                ),
                (_, Some(local_id)) => (
                    "merge",
                    "local_skill",
                    Some(local_id),
                    0.70,
                    "本地已有同名 Skill，建议合并".to_string(),
                ),
                (Some((cid, repo, _)), _) => (
                    "merge",
                    "community",
                    Some(*cid),
                    0.50,
                    format!("社区 {} 有相似 Skill，建议合并参考", repo),
                ),
                (None, None) => (
                    "keep",
                    "none",
                    None,
                    0.60,
                    "无社区或本地相似项，建议保留本地版本".to_string(),
                ),
            };

        let _ = conn.execute(
            "INSERT INTO workflow_recommendations
             (step_id, workflow_id, recommendation_type, target_type, target_community_id, target_local_skill_id, confidence, reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                step_id,
                id,
                rec_type,
                target_type,
                target_id.filter(|_| target_type == "community"),
                target_id.filter(|_| target_type == "local_skill"),
                confidence,
                reason,
                now_string(),
            ],
        );
    }
}

/// Parse the two-part QA response: JSON metadata then ===DRAFT=== then markdown
fn parse_qa_response(content: &str, fallback_draft: &str) -> (String, String) {
    let cleaned = content.trim();
    if let Some(draft_pos) = cleaned.find("\n===DRAFT===") {
        let meta = cleaned[..draft_pos].trim().to_string();
        let draft = cleaned[draft_pos + "\n===DRAFT===".len()..]
            .trim()
            .to_string();
        let draft = strip_markdown_fence(&draft);
        if draft.trim().len() >= 80 {
            return (meta, draft);
        }
    }
    // Try alternative sentinel: ---DRAFT---
    if let Some(draft_pos) = cleaned.find("\n---DRAFT---") {
        let meta = cleaned[..draft_pos].trim().to_string();
        let draft = cleaned[draft_pos + "\n---DRAFT---".len()..]
            .trim()
            .to_string();
        let draft = strip_markdown_fence(&draft);
        if draft.trim().len() >= 80 {
            return (meta, draft);
        }
    }
    // Try ===DRAFT=== without leading newline
    if let Some(draft_pos) = cleaned.find("===DRAFT===") {
        let meta = cleaned[..draft_pos].trim().to_string();
        let draft = cleaned[draft_pos + "===DRAFT===".len()..]
            .trim()
            .to_string();
        let draft = strip_markdown_fence(&draft);
        if draft.trim().len() >= 80 {
            return (meta, draft);
        }
    }
    // Fallback: return the whole content as meta, keep original draft
    (cleaned.to_string(), fallback_draft.to_string())
}

fn llm_config(db_path: &Path) -> Result<Option<LlmConfig>> {
    let conn = db::open_conn(db_path)?;
    if !crate::get_config_bool(&conn, "llm_enabled", false)? {
        return Ok(None);
    }
    let Some(api_key) = crate::get_config(&conn, "llm_api_key")? else {
        return Ok(None);
    };
    let base_url = crate::get_config(&conn, "llm_base_url")?
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string())
        .trim_end_matches('/')
        .to_string();
    let model = crate::get_config(&conn, "llm_model")?.unwrap_or_else(|| "gpt-5.2".to_string());
    let api_format =
        crate::get_config(&conn, "llm_api_format")?.unwrap_or_else(|| "openai".to_string());
    if api_key.trim().is_empty() || base_url.trim().is_empty() || model.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(LlmConfig {
        base_url,
        api_key,
        model,
        api_format,
    }))
}

async fn call_llm(db_path: &Path, system_prompt: &str, user_prompt: &str) -> Result<String> {
    call_llm_with_retry(db_path, system_prompt, user_prompt, 2).await
}

async fn call_llm_with_retry(
    db_path: &Path,
    system_prompt: &str,
    user_prompt: &str,
    retries: u32,
) -> Result<String> {
    let config = llm_config(db_path)?.ok_or_else(|| anyhow!("大模型未启用或未配置 API Key"))?;
    let is_anthropic = config.api_format == "anthropic";
    let url = if is_anthropic {
        format!("{}/v1/messages", config.base_url)
    } else {
        format!("{}/chat/completions", config.base_url)
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let mut last_error = String::new();
    for attempt in 0..=retries {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(500 * attempt as u64)).await;
        }

        let mut req = client
            .post(&url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json");
        if is_anthropic {
            req = req
                .header("x-api-key", &config.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&json!({
                    "model": config.model,
                    "max_tokens": 4096,
                    "system": system_prompt,
                    "messages": [
                        { "role": "user", "content": user_prompt }
                    ],
                    "temperature": 0.2
                }));
        } else {
            req = req.bearer_auth(&config.api_key).json(&json!({
                "model": config.model,
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": user_prompt }
                ],
                "temperature": 0.2
            }));
        }

        let response = match req.send().await {
            Ok(r) => r,
            Err(err) => {
                last_error = format!("大模型网络连接失败：{}", err);
                continue;
            }
        };

        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        // Read raw bytes to avoid Content-Encoding decoding issues
        let bytes = match response.bytes().await {
            Ok(b) => b,
            Err(err) => {
                last_error = format!(
                    "大模型响应读取失败 (HTTP {}, Content-Type: {})：{}",
                    status.as_u16(),
                    content_type,
                    err
                );
                continue;
            }
        };

        if bytes.is_empty() {
            last_error = format!(
                "大模型 API 返回空响应 (HTTP {}, Content-Type: {})",
                status.as_u16(),
                content_type
            );
            continue;
        }

        let text = String::from_utf8_lossy(&bytes).into_owned();

        if !status.is_success() {
            let preview: String = text.chars().take(500).collect();
            last_error = format!(
                "大模型 API 请求失败：HTTP {} (Content-Type: {}) {}",
                status.as_u16(),
                content_type,
                preview
            );
            continue;
        }

        match serde_json::from_str::<Value>(&text) {
            Ok(value) => {
                let content = if is_anthropic {
                    value
                        .get("content")
                        .and_then(Value::as_array)
                        .and_then(|items| items.first())
                        .and_then(|item| item.get("text"))
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                } else {
                    value
                        .get("choices")
                        .and_then(Value::as_array)
                        .and_then(|choices| choices.first())
                        .and_then(|choice| choice.get("message"))
                        .and_then(|message| message.get("content"))
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                };

                match content {
                    Some(c) if !c.trim().is_empty() => return Ok(c),
                    Some(_) => {
                        last_error = "大模型返回了空的 content 字段".to_string();
                        continue;
                    }
                    None => {
                        let preview: String = text.chars().take(300).collect();
                        last_error =
                            format!("大模型返回格式异常 (HTTP {})：{}", status.as_u16(), preview);
                        continue;
                    }
                }
            }
            Err(err) => {
                let preview: String = text.chars().take(300).collect();
                last_error = format!(
                    "大模型返回 JSON 解析失败 (HTTP {}): {}。原始响应：{}",
                    status.as_u16(),
                    err,
                    preview
                );
                continue;
            }
        }
    }

    Err(anyhow!("{}（已重试 {} 次）", last_error, retries))
}

fn extract_json_object(text: &str) -> Result<String> {
    let stripped = strip_markdown_fence(text);
    let start = stripped
        .find('{')
        .ok_or_else(|| anyhow!("大模型未返回 JSON 对象"))?;
    let end = stripped
        .rfind('}')
        .ok_or_else(|| anyhow!("大模型未返回完整 JSON 对象"))?;
    Ok(stripped[start..=end].to_string())
}

fn sanitize_skill_name(name: &str) -> String {
    let mut value = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else if ch.is_whitespace() || ch == '_' || ch == '/' || ch == ':' {
                '-'
            } else {
                ch
            }
        })
        .collect::<String>();
    while value.contains("--") {
        value = value.replace("--", "-");
    }
    value = value.trim_matches('-').to_string();
    if value.is_empty() {
        "generated-skill".to_string()
    } else {
        value
    }
}

fn strip_markdown_fence(text: &str) -> String {
    let trimmed = text.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }
    let mut lines = trimmed.lines().collect::<Vec<_>>();
    if lines
        .first()
        .is_some_and(|line| line.trim_start().starts_with("```"))
    {
        lines.remove(0);
    }
    if lines
        .last()
        .is_some_and(|line| line.trim_start().starts_with("```"))
    {
        lines.pop();
    }
    lines.join("\n").trim().to_string()
}
