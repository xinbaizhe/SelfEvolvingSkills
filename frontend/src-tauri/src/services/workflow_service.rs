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
            .or_else(|| self.compressed_summary.as_deref())
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

    // Split sessions: those matching an existing skill → iteration candidates
    let (iter_sessions, new_sessions): (Vec<&SessionRow>, Vec<&SessionRow>) = sessions
        .iter()
        .partition(|s| s.matched_skill.is_some());

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
            if let Some(cluster) = build_iteration_cluster(conn, &skill_name, &group) {
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
        let mut new_clusters = cluster_by_project_and_similarity(&owned_sessions);
        if new_clusters.is_empty() {
            new_clusters = cluster_by_keywords(&owned_sessions);
        }
        clusters.extend(new_clusters);
    }

    merge_duplicate_named_clusters(&mut clusters);
    mark_existing_skill_duplicates(conn, &mut clusters);
    make_cluster_names_unique(&mut clusters);
    clusters.sort_by_key(|cluster| std::cmp::Reverse(cluster.skill_score));
    clusters
}

fn build_iteration_cluster(
    conn: &Connection,
    skill_name: &str,
    sessions: &[&SessionRow],
) -> Option<WorkflowCluster> {
    // Get current iteration_num for this skill
    let current_iter: i64 = conn
        .query_row(
            "SELECT COALESCE(iteration_num, 1) FROM skills WHERE name = ?1 LIMIT 1",
            [skill_name],
            |row| row.get(0),
        )
        .unwrap_or(1);

    let next_iter = current_iter + 1;
    let frequency = sessions.len() as i64;

    let agents = sessions
        .iter()
        .filter_map(|row| row.agent_source.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let sample_tasks = sessions
        .iter()
        .filter_map(|row| row.first_prompt.as_deref())
        .take(5)
        .map(|prompt| crate::utils::text::truncate_chars(prompt, 200))
        .collect::<Vec<_>>();

    let total_msgs: i64 = sessions.iter().map(|r| r.message_count).sum();
    let avg_msg_count = total_msgs as f64 / frequency as f64;

    let freq_score = (frequency as f64 * 7.0).min(60.0);
    let complexity_score = (avg_msg_count / 3.0).min(25.0);
    let score = (freq_score + complexity_score).clamp(20.0, 90.0).round() as i64;

    // Higher base confidence for iteration since we know the skill is already in use
    let confidence = ((frequency as f64 / 15.0).min(0.5) + 0.35).clamp(0.3, 0.90);

    let cluster = WorkflowCluster {
        name: format!("{}-upgrade-v{}", skill_name, next_iter),
        description: format!(
            "Skill「{}」的升级候选（第 {} 轮迭代）—— 基于 {} 条新会话发现的使用模式变化",
            skill_name, next_iter, frequency
        ),
        frequency,
        source_agents: agents,
        estimated_time_saved: format!("迭代优化"),
        can_generate_skill: true,
        skill_score: score,
        confidence,
        sample_tasks,
        source_skills: vec![skill_name.to_string()],
        evolves_skill: Some(skill_name.to_string()),
        iteration_num: Some(next_iter),
    };
    Some(cluster)
}

fn cluster_by_project_and_similarity(sessions: &[SessionRow]) -> Vec<WorkflowCluster> {
    let mut project_groups: HashMap<String, Vec<&SessionRow>> = HashMap::new();
    for session in sessions {
        let key = session.project_name.as_deref().unwrap_or("unknown");
        project_groups
            .entry(key.to_string())
            .or_default()
            .push(session);
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
    let analysis_texts = rows
        .iter()
        .map(|row| row.analysis_text())
        .collect::<Vec<_>>();
    let analysis_refs = analysis_texts
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let keywords = extract_keywords(&analysis_refs);
    let name = skill_name_from_keywords(label, &keywords, &analysis_refs);
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
    let unique_agents = agents.len() as i64;

    // Average message count per session — proxy for workflow complexity
    let total_msgs: i64 = rows.iter().map(|r| r.message_count).sum();
    let avg_msg_count = if frequency > 0 {
        total_msgs as f64 / frequency as f64
    } else {
        0.0
    };

    // --- skill_score: multi-factor (0-100, clamped 15-95) ---
    // Frequency component: more occurrences → higher base score
    let freq_score = (frequency as f64 * 7.0).min(60.0);
    // Complexity component: longer sessions suggest a more valuable, reusable workflow
    let complexity_score = (avg_msg_count / 3.0).min(25.0);
    // Agent diversity: workflows spanning multiple agents are more reusable
    let agent_score = ((unique_agents - 1) as f64 * 5.0).min(10.0).max(0.0);
    let score = (freq_score + complexity_score + agent_score)
        .clamp(15.0, 95.0)
        .round() as i64;

    // --- confidence: independent of score, measures data reliability (0.0-1.0) ---
    let size_conf = (frequency as f64 / 20.0).min(0.55);
    let complexity_conf = if avg_msg_count > 15.0 {
        0.25
    } else if avg_msg_count > 8.0 {
        0.15
    } else {
        0.05
    };
    let agent_conf = if unique_agents > 2 {
        0.15
    } else if unique_agents > 1 {
        0.08
    } else {
        0.0
    };
    let confidence = (size_conf + complexity_conf + agent_conf).clamp(0.2, 0.95);

    // --- estimated_time_saved: based on actual message counts ---
    // Each message represents ~2.5 minutes of AI-assisted work
    let total_minutes = frequency as f64 * avg_msg_count * 2.5;
    let time_label = compute_time_span_label(rows, total_minutes);

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
        estimated_time_saved: time_label,
        can_generate_skill: true,
        skill_score: score,
        confidence,
        sample_tasks,
        source_skills: vec![],
        evolves_skill: None,
        iteration_num: None,
    }
}

/// Compute a human-readable time-saved label using actual session timestamps.
fn compute_time_span_label(rows: &[&SessionRow], total_minutes: f64) -> String {
    let timestamps: Vec<&str> = rows
        .iter()
        .filter_map(|r| r.started_at.as_deref())
        .collect();
    if timestamps.len() < 2 {
        return format!("{:.1}h 总计", total_minutes / 60.0);
    }
    // Parse the earliest and latest started_at (format: yyyy-MM-dd HH:mm:ss or ISO)
    let parse_ts = |s: &str| {
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
            .ok()
    };
    let earliest = timestamps.iter().filter_map(|t| parse_ts(t)).min();
    let latest = timestamps.iter().filter_map(|t| parse_ts(t)).max();
    match (earliest, latest) {
        (Some(first), Some(last)) if last > first => {
            let span_hours = (last - first).num_hours() as f64;
            let span_weeks = (span_hours / (24.0 * 7.0)).max(0.5); // minimum half-week to avoid inflating
            format!("{:.1}h/周", total_minutes / 60.0 / span_weeks)
        }
        _ => format!("{:.1}h 总计", total_minutes / 60.0),
    }
}

fn skill_name_from_keywords(base_name: &str, keywords: &[String], texts: &[&str]) -> String {
    let haystack = format!("{} {}", base_name, texts.join(" ")).to_lowercase();
    let subject = first_named_part(
        &haystack,
        &[
            ("obsidian", &["obsidian"][..]),
            ("github", &["github", "pull request", "issue", "pr"]),
            ("git", &["git", "commit", "branch", "merge conflict"]),
            ("sqlite", &["sqlite", "database", "db"]),
            ("json", &["json"]),
            ("csv", &["csv"]),
            ("readme", &["readme"]),
            ("docs", &["doc", "docs", "documentation"]),
            ("vue", &["vue"]),
            ("react", &["react"]),
            ("css", &["css", "style", "styles"]),
            ("typescript", &["typescript", "tsc"]),
            ("rust", &["rust", "cargo"]),
            ("api", &["api", "endpoint"]),
        ],
    )
    .or_else(|| useful_name_keyword(keywords))
    .unwrap_or_else(|| "workflow".to_string());

    let action = first_named_part(
        &haystack,
        &[
            ("review", &["review", "bug", "risk"]),
            (
                "test",
                &["test", "check", "vitest", "jest", "cargo check", "tsc"],
            ),
            ("refactor", &["refactor"]),
            ("generate", &["generate", "create", "draft"]),
            ("summarize", &["summary", "summarize", "weekly"]),
            ("migrate", &["migrate", "import", "export"]),
            ("debug", &["debug", "fix", "error"]),
            ("optimize", &["optimize", "performance"]),
            ("configure", &["config", "configure"]),
            (
                "automate",
                &["automation", "batch", "script", "workflow", "pipeline"],
            ),
        ],
    )
    .unwrap_or_else(|| "assist".to_string());

    let artifact = first_named_part(
        &haystack,
        &[
            ("report", &["report", "weekly"]),
            ("notes", &["notes", "note"]),
            ("component", &["component"]),
            ("tests", &["tests"]),
            ("release", &["release"]),
            ("skill", &["skill", "skills"]),
            ("docs", &["readme", "documentation"]),
            ("data", &["data"]),
        ],
    );

    let mut parts = vec![subject, action];
    if let Some(artifact) = artifact {
        if !parts.contains(&artifact) {
            parts.push(artifact);
        }
    }
    if parts.len() < 3 {
        if let Some(extra) = useful_name_keyword(keywords) {
            if !parts.contains(&extra) {
                parts.push(extra);
            }
        }
    }
    sanitize_skill_name(&parts.join("-"))
}

fn first_named_part(haystack: &str, groups: &[(&str, &[&str])]) -> Option<String> {
    groups.iter().find_map(|(label, keys)| {
        keys.iter()
            .any(|key| haystack.contains(&key.to_lowercase()))
            .then(|| (*label).to_string())
    })
}

fn useful_name_keyword(keywords: &[String]) -> Option<String> {
    keywords
        .iter()
        .map(|word| sanitize_skill_name(word))
        .find(|word| {
            word.len() >= 3
                && !matches!(
                    word.as_str(),
                    "frontend"
                        | "backend"
                        | "review"
                        | "test"
                        | "workflow"
                        | "pipeline"
                        | "component"
                        | "style"
                        | "skill"
                        | "skills"
                        | "agent"
                        | "code"
                        | "task"
                )
        })
}

fn sanitize_skill_name(name: &str) -> String {
    let mut output = String::new();
    let mut last_dash = false;
    for ch in name.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch);
            last_dash = false;
        } else if !last_dash {
            output.push('-');
            last_dash = true;
        }
    }
    let cleaned = output.trim_matches('-').to_string();
    if cleaned.is_empty() {
        "general-workflow".to_string()
    } else {
        cleaned
    }
}

