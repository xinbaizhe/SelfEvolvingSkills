use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};

use super::CommunityRecommendation;
use crate::utils::time::now_string;

pub(super) fn save_recommendations(
    conn: &Connection,
    topic: &str,
    items: &[CommunityRecommendation],
) -> Result<()> {
    let now = now_string();
    for item in items {
        conn.execute(
            "INSERT INTO community_skills
             (name, repo_full_name, repo_url, stars, description, skill_md_content, file_url,
              installed, verified, relevance_score, quality_score, weighted_score, license,
              pushed_at, matched_file, readme_excerpt, source, recommendation_reason, topic, fetched_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, 0, 1, ?6, ?7, ?8, ?9, ?10, NULL, ?11, ?12, ?13, ?14, ?15, ?15)
             ON CONFLICT(name, repo_full_name) DO UPDATE SET
                repo_url = excluded.repo_url,
                stars = CASE WHEN excluded.stars > 0 THEN excluded.stars ELSE community_skills.stars END,
                description = excluded.description,
                verified = 1,
                relevance_score = excluded.relevance_score,
                quality_score = excluded.quality_score,
                weighted_score = excluded.weighted_score,
                license = excluded.license,
                pushed_at = excluded.pushed_at,
                matched_file = excluded.matched_file,
                readme_excerpt = excluded.readme_excerpt,
                source = excluded.source,
                recommendation_reason = excluded.recommendation_reason,
                topic = excluded.topic,
                fetched_at = excluded.fetched_at",
            params![
                item.name,
                item.repo_full_name,
                item.repo_url,
                item.stars,
                item.description,
                item.relevance_score,
                item.quality_score,
                item.weighted_score,
                item.license,
                item.pushed_at,
                item.description,
                item.source,
                item.reason,
                topic,
                now,
            ],
        )?;
    }
    prune_community_recommendations(conn, topic, 100)?;
    Ok(())
}

pub(super) fn prune_community_recommendations(
    conn: &Connection,
    topic: &str,
    keep: i64,
) -> Result<()> {
    conn.execute(
        "DELETE FROM community_skills
         WHERE topic = ?1
           AND id NOT IN (
             SELECT id FROM community_skills
             WHERE verified = 1 AND topic = ?1
             ORDER BY
               CASE WHEN source = 'LLM' AND stars <= 0 THEN 0 ELSE 1 END DESC,
               weighted_score DESC, stars DESC, fetched_at DESC, id DESC
             LIMIT ?2
         )",
        params![topic, keep],
    )?;
    Ok(())
}

pub(super) fn extract_json_object(text: &str) -> Result<String> {
    let stripped = strip_markdown_fence(text);
    let start = stripped
        .find('{')
        .ok_or_else(|| anyhow!("LLM did not return a JSON object"))?;
    let end = stripped
        .rfind('}')
        .ok_or_else(|| anyhow!("LLM did not return a complete JSON object"))?;
    Ok(stripped[start..=end].to_string())
}

