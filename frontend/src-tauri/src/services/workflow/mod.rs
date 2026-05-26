mod clustering;
mod naming;

pub(crate) use clustering::make_cluster_names_unique;

use rusqlite::{params, Connection};
use std::collections::HashMap;

pub(crate) struct WorkflowCluster {
    pub name: String,
    pub description: String,
    pub frequency: i64,
    pub source_agents: Vec<String>,
    pub estimated_time_saved: String,
    pub can_generate_skill: bool,
    pub skill_score: i64,
    pub confidence: f64,
    pub sample_tasks: Vec<String>,
    pub source_skills: Vec<String>,
    pub evolves_skill: Option<String>,
    pub iteration_num: Option<i64>,
}

#[allow(dead_code)]
struct SessionRow {
    project_name: Option<String>,
    agent_source: Option<String>,
    first_prompt: Option<String>,
    compressed_summary: Option<String>,
    entrypoint: Option<String>,
    message_count: i64,
    started_at: Option<String>,
    matched_skill: Option<String>,
}

impl SessionRow {
    fn analysis_text(&self) -> String {
        [
            self.first_prompt.as_deref(),
            self.compressed_summary.as_deref(),
        ]
        .into_iter()
        .flatten()
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
    }

    fn sample_text(&self) -> Option<&str> {
        self.first_prompt
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .or(self.compressed_summary.as_deref())
    }
}

pub(crate) fn cluster_workflows(conn: &Connection) -> Vec<WorkflowCluster> {
    let mut stmt = match conn.prepare(
        "SELECT project_name, agent_source, first_prompt, compressed_summary, entrypoint, message_count, started_at, matched_skill
         FROM sessions
         WHERE (first_prompt IS NOT NULL AND first_prompt != '')
            OR (compressed_summary IS NOT NULL AND compressed_summary != '')
         ORDER BY started_at DESC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };

    let sessions: Vec<SessionRow> = stmt
        .query_map([], |row| {
            Ok(SessionRow {
                project_name: row.get(0)?,
                agent_source: row.get(1)?,
                first_prompt: row.get(2)?,
                compressed_summary: row.get(3)?,
                entrypoint: row.get(4)?,
                message_count: row.get(5)?,
                started_at: row.get(6)?,
                matched_skill: row.get(7)?,
            })
        })
        .map(|rows| rows.filter_map(|row| row.ok()).collect())
        .unwrap_or_default();

    if sessions.len() < 2 {
        return Vec::new();
    }

    let mut clusters: Vec<WorkflowCluster> = Vec::new();

    let (iter_sessions, new_sessions): (Vec<&SessionRow>, Vec<&SessionRow>) =
        sessions.iter().partition(|s| s.matched_skill.is_some());

    // ---- Iteration clusters: group by matched_skill ----
    let mut iter_groups: HashMap<String, Vec<&SessionRow>> = HashMap::new();
    for session in &iter_sessions {
        if let Some(ref skill_name) = session.matched_skill {
            iter_groups
                .entry(skill_name.clone())
                .or_default()
                .push(session);
        }
    }
    for (skill_name, group) in iter_groups {
        if group.len() >= 2 {
            if let Some(cluster) = clustering::build_iteration_cluster(conn, &skill_name, &group) {
                clusters.push(cluster);
            }
        }
    }

    // ---- New clusters: existing logic for unmatched sessions ----
    if new_sessions.len() >= 2 {
        let owned_sessions: Vec<SessionRow> = new_sessions
            .iter()
            .map(|s| SessionRow {
                project_name: s.project_name.clone(),
                agent_source: s.agent_source.clone(),
                first_prompt: s.first_prompt.clone(),
                compressed_summary: s.compressed_summary.clone(),
                entrypoint: s.entrypoint.clone(),
                message_count: s.message_count,
                started_at: s.started_at.clone(),
                matched_skill: None,
            })
            .collect();
        let mut new_clusters = clustering::cluster_by_project_and_similarity(&owned_sessions);
        if new_clusters.is_empty() {
            new_clusters = clustering::cluster_by_keywords(&owned_sessions);
        }
        clusters.extend(new_clusters);
    }

    clustering::merge_duplicate_named_clusters(&mut clusters);
    clustering::mark_existing_skill_duplicates(conn, &mut clusters);
    clustering::make_cluster_names_unique(&mut clusters);
    clusters.sort_by_key(|cluster| std::cmp::Reverse(cluster.skill_score));
    clusters
}

