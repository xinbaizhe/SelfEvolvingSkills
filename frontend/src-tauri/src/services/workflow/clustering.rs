use std::collections::HashMap;

use super::naming;
use super::SessionRow;
use super::WorkflowCluster;

pub(super) fn build_iteration_cluster(
    conn: &rusqlite::Connection,
    skill_name: &str,
    sessions: &[&SessionRow],
) -> Option<WorkflowCluster> {
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

    let confidence = ((frequency as f64 / 15.0).min(0.5) + 0.35).clamp(0.3, 0.90);

    let cluster = WorkflowCluster {
        name: format!("{}-upgrade-v{}", skill_name, next_iter),
        description: format!(
            "Skill「{}」的升级候选（第 {} 轮迭代）—— 基于 {} 条新会话发现的使用模式变化",
            skill_name, next_iter, frequency
        ),
        frequency,
        source_agents: agents,
        estimated_time_saved: "迭代优化".to_string(),
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

pub(super) fn cluster_by_project_and_similarity(sessions: &[SessionRow]) -> Vec<WorkflowCluster> {
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

pub(super) fn cluster_by_keywords(sessions: &[SessionRow]) -> Vec<WorkflowCluster> {
    let mut groups: HashMap<String, Vec<&SessionRow>> = HashMap::new();
    for session in sessions {
        let text = session.analysis_text();
        let key = naming::classify_prompt(&text)
            .or_else(|| naming::extract_keywords(&[text.as_str()]).first().cloned())
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
    let keywords = naming::extract_keywords(&analysis_refs);
    let name = naming::skill_name_from_keywords(label, &keywords, &analysis_refs);
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

    let total_msgs: i64 = rows.iter().map(|r| r.message_count).sum();
    let avg_msg_count = if frequency > 0 {
        total_msgs as f64 / frequency as f64
    } else {
        0.0
    };

    let freq_score = (frequency as f64 * 7.0).min(60.0);
    let complexity_score = (avg_msg_count / 3.0).min(25.0);
    let agent_score = ((unique_agents - 1) as f64 * 5.0).clamp(0.0, 10.0);
    let score = (freq_score + complexity_score + agent_score)
        .clamp(15.0, 95.0)
        .round() as i64;

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

fn compute_time_span_label(rows: &[&SessionRow], total_minutes: f64) -> String {
    let timestamps: Vec<&str> = rows
        .iter()
        .filter_map(|r| r.started_at.as_deref())
        .collect();
    if timestamps.len() < 2 {
        return format!("{:.1}h 总计", total_minutes / 60.0);
    }
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
            let span_weeks = (span_hours / (24.0 * 7.0)).max(0.5);
            format!("{:.1}h/周", total_minutes / 60.0 / span_weeks)
        }
        _ => format!("{:.1}h 总计", total_minutes / 60.0),
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
        let words_i = naming::tokenize(&sessions[i].analysis_text());

        for j in (i + 1)..sessions.len() {
            if used[j] {
                continue;
            }
            let words_j = naming::tokenize(&sessions[j].analysis_text());
            if naming::jaccard_similarity(&words_i, &words_j) >= 0.25 {
                group.push(sessions[j]);
                used[j] = true;
            }
        }
        groups.push(group);
    }

    groups
}

pub(super) fn merge_duplicate_named_clusters(clusters: &mut Vec<WorkflowCluster>) {
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

pub(super) fn mark_existing_skill_duplicates(
    conn: &rusqlite::Connection,
    clusters: &mut [WorkflowCluster],
) {
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
        let cluster_words = naming::tokenize(&format!(
            "{}\n{}\n{}",
            cluster.name,
            cluster.description,
            cluster.sample_tasks.join("\n")
        ));
        let mut best_match: Option<(String, f64)> = None;
        for (name, description, body) in &existing {
            let skill_words = naming::tokenize(&format!("{name}\n{description}\n{body}"));
            let score = naming::jaccard_similarity(&cluster_words, &skill_words);
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

pub(crate) fn make_cluster_names_unique(clusters: &mut [WorkflowCluster]) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    for cluster in clusters {
        let count = seen.entry(cluster.name.clone()).or_insert(0);
        if *count > 0 {
            let suffix = naming::extract_keywords(
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
