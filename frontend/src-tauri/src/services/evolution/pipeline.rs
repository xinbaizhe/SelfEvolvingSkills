use rusqlite::params;
use serde_json::{json, Value};
use std::path::Path;
use tauri::{AppHandle, Emitter};

use crate::db;
use crate::services::community;
use crate::services::scan;
use crate::services::workflow::{self};
use crate::utils::time::now_string;

use super::optimize;
use super::status;
use super::PHASES;

pub(super) fn filter_sources(
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

pub(super) fn phase_update(
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

pub(super) fn emit_only(app: &AppHandle, run_id: i64, phase: &str, progress: i64, message: &str) {
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

pub(crate) fn start_evolution_pipeline(
    conn: &rusqlite::Connection,
    app: &AppHandle,
    db_path: &Path,
    scan_lock: std::sync::Arc<tokio::sync::Mutex<()>>,
    agent_ids: Option<Vec<String>>,
) -> Result<Value, anyhow::Error> {
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

    let _ = conn.execute("DELETE FROM workflow_clusters WHERE status != 'installed' AND status != 'manual-draft' AND recommendation_source != 'manual-existing-skill'", []);
    let _ = conn.execute("DELETE FROM community_skills", []);

    let app_handle = app.clone();
    let db_path_clone = db_path.to_path_buf();
    let scan_lock = scan_lock.clone();

    tokio::spawn(async move {
        let mut llm_error: Option<String> = None;

        // =====================================================================
        // Phase 1: discover (0-20%)
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
            if let Err(e) = scan::sync_source_configs(&conn2) {
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
                scan::enabled_source_paths(&conn2).unwrap_or_else(|e| {
                    log::warn!("Failed to get enabled source paths: {e}");
                    Vec::new()
                }),
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

            let skills = scan::scan_skills(&sources);
            phase_update(
                &app_handle,
                &db_path_clone,
                next_run_id,
                "discover",
                14,
                &format!("发现 {} 个 Skills", skills.len()),
            );
            let _ = scan::upsert_skills(&conn2, &skills);

            let agents = scan::scan_agents(&sources);
            phase_update(
                &app_handle,
                &db_path_clone,
                next_run_id,
                "discover",
                17,
                &format!("发现 {} 个 Agents", agents.len()),
            );
            let _ = scan::upsert_agents(&conn2, &agents);

            let sessions = scan::scan_sessions(&sources);
            phase_update(
                &app_handle,
                &db_path_clone,
                next_run_id,
                "discover",
                20,
                &format!("发现 {} 条历史会话，已生成本地压缩摘要", sessions.len()),
            );
            let _ = scan::upsert_sessions(&conn2, &sessions);
            drop(_guard);
            drop(conn2);
        }

        // =====================================================================
        // Phase 2: reference_retrieval (20-32%)
        // =====================================================================
        phase_update(
            &app_handle,
            &db_path_clone,
            next_run_id,
            "reference_retrieval",
            22,
            "正在检索社区参考 Skills（GitHub / Web / 本地）...",
        );
        let community_error = match community::fetch_top_community_skills(&db_path_clone, &[]).await
        {
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
        // Phase 3: cluster (32-50%)
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
        let mut clusters = workflow::cluster_workflows(&conn3);
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
        match optimize::refine_clusters_with_llm(&db_path_clone, &mut clusters).await {
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
        workflow::make_cluster_names_unique(&mut clusters);

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
        let _ = workflow::save_clusters(&conn4, &clusters);
        phase_update(
            &app_handle,
            &db_path_clone,
            next_run_id,
            "cluster",
            50,
            "聚类结果已保存",
        );

        // =====================================================================
        // Phase 4: draft_generate (50-62%)
        // =====================================================================
        phase_update(
            &app_handle,
            &db_path_clone,
            next_run_id,
            "draft_generate",
            52,
            "正在基于模板生成 Skill 草稿（纯本地）...",
        );
        let drafts = workflow::generate_skill_drafts(&conn4, &clusters);
        for cluster in clusters
            .iter()
            .filter(|c| c.can_generate_skill && c.skill_score >= 60)
        {
            if let Ok(id) = workflow::workflow_id_for_cluster(&conn4, cluster) {
                if let Ok(Some(draft_body)) = conn4.query_row(
                    "SELECT draft_body FROM workflow_clusters WHERE id = ?1",
                    [id],
                    |row| row.get::<_, Option<String>>(0),
                ) {
                    if !draft_body.trim().is_empty() {
                        status::record_skill_iteration(
                            &db_path_clone,
                            &cluster.name,
                            1,
                            None,
                            &draft_body,
                            "初始模板生成",
                            None,
                            None,
                            Some(&format!("本地模板生成，评分 {}", cluster.skill_score)),
                            next_run_id,
                        );
                    }
                }
            }
        }
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
        // Phase 5: optimize (62-78%)
        // =====================================================================
        phase_update(
            &app_handle,
            &db_path_clone,
            next_run_id,
            "optimize",
            64,
            "正在用大模型逐条优化草稿...",
        );
        let optimized = match optimize::optimize_drafts_with_llm(
            &db_path_clone,
            &clusters,
            next_run_id,
        )
        .await
        {
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

        let ab_count = match optimize::generate_ab_variants(&db_path_clone, &clusters).await {
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
        // Phase 6: qa_review (78-90%)
        // =====================================================================
        phase_update(
            &app_handle,
            &db_path_clone,
            next_run_id,
            "qa_review",
            80,
            "Skill Review Agent 正在评审草稿...",
        );
        let reviewed =
            match optimize::qa_drafts_with_llm(&db_path_clone, &mut clusters, next_run_id).await {
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
        // Phase 7: diff_recommend (90-100%)
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
        let comparisons = community::compare_with_community(&conn5, &clusters);
        phase_update(
            &app_handle,
            &db_path_clone,
            next_run_id,
            "diff_recommend",
            96,
            &format!("差异对比完成，{} 个工作流有社区参考", comparisons),
        );

        let diff_step_id = step_ids["diff_recommend"];
        let _ = conn5.execute(
            "UPDATE evolution_steps SET status = 'running' WHERE id = ?1",
            params![diff_step_id],
        );
        optimize::generate_recommendations(&conn5, diff_step_id, &clusters);
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
