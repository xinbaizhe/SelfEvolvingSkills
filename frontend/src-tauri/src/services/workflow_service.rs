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
    pub sample_tasks: Vec<String>,
    pub source_skills: Vec<String>,
}

#[allow(dead_code)]
struct SessionRow {
    project_name: Option<String>,
    agent_source: Option<String>,
    first_prompt: Option<String>,
    compressed_summary: Option<String>,
    entrypoint: Option<String>,
}

impl SessionRow {
    fn analysis_text(&self) -> String {
        [self.first_prompt.as_deref(), self.compressed_summary.as_deref()]
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
            .or_else(|| self.compressed_summary.as_deref())
    }
}

pub(crate) fn cluster_workflows(conn: &Connection) -> Vec<WorkflowCluster> {
    let mut stmt = match conn.prepare(
        "SELECT project_name, agent_source, first_prompt, compressed_summary, entrypoint
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
            })
        })
        .map(|rows| rows.filter_map(|row| row.ok()).collect())
        .unwrap_or_default();

    if sessions.len() < 2 {
        return Vec::new();
    }

    let mut clusters = cluster_by_project_and_similarity(&sessions);
    if clusters.is_empty() {
        clusters = cluster_by_keywords(&sessions);
    }

    clusters.sort_by_key(|cluster| std::cmp::Reverse(cluster.skill_score));
    clusters
}

fn cluster_by_project_and_similarity(sessions: &[SessionRow]) -> Vec<WorkflowCluster> {
    let mut project_groups: HashMap<String, Vec<&SessionRow>> = HashMap::new();
    for session in sessions {
        let key = session.project_name.as_deref().unwrap_or("unknown");
        project_groups.entry(key.to_string()).or_default().push(session);
    }

    let mut clusters = Vec::new();
    for (project, group) in project_groups {
        if group.len() < 2 {
            continue;
        }
        for sub in cluster_by_keyword_overlap(&group) {
            if sub.len() >= 2 {
                clusters.push(build_cluster(&project, &sub));
            }
        }
    }
    clusters
}

fn cluster_by_keywords(sessions: &[SessionRow]) -> Vec<WorkflowCluster> {
    let mut groups: HashMap<String, Vec<&SessionRow>> = HashMap::new();
    for session in sessions {
        let text = session.analysis_text();
        let key = classify_prompt(&text)
            .or_else(|| extract_keywords(&[text.as_str()]).first().cloned())
            .unwrap_or_else(|| "通用工作流".to_string());
        groups.entry(key).or_default().push(session);
    }

    groups
        .into_iter()
        .filter(|(_, group)| group.len() >= 2)
        .map(|(name, group)| build_cluster(&name, &group))
        .collect()
}

fn build_cluster(label: &str, rows: &[&SessionRow]) -> WorkflowCluster {
    let analysis_texts = rows.iter().map(|row| row.analysis_text()).collect::<Vec<_>>();
    let analysis_refs = analysis_texts.iter().map(String::as_str).collect::<Vec<_>>();
    let keywords = extract_keywords(&analysis_refs);
    let name = classify_prompt(&analysis_refs.join("\n"))
        .or_else(|| keywords.first().cloned())
        .unwrap_or_else(|| label.to_string());
    let agents = rows
        .iter()
        .filter_map(|row| row.agent_source.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let sample_tasks = rows
        .iter()
        .filter_map(|row| row.sample_text())
        .take(5)
        .map(|prompt| crate::utils::text::truncate_chars(prompt, 200))
        .collect::<Vec<_>>();
    let frequency = rows.len() as i64;
    let score = ((frequency as f64 / 5.0) * 100.0).round() as i64;
    let score = score.clamp(60, 95);
    let keyword_text = if keywords.is_empty() {
        name.clone()
    } else {
        keywords.join(", ")
    };

    WorkflowCluster {
        name: name.clone(),
        description: format!(
            "在历史会话中发现 {} 次相似操作。关键词：{}",
            frequency, keyword_text
        ),
        frequency,
        source_agents: agents,
        estimated_time_saved: format!("{:.1}h/周", frequency as f64 * 0.3),
        can_generate_skill: true,
        skill_score: score,
        sample_tasks,
        source_skills: vec![],
    }
}

fn cluster_by_keyword_overlap<'a>(sessions: &[&'a SessionRow]) -> Vec<Vec<&'a SessionRow>> {
    let mut groups: Vec<Vec<&SessionRow>> = Vec::new();
    let mut used = vec![false; sessions.len()];

    for i in 0..sessions.len() {
        if used[i] {
            continue;
        }
        let mut group = vec![sessions[i]];
        used[i] = true;
        let words_i = tokenize(&sessions[i].analysis_text());

        for j in (i + 1)..sessions.len() {
            if used[j] {
                continue;
            }
            let words_j = tokenize(&sessions[j].analysis_text());
            if jaccard_similarity(&words_i, &words_j) >= 0.25 {
                group.push(sessions[j]);
                used[j] = true;
            }
        }
        groups.push(group);
    }

    groups
}