pub(super) fn strip_markdown_fence(text: &str) -> String {
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

pub(super) fn metadata_relevance_score(
    query: &str,
    name: &str,
    description: &str,
    repo_full_name: &str,
) -> f64 {
    let tokens = tokenize(query);
    if tokens.is_empty() {
        return 0.0;
    }
    let haystack = format!("{name} {description} {repo_full_name}").to_lowercase();
    let matches = tokens
        .iter()
        .filter(|token| haystack.contains(token.as_str()))
        .count() as f64;
    (matches / tokens.len() as f64).clamp(0.0, 1.0)
}

pub(super) fn metadata_quality_score(name: &str, description: &str, repo_full_name: &str) -> f64 {
    let lower = format!("{name} {description} {repo_full_name}").to_lowercase();
    let mut score: f64 = 0.15;
    if lower.contains("skill") {
        score += 0.25;
    }
    if lower.contains("agent")
        || lower.contains("assistant")
        || lower.contains("claude")
        || lower.contains("codex")
    {
        score += 0.25;
    }
    if lower.contains("instruction") || lower.contains("prompt") || lower.contains("workflow") {
        score += 0.2;
    }
    if description.chars().count() >= 40 {
        score += 0.15;
    }
    score.clamp(0.0, 1.0)
}

pub(super) fn github_metadata_reason(
    query: &str,
    name: &str,
    description: &str,
    repo_full_name: &str,
    stars: i64,
) -> String {
    let description = description.trim();
    let project_summary = if description.is_empty() {
        format!(
            "这个仓库名为 {repo_full_name}，GitHub 搜索只返回了名称、star 和仓库路径，没有足够描述。"
        )
    } else {
        format!("这个仓库名为 {repo_full_name}，仓库描述是：{description}")
    };
    let signal = matched_signal(query, name, description, repo_full_name);
    let stars_text = if stars > 0 {
        format!("它有 {stars} 个 star，说明至少有一定社区关注度。")
    } else {
        "当前搜索结果没有可用 star 信号，不能只凭热度判断价值。".to_string()
    };
    let lower = format!("{description} {repo_full_name}").to_lowercase();
    let usage = if lower.contains("agent") {
        "它可能对 Skills 工作台的 Agent 能力扩展、工具选择、工作流拆分有参考价值。"
    } else if lower.contains("prompt") || lower.contains("instruction") {
        "它可能对 Skill 的提示词结构、规则写法和触发说明有参考价值。"
    } else if lower.contains("workflow") {
        "它可能对工作流编排、步骤拆分和质量检查有参考价值。"
    } else {
        "它只通过轻量元数据命中，是否值得采用需要打开 README 后再判断。"
    };

    format!(
        "它是什么：{project_summary}\n\n为什么推荐：它命中了「{query}」中的这些信号：{signal}。{stars_text}\n\n对 Skills 工作台有什么用：{usage}\n\n建议优先看什么：先看 README 的项目定位、examples 或 docs 中是否有可复用的 agent instructions、prompt、workflow、rules；如果 README 只是在罗列链接或没有具体示例，可以直接跳过。"
    )
}

pub(super) fn matched_signal(
    query: &str,
    name: &str,
    description: &str,
    repo_full_name: &str,
) -> String {
    let haystack = format!("{name} {description} {repo_full_name}").to_lowercase();
    let matches = tokenize(query)
        .into_iter()
        .filter(|token| haystack.contains(token))
        .collect::<Vec<_>>();
    if matches.is_empty() {
        "没有明显关键词，仅来自 GitHub 搜索排序".to_string()
    } else {
        matches.join(", ")
    }
}

pub(super) fn weighted_score(
    stars: i64,
    relevance: f64,
    quality: f64,
    license: Option<&str>,
    pushed_at: Option<&str>,
) -> f64 {
    let star_score = ((stars.max(0) as f64 + 1.0).log10() / 5.0).clamp(0.0, 1.0);
    let license_score = license.map(|_| 1.0).unwrap_or(0.0);
    let recency_score = pushed_at.map(recency_score).unwrap_or(0.0);
    let freshness_license_score = recency_score * 0.07 + license_score * 0.03;
    (star_score * 0.40 + relevance * 0.30 + quality * 0.20 + freshness_license_score)
        .clamp(0.0, 1.0)
}

fn recency_score(value: &str) -> f64 {
    let Ok(pushed) = chrono::DateTime::parse_from_rfc3339(value) else {
        return 0.0;
    };
    let age_days = (chrono::Utc::now() - pushed.with_timezone(&chrono::Utc)).num_days();
    if age_days <= 90 {
        1.0
    } else if age_days <= 365 {
        0.75
    } else if age_days <= 1095 {
        0.45
    } else {
        0.15
    }
}

pub(super) fn format_timestamp(value: Option<String>) -> Option<String> {
    let value = value?;
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&value) {
        return Some(dt.format("%Y-%m-%d %H:%M:%S").to_string());
    }
    for fmt in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&value, fmt) {
            return Some(dt.format("%Y-%m-%d %H:%M:%S").to_string());
        }
    }
    Some(value.chars().take(19).collect())
}

pub(super) fn tokenize(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_alphanumeric() && ch != '-' && ch != '_')
        .map(str::trim)
        .filter(|word| word.chars().count() >= 2)
        .map(|word| word.to_lowercase())
        .collect()
}

pub(super) fn slug(value: &str) -> String {
    let slug = value
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    slug.trim_matches('-').to_string()
}

pub(super) fn url_query(value: &str) -> String {
    value.trim().replace(' ', "+")
}