fn merge_duplicate_named_clusters(clusters: &mut Vec<WorkflowCluster>) {
    let mut merged: Vec<WorkflowCluster> = Vec::new();
    for cluster in clusters.drain(..) {
        if let Some(existing) = merged.iter_mut().find(|item| item.name == cluster.name) {
            existing.frequency += cluster.frequency;
            existing.skill_score = existing.skill_score.max(cluster.skill_score).min(100);
            existing.confidence = existing.confidence.max(cluster.confidence);
            for agent in cluster.source_agents {
                if !existing.source_agents.contains(&agent) {
                    existing.source_agents.push(agent);
                }
            }
            for task in cluster.sample_tasks {
                if existing.sample_tasks.len() < 8 && !existing.sample_tasks.contains(&task) {
                    existing.sample_tasks.push(task);
                }
            }
            for source_skill in cluster.source_skills {
                if !existing.source_skills.contains(&source_skill) {
                    existing.source_skills.push(source_skill);
                }
            }
            existing.description = format!(
                "{}\n已合并同名候选，累计出现 {} 次。",
                existing.description, existing.frequency
            );
        } else {
            merged.push(cluster);
        }
    }
    *clusters = merged;
}

pub(crate) fn make_cluster_names_unique(clusters: &mut [WorkflowCluster]) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    for cluster in clusters {
        let count = seen.entry(cluster.name.clone()).or_insert(0);
        if *count > 0 {
            let suffix = extract_keywords(
                &cluster
                    .sample_tasks
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            )
            .into_iter()
            .take(2)
            .collect::<Vec<_>>()
            .join("-");
            cluster.name = if suffix.is_empty() {
                format!("{}-{}", cluster.name, *count + 1)
            } else {
                format!("{}-{}", cluster.name, suffix)
            };
        }
        *count += 1;
    }
}