fn tokenize(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_alphanumeric() && ch != '-' && ch != '_')
        .filter(|word| word.len() >= 2)
        .map(|word| word.to_lowercase())
        .collect()
}

fn jaccard_similarity(a: &[String], b: &[String]) -> f64 {
    let set_a = a.iter().collect::<std::collections::HashSet<_>>();
    let set_b = b.iter().collect::<std::collections::HashSet<_>>();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}

fn classify_prompt(prompt: &str) -> Option<String> {
    let lower = prompt.to_lowercase();
    let groups: &[(&str, &[&str])] = &[
        ("代码审查", &["review", "审查", "检查", "bug", "漏洞", "风险"]),
        ("测试与验证", &["test", "测试", "验证", "vitest", "jest", "cargo check", "tsc"]),
        ("前端界面", &["ui", "页面", "组件", "样式", "vue", "react", "css"]),
        ("数据处理", &["data", "json", "csv", "数据库", "sqlite", "导入", "导出"]),
        ("文档写作", &["doc", "readme", "文档", "说明", "方案"]),
        ("自动化流程", &["自动", "脚本", "workflow", "pipeline", "批量"]),
    ];
    groups.iter().find_map(|(label, keys)| {
        if keys.iter().any(|key| lower.contains(key)) {
            Some((*label).to_string())
        } else {
            None
        }
    })
}

fn extract_keywords(prompts: &[&str]) -> Vec<String> {
    let mut freq: HashMap<String, usize> = HashMap::new();
    for prompt in prompts {
        for word in tokenize(prompt) {
            if word.len() >= 3 && !is_stop_word(&word) {
                *freq.entry(word).or_default() += 1;
            }
        }
    }
    let mut words = freq.into_iter().collect::<Vec<_>>();
    words.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    words.into_iter().take(6).map(|(word, _)| word).collect()
}

fn is_stop_word(word: &str) -> bool {
    matches!(
        word,
        "the" | "and" | "for" | "with" | "this" | "that" | "请" | "帮我" | "一个" | "这个"
    )
}

pub(crate) fn save_clusters(conn: &Connection, clusters: &[WorkflowCluster]) -> anyhow::Result<i64> {
    conn.execute("DELETE FROM workflow_clusters", [])?;

    let now = crate::utils::time::now_string();
    for cluster in clusters {
        conn.execute(
            "INSERT INTO workflow_clusters
             (name, description, frequency, source_agents, estimated_time_saved,
              can_generate_skill, skill_score, status, sample_tasks, recommendation_source,
              confidence, reasoning, source_skills, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8, 'local-compressed-workflow',
                     ?9, ?10, ?11, ?12, ?12)",
            params![
                cluster.name,
                cluster.description,
                cluster.frequency,
                serde_json::to_string(&cluster.source_agents)?,
                cluster.estimated_time_saved,
                cluster.can_generate_skill as i64,
                cluster.skill_score,
                serde_json::to_string(&cluster.sample_tasks)?,
                cluster.skill_score as f64 / 100.0,
                format!("在 {} 条历史会话中发现相似模式，已使用本地压缩摘要降低上下文成本", cluster.frequency),
                serde_json::to_string(&cluster.source_skills)?,
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
        let _ = conn.execute(
            "UPDATE workflow_clusters SET draft_body = ?2, can_generate_skill = 1, updated_at = ?3
             WHERE name = ?1 AND can_generate_skill = 1",
            params![cluster.name, draft, now],
        );
        count += 1;
    }

    count
}