pub(crate) fn save_clusters(
    conn: &Connection,
    clusters: &[WorkflowCluster],
) -> anyhow::Result<i64> {
    conn.execute(
        "DELETE FROM workflow_clusters WHERE recommendation_source != 'manual-existing-skill' AND status != 'manual-draft' AND status != 'installed'",
        [],
    )?;

    let now = crate::utils::time::now_string();
    for cluster in clusters {
        conn.execute(
            "INSERT INTO workflow_clusters
             (name, description, frequency, source_agents, estimated_time_saved,
              can_generate_skill, skill_score, status, sample_tasks, recommendation_source,
              confidence, reasoning, source_skills, evolves_skill, iteration_num, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8, 'local-compressed-workflow',
                     ?9, ?10, ?11, ?12, ?13, ?14, ?14)",
            params![
                cluster.name,
                cluster.description,
                cluster.frequency,
                serde_json::to_string(&cluster.source_agents)?,
                cluster.estimated_time_saved,
                cluster.can_generate_skill as i64,
                cluster.skill_score,
                serde_json::to_string(&cluster.sample_tasks)?,
                cluster.confidence,
                format!(
                    "在 {} 条历史会话中发现相似模式，已使用本地压缩摘要降低上下文成本",
                    cluster.frequency
                ),
                serde_json::to_string(&cluster.source_skills)?,
                cluster.evolves_skill,
                cluster.iteration_num,
                now,
            ],
        )?;
    }
    Ok(clusters.len() as i64)
}

const SKILL_TEMPLATE: &str = r#"---
name: {name}
description: {description}
category: {category}
origin: self-evolving
source_type: generated
---

# {name}

## 概述
自动发现于本地 AI 编程助手历史记录中的重复工作流。

## 触发场景
{trigger_scenarios}

## 使用频率
过去一段时间内出现 {frequency} 次。

## 示例任务
{sample_tasks}

## 适用 Agent
{agent_list}

---

*此 Skill 由 Self Evolving Skills 自动生成，请审核后安装到目标 Agent 的 Skills 目录。*
"#;

pub(crate) fn generate_skill_drafts(conn: &Connection, clusters: &[WorkflowCluster]) -> i64 {
    let mut count = 0;

    for cluster in clusters {
        if !cluster.can_generate_skill || cluster.skill_score < 60 {
            continue;
        }

        let category = crate::utils::categorization::derive_category(&cluster.name);
        let scenarios = cluster
            .sample_tasks
            .iter()
            .enumerate()
            .map(|(index, task)| format!("{}. {}", index + 1, task))
            .collect::<Vec<_>>()
            .join("\n");
        let tasks = cluster
            .sample_tasks
            .iter()
            .map(|task| format!("- {}", crate::utils::text::truncate_chars(task, 150)))
            .collect::<Vec<_>>()
            .join("\n");
        let agent_list = if cluster.source_agents.is_empty() {
            "未检测到".to_string()
        } else {
            cluster.source_agents.join(", ")
        };

        let draft = SKILL_TEMPLATE
            .replace("{name}", &cluster.name)
            .replace("{description}", &cluster.description)
            .replace("{category}", &category)
            .replace("{trigger_scenarios}", &scenarios)
            .replace("{frequency}", &cluster.frequency.to_string())
            .replace("{sample_tasks}", &tasks)
            .replace("{agent_list}", &agent_list);

        let now = crate::utils::time::now_string();
        if let Ok(id) = workflow_id_for_cluster(conn, cluster) {
            let _ = conn.execute(
                "UPDATE workflow_clusters SET draft_body = ?2, can_generate_skill = 1, updated_at = ?3
                 WHERE id = ?1",
                params![id, draft, now],
            );
        }
        count += 1;
    }

    count
}

pub(crate) fn workflow_id_for_cluster(
    conn: &Connection,
    cluster: &WorkflowCluster,
) -> anyhow::Result<i64> {
    let sample_tasks = serde_json::to_string(&cluster.sample_tasks)?;
    conn.query_row(
        "SELECT id FROM workflow_clusters WHERE name = ?1 AND sample_tasks = ?2 LIMIT 1",
        params![cluster.name, sample_tasks],
        |row| row.get(0),
    )
    .map_err(Into::into)
}