fn mark_existing_skill_duplicates(conn: &Connection, clusters: &mut [WorkflowCluster]) {
    let mut stmt = match conn.prepare("SELECT name, description, body_text FROM skills") {
        Ok(stmt) => stmt,
        Err(_) => return,
    };
    let existing = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            ))
        })
        .map(|rows| rows.filter_map(|row| row.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    if existing.is_empty() {
        return;
    }

    for cluster in clusters {
        let cluster_words = tokenize(&format!(
            "{}\n{}\n{}",
            cluster.name,
            cluster.description,
            cluster.sample_tasks.join("\n")
        ));
        let mut best_match: Option<(String, f64)> = None;
        for (name, description, body) in &existing {
            let skill_words = tokenize(&format!("{name}\n{description}\n{body}"));
            let score = jaccard_similarity(&cluster_words, &skill_words);
            if score >= 0.72
                && best_match
                    .as_ref()
                    .map(|(_, best)| score > *best)
                    .unwrap_or(true)
            {
                best_match = Some((name.clone(), score));
            }
        }

        if let Some((name, score)) = best_match {
            cluster.can_generate_skill = false;
            cluster.skill_score = cluster.skill_score.min(55);
            cluster.source_skills.push(format!(
                "Existing Skill Matcher Agent: 与已有 Skill「{}」高度相似，相似度 {:.0}%，建议进化已有 Skill 而不是新建。",
                name,
                score * 100.0
            ));
        }
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
        (
            "代码审查",
            &["review", "审查", "检查", "bug", "漏洞", "风险"],
        ),
        (
            "测试与验证",
            &[
                "test",
                "测试",
                "验证",
                "vitest",
                "jest",
                "cargo check",
                "tsc",
            ],
        ),
        (
            "前端界面",
            &["ui", "页面", "组件", "样式", "vue", "react", "css"],
        ),
        (
            "数据处理",
            &["data", "json", "csv", "数据库", "sqlite", "导入", "导出"],
        ),
        ("文档写作", &["doc", "readme", "文档", "说明", "方案"]),
        (
            "自动化流程",
            &["自动", "脚本", "workflow", "pipeline", "批量"],
        ),
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
