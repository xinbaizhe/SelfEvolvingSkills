use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

use crate::db;
use crate::utils::time::now_string;

#[derive(Debug, Clone)]
struct CommunityRecommendation {
    name: String,
    repo_full_name: String,
    repo_url: String,
    stars: i64,
    description: String,
    reason: String,
    source: String,
    license: Option<String>,
    pushed_at: Option<String>,
    relevance_score: f64,
    quality_score: f64,
    weighted_score: f64,
}

#[derive(Debug, Clone)]
struct LlmConfig {
    base_url: String,
    api_key: String,
    model: String,
    api_format: String,
}

#[derive(Debug, Deserialize)]
struct LlmRecommendationResponse {
    skills: Vec<LlmRecommendationItem>,
}

#[derive(Debug, Deserialize)]
struct LlmRecommendationItem {
    name: String,
    repo_full_name: Option<String>,
    repo_url: Option<String>,
    description: Option<String>,
    reason: Option<String>,
    action: Option<String>,
    score: Option<f64>,
}

async fn fetch_github_skill_candidates(
    query: &str,
    page: i64,
    per_page: i64,
) -> Result<Vec<Value>> {
    let search_query = format!(
        "{} (awesome OR popular OR \"best practices\" OR framework OR toolkit OR agents OR prompts OR instructions) (skill OR SKILL.md OR agent instructions OR prompt engineering) stars:>1000",
        query
    );
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Self-Evolving-Skills"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(12))
        .build()?;

    let response = client
        .get("https://api.github.com/search/repositories")
        .query(&[
            ("q", search_query.as_str()),
            ("sort", "stars"),
            ("order", "desc"),
            ("page", &page.to_string()),
            ("per_page", &per_page.clamp(1, 10).to_string()),
        ])
        .send()
        .await
        .map_err(|err| anyhow!("GitHub network request failed: {}", err))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        let message = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|value| {
                value
                    .get("message")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .unwrap_or_else(|| text.chars().take(300).collect());
        return Err(anyhow!(
            "GitHub API request failed: HTTP {} {}",
            status.as_u16(),
            message
        ));
    }

    let payload: Value = serde_json::from_str(&text)?;
    payload
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| anyhow!("GitHub API response is missing items"))
}

fn github_candidates_to_recommendations(
    query: &str,
    items: &[Value],
) -> Vec<CommunityRecommendation> {
    let mut recommendations = items
        .iter()
        .filter_map(|item| {
            let repo_full_name = item.get("full_name").and_then(Value::as_str)?;
            let repo_url = item.get("html_url").and_then(Value::as_str)?;
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(repo_full_name);
            let description = item
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let stars = item
                .get("stargazers_count")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            let license = item
                .get("license")
                .and_then(|value| value.get("spdx_id"))
                .and_then(Value::as_str)
                .filter(|value| *value != "NOASSERTION")
                .map(ToString::to_string);
            let pushed_at = item
                .get("pushed_at")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            let relevance = metadata_relevance_score(query, name, &description, repo_full_name);
            if relevance < 0.08 {
                return None;
            }
            let quality = metadata_quality_score(name, &description, repo_full_name);
            let weighted = weighted_score(
                stars,
                relevance,
                quality,
                license.as_deref(),
                pushed_at.as_deref(),
            );
            let reason = github_metadata_reason(query, name, &description, repo_full_name, stars);
            Some(CommunityRecommendation {
                name: name.to_string(),
                repo_full_name: repo_full_name.to_string(),
                repo_url: repo_url.to_string(),
                stars,
                description,
                reason,
                source: "GitHub".to_string(),
                license,
                pushed_at,
                relevance_score: relevance,
                quality_score: quality,
                weighted_score: weighted,
            })
        })
        .collect::<Vec<_>>();
    recommendations.sort_by(|a, b| {
        b.weighted_score
            .partial_cmp(&a.weighted_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    recommendations
}

async fn enrich_recommendations_with_github_metadata(items: &mut [CommunityRecommendation]) {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("Self-Evolving-Skills")
        .build()
    {
        Ok(client) => client,
        Err(_) => return,
    };

    for item in items {
        if item.repo_full_name.starts_with("llm-recommendation/") {
            continue;
        }
        let url = format!("https://api.github.com/repos/{}", item.repo_full_name);
        let Ok(response) = client
            .get(url)
            .header(ACCEPT, "application/vnd.github+json")
            .send()
            .await
        else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        let Ok(repo) = response.json::<Value>().await else {
            continue;
        };
        item.stars = repo
            .get("stargazers_count")
            .and_then(Value::as_i64)
            .unwrap_or(item.stars);
        item.license = repo
            .get("license")
            .and_then(|value| value.get("spdx_id"))
            .and_then(Value::as_str)
            .filter(|value| *value != "NOASSERTION")
            .map(ToString::to_string)
            .or_else(|| item.license.clone());
        item.pushed_at = repo
            .get("pushed_at")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or_else(|| item.pushed_at.clone());
        item.repo_url = repo
            .get("html_url")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| item.repo_url.clone());
        item.weighted_score = weighted_score(
            item.stars,
            item.relevance_score,
            item.quality_score,
            item.license.as_deref(),
            item.pushed_at.as_deref(),
        )
        .max(item.weighted_score);
        if item.source == "LLM" && item.stars <= 0 {
            item.weighted_score = item.weighted_score.min(0.19);
        }
    }
}

async fn llm_recommend_community_skills(
    db_path: &Path,
    query: &str,
) -> Result<Vec<CommunityRecommendation>> {
    let Some(config) = llm_config(db_path)? else {
        return Ok(Vec::new());
    };

    let user_prompt = format!(
        r#"Recommend the best 5 community Skills or agent-instruction GitHub repositories for this need: {query}

Popularity constraints:
- Prefer very popular, widely used, high-star, actively maintained GitHub projects.
- Favor projects with signals like awesome, popular, best practices, framework, toolkit, agents, prompts, instructions, prompt engineering.
- Do not recommend tiny personal experiments, low-star repos, or repos without clear documentation.
- Prefer real GitHub repositories. If uncertain, leave repo_full_name and repo_url empty.
- Decide whether each item is add or replace.

The reason field must be Chinese multi-paragraph text and must explain:
1. What it is.
2. How it helps the Skills workbench.
3. When the user should open it.
4. What to inspect first in the repo.

Return JSON only:
{{"skills":[{{"name":"...","repo_full_name":"owner/repo or empty","repo_url":"https://github.com/... or empty","description":"...","reason":"中文推荐理由","action":"add|replace","score":0.0}}]}}"#
    );

    let content = call_llm(
        &config,
        "You are a community AI Agent Skill recommendation expert. Return JSON only. Reasons must be in Chinese and specific enough for a user to decide whether to open the source.",
        &user_prompt,
    )
    .await?;

    let parsed: LlmRecommendationResponse = serde_json::from_str(&extract_json_object(&content)?)?;
    let mut recommendations = parsed
        .skills
        .into_iter()
        .take(5)
        .enumerate()
        .map(|(index, item)| {
            let safe_name = item.name.trim().chars().take(120).collect::<String>();
            let fallback_repo = format!("llm-recommendation/{}", slug(&safe_name));
            let repo_full_name = item
                .repo_full_name
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(fallback_repo);
            let repo_url = item
                .repo_url
                .filter(|value| value.starts_with("https://github.com/"))
                .unwrap_or_else(|| format!("https://github.com/search?q={}", url_query(&safe_name)));
            let description = item
                .description
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "LLM recommended community Skill reference.".to_string());
            let action = item.action.unwrap_or_else(|| "add".to_string());
            let reason = item.reason.filter(|value| !value.trim().is_empty()).unwrap_or_else(|| {
                format!(
                    "它是什么：这是大模型判断适合当前主题的社区 Skill 或 Agent 指令参考。\n\n对 Skills 工作台有什么用：可用于补充社区参考来源，帮助对比本地生成草稿和外部成熟实践。\n\n适合什么时候看：当你想判断当前本地 Skill 是否应该合并、替换、重写或增加步骤时，可以打开来源核对结构。\n\n建议优先看什么：README、示例 prompt、agent instructions、workflow 或 rules 文件。"
                )
            });
            let description = format!(
                "{} 推荐动作：{}。",
                description,
                if action == "replace" { "替换" } else { "新增" }
            );
            let score = item.score.unwrap_or(0.92 - index as f64 * 0.04).clamp(0.0, 1.0);
            CommunityRecommendation {
                name: safe_name,
                repo_full_name,
                repo_url,
                stars: 0,
                description,
                reason,
                source: "LLM".to_string(),
                license: None,
                pushed_at: None,
                relevance_score: score,
                quality_score: score,
                weighted_score: score,
            }
        })
        .collect::<Vec<_>>();
    enrich_recommendations_with_github_metadata(&mut recommendations).await;
    Ok(recommendations)
}
fn save_recommendations(
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

fn prune_community_recommendations(conn: &Connection, topic: &str, keep: i64) -> Result<()> {
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

fn ensure_seed_recommendations(conn: &Connection, query: &str) -> Result<()> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(id) FROM community_skills WHERE verified = 1 AND topic = ?1",
        [query],
        |row| row.get(0),
    )?;
    let missing_reason: i64 = conn.query_row(
        "SELECT COUNT(id) FROM community_skills
         WHERE verified = 1
           AND topic = ?1
           AND (recommendation_reason IS NULL
                OR recommendation_reason = ''
                OR recommendation_reason LIKE '璇ヤ粨搴撲笌%')",
        [query],
        |row| row.get(0),
    )?;
    if count < 5 || missing_reason > 0 {
        save_recommendations(conn, query, &seed_recommendations(query))?;
    }
    Ok(())
}

async fn refresh_seed_recommendations_with_metadata(db_path: &Path, query: &str) {
    let mut items = seed_recommendations(query);
    enrich_recommendations_with_github_metadata(&mut items).await;
    if let Ok(conn) = db::open_conn(db_path) {
        let _ = save_recommendations(&conn, query, &items);
    }
}

fn seed_recommendations(query: &str) -> Vec<CommunityRecommendation> {
    let items = [
        (
            "Claude Code Skills",
            "anthropics/claude-code",
            "https://github.com/anthropics/claude-code",
            "Claude Code Skills pattern: focused SKILL.md instructions, progressive disclosure, and tool-aware workflows for reusable agent skills.",
        ),
        (
            "OpenAI Agents SDK Examples",
            "openai/openai-agents-python",
            "https://github.com/openai/openai-agents-python",
            "Reference implementation for agent workflows, handoffs, guardrails, tracing, and structured agent behavior.",
        ),
        (
            "Awesome Copilot Instructions",
            "github/awesome-copilot",
            "https://github.com/github/awesome-copilot",
            "Community collection of reusable Copilot instructions, prompts, and agent customization examples.",
        ),
        (
            "Cursor Rules Templates",
            "PatrickJS/awesome-cursorrules",
            "https://github.com/PatrickJS/awesome-cursorrules",
            "Reusable Cursor rule examples for framework-specific AI coding workflows and project conventions.",
        ),
        (
            "Aider Conventions",
            "paul-gauthier/aider",
            "https://github.com/paul-gauthier/aider",
            "AI coding assistant repository with practical conventions around repo-aware coding, editing loops, and automation workflows.",
        ),
    ];

    items
        .into_iter()
        .enumerate()
        .map(|(index, (name, repo, url, description))| {
            let relevance = metadata_relevance_score(query, name, description, repo).max(0.72 - index as f64 * 0.04);
            let quality = (0.9 - index as f64 * 0.03).max(0.72);
            CommunityRecommendation {
                name: name.to_string(),
                repo_full_name: repo.to_string(),
                repo_url: url.to_string(),
                stars: 0,
                description: description.to_string(),
                reason: format!(
                    "它是什么：{}\n\n对 Skills 工作台有什么用：它可以提供成熟的社区样例，用来改进推荐、草稿结构、质量评审或差异对比，而不是只依赖本地历史。\n\n适合什么时候看：当你想知道同类 Agent/Skill 项目如何组织指令、工具、工作流和验证标准时值得打开。\n\n建议优先看什么：README、examples、docs、prompt/instructions/rules/workflow 相关目录。",
                    description
                ),
                source: "Seed".to_string(),
                license: None,
                pushed_at: None,
                relevance_score: relevance,
                quality_score: quality,
                weighted_score: (0.86 - index as f64 * 0.04).max(0.68),
            }
        })
        .collect()
}
fn spawn_background_refresh(db_path: std::path::PathBuf, query: String, page: i64, per_page: i64) {
    tokio::spawn(async move {
        refresh_seed_recommendations_with_metadata(&db_path, &query).await;

        let llm_items = llm_recommend_community_skills(&db_path, &query)
            .await
            .unwrap_or_default();
        if !llm_items.is_empty() {
            if let Ok(conn) = db::open_conn(&db_path) {
                let _ = save_recommendations(&conn, &query, &llm_items);
            }
        }

        let github_items = match fetch_github_skill_candidates(&query, page, per_page).await {
            Ok(candidates) => github_candidates_to_recommendations(&query, &candidates),
            Err(_) => Vec::new(),
        };
        if !github_items.is_empty() {
            if let Ok(conn) = db::open_conn(&db_path) {
                let _ = save_recommendations(&conn, &query, &github_items);
            }
        }
    });
}

fn search_cached_community_skills(
    conn: &Connection,
    query_str: &str,
    page: i64,
    per_page: i64,
) -> Result<Value> {
    let total: i64 = conn.query_row(
        "SELECT COUNT(id) FROM community_skills WHERE verified = 1 AND topic = ?1",
        [query_str],
        |row| row.get(0),
    )?;

    let mut stmt = conn.prepare(
        "SELECT id, name, repo_full_name, repo_url, stars, description, installed, fetched_at,
                relevance_score, quality_score, weighted_score, license, pushed_at, matched_file,
                source, recommendation_reason
         FROM community_skills
         WHERE verified = 1 AND topic = ?1
         ORDER BY
           CASE WHEN source = 'LLM' AND stars <= 0 THEN 0 ELSE 1 END DESC,
           weighted_score DESC, stars DESC
         LIMIT ?2 OFFSET ?3",
    )?;

    let items = stmt
        .query_map(params![query_str, per_page, (page - 1).max(0) * per_page], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "repo": row.get::<_, String>(2)?,
                "repo_url": row.get::<_, String>(3)?,
                "stars": row.get::<_, i64>(4)?,
                "description": row.get::<_, Option<String>>(5)?,
                "installed": false,
                "source": row.get::<_, Option<String>>(14)?.unwrap_or_else(|| "GitHub".to_string()),
                "status": "recommended",
                "fetched_at": format_timestamp(row.get::<_, Option<String>>(7)?),
                "relevance_score": row.get::<_, Option<f64>>(8)?.unwrap_or(0.0),
                "quality_score": row.get::<_, Option<f64>>(9)?.unwrap_or(0.0),
                "weighted_score": row.get::<_, Option<f64>>(10)?.unwrap_or(0.0),
                "license": row.get::<_, Option<String>>(11)?,
                "pushed_at": format_timestamp(row.get::<_, Option<String>>(12)?),
                "matched_file": row.get::<_, Option<String>>(13)?,
                "recommendation_reason": row.get::<_, Option<String>>(15)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(json!({ "items": items, "total": total, "page": page, "per_page": per_page }))
}

pub(crate) async fn search_community_skills(
    db_path: &Path,
    query_str: &str,
    page: i64,
    per_page: i64,
) -> Result<Value> {
    let query = if query_str.trim().is_empty() {
        "AI coding agent skills"
    } else {
        query_str.trim()
    }
    .to_string();

    {
        let conn = db::open_conn(db_path)?;
        ensure_seed_recommendations(&conn, &query)?;
    }

    spawn_background_refresh(db_path.to_path_buf(), query.clone(), page, per_page.min(10));

    let conn = db::open_conn(db_path)?;
    search_cached_community_skills(&conn, &query, page, per_page.min(10))
}

pub(crate) fn get_installed_skills(conn: &Connection) -> Result<Value> {
    search_cached_community_skills(conn, "", 1, 10)
}

pub(crate) async fn fetch_top_community_skills(
    db_path: &Path,
    clusters: &[crate::services::workflow_service::WorkflowCluster],
) -> Result<i64> {
    let conn = db::open_conn(db_path)?;
    let before = crate::db::count_table(&conn, "community_skills").unwrap_or(0);
    drop(conn);

    let query = clusters
        .iter()
        .filter(|cluster| cluster.can_generate_skill)
        .map(|cluster| cluster.name.as_str())
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    let query = if query.trim().is_empty() {
        "AI coding agent skills"
    } else {
        query.as_str()
    };

    let llm_items = llm_recommend_community_skills(db_path, query)
        .await
        .unwrap_or_default();
    if !llm_items.is_empty() {
        let conn = db::open_conn(db_path)?;
        save_recommendations(&conn, query, &llm_items)?;
    }

    let conn = db::open_conn(db_path)?;
    let after = crate::db::count_table(&conn, "community_skills").unwrap_or(before);
    Ok((after - before).max(0))
}

pub(crate) fn compare_with_community(
    conn: &Connection,
    clusters: &[crate::services::workflow_service::WorkflowCluster],
) -> i64 {
    let mut comparisons = 0;

    for cluster in clusters.iter().filter(|cluster| cluster.can_generate_skill) {
        let Ok(workflow_id) =
            crate::services::workflow_service::workflow_id_for_cluster(conn, cluster)
        else {
            continue;
        };
        let lower_name = cluster.name.to_lowercase();
        let mut stmt = match conn.prepare(
            "SELECT name, repo_url, stars, description, weighted_score, matched_file FROM community_skills
             WHERE verified = 1 AND (lower(name) LIKE ?1 OR lower(description) LIKE ?1 OR lower(repo_full_name) LIKE ?1 OR lower(readme_excerpt) LIKE ?1)
             ORDER BY weighted_score DESC, stars DESC LIMIT 5",
        ) {
            Ok(stmt) => stmt,
            Err(_) => continue,
        };

        let similar = stmt
            .query_map(params![format!("%{}%", lower_name)], |row| {
                Ok(json!({
                    "name": row.get::<_, String>(0)?,
                    "url": row.get::<_, String>(1)?,
                    "stars": row.get::<_, i64>(2)?,
                    "description": row.get::<_, Option<String>>(3)?,
                    "weighted_score": row.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
                    "source": row.get::<_, Option<String>>(5)?,
                }))
            })
            .map(|rows| rows.filter_map(|row| row.ok()).collect::<Vec<Value>>())
            .unwrap_or_default();

        if !similar.is_empty() {
            let now = now_string();
            let _ = conn.execute(
                "UPDATE workflow_clusters SET similar_skills = ?2, updated_at = ?3 WHERE id = ?1",
                params![
                    workflow_id,
                    serde_json::to_string(&similar).unwrap_or_default(),
                    now,
                ],
            );
            comparisons += 1;
        }
    }

    comparisons
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

async fn call_llm(config: &LlmConfig, system_prompt: &str, user_prompt: &str) -> Result<String> {
    call_llm_with_retry(config, system_prompt, user_prompt, 2).await
}

async fn call_llm_with_retry(
    config: &LlmConfig,
    system_prompt: &str,
    user_prompt: &str,
    retries: u32,
) -> Result<String> {
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
                last_error = format!("澶фā鍨嬬綉缁滆繛鎺ュけ璐ワ細{}", err);
                continue;
            }
        };

        let status = response.status();
        let bytes = match response.bytes().await {
            Ok(b) => b,
            Err(err) => {
                last_error = format!(
                    "LLM response read failed (HTTP {}): {}",
                    status.as_u16(),
                    err
                );
                continue;
            }
        };

        if bytes.is_empty() {
            last_error = format!("澶фā鍨?API 杩斿洖绌哄搷搴?(HTTP {})", status.as_u16());
            continue;
        }

        let text = String::from_utf8_lossy(&bytes).into_owned();

        if !status.is_success() {
            let preview: String = text.chars().take(500).collect();
            last_error = format!(
                "澶фā鍨?API 璇锋眰澶辫触锛欻TTP {} {}",
                status.as_u16(),
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
                        last_error = "澶фā鍨嬭繑鍥炰簡绌虹殑 content 瀛楁".to_string();
                        continue;
                    }
                    None => {
                        let preview: String = text.chars().take(300).collect();
                        last_error = format!(
                            "LLM response missing content (HTTP {}): {}",
                            status.as_u16(),
                            preview
                        );
                        continue;
                    }
                }
            }
            Err(err) => {
                let preview: String = text.chars().take(300).collect();
                last_error = format!(
                    "澶фā鍨嬭繑鍥?JSON 瑙ｆ瀽澶辫触 (HTTP {}): {}銆傚師濮嬪搷搴旓細{}",
                    status.as_u16(),
                    err,
                    preview
                );
                continue;
            }
        }
    }

    Err(anyhow!("{}锛堝凡閲嶈瘯 {} 娆★級", last_error, retries))
}

fn extract_json_object(text: &str) -> Result<String> {
    let stripped = strip_markdown_fence(text);
    let start = stripped
        .find('{')
        .ok_or_else(|| anyhow!("LLM did not return a JSON object"))?;
    let end = stripped
        .rfind('}')
        .ok_or_else(|| anyhow!("LLM did not return a complete JSON object"))?;
    Ok(stripped[start..=end].to_string())
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

fn metadata_relevance_score(
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

fn metadata_quality_score(name: &str, description: &str, repo_full_name: &str) -> f64 {
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

fn github_metadata_reason(
    query: &str,
    name: &str,
    description: &str,
    repo_full_name: &str,
    stars: i64,
) -> String {
    let description = description.trim();
    let project_summary = if description.is_empty() {
        format!("这个仓库名为 {repo_full_name}，GitHub 搜索只返回了名称、star 和仓库路径，没有足够描述。")
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
fn matched_signal(query: &str, name: &str, description: &str, repo_full_name: &str) -> String {
    let haystack = format!("{name} {description} {repo_full_name}").to_lowercase();
    let matches = tokenize(query)
        .into_iter()
        .filter(|token| haystack.contains(token))
        .collect::<Vec<_>>();
    if matches.is_empty() {
        "娌℃湁鏄庢樉鍏抽敭璇嶏紝浠呮潵鑷?GitHub 鎼滅储鎺掑簭".to_string()
    } else {
        matches.join(", ")
    }
}

fn weighted_score(
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

fn format_timestamp(value: Option<String>) -> Option<String> {
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

fn tokenize(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_alphanumeric() && ch != '-' && ch != '_')
        .map(str::trim)
        .filter(|word| word.chars().count() >= 2)
        .map(|word| word.to_lowercase())
        .collect()
}

fn slug(value: &str) -> String {
    let slug = value
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    slug.trim_matches('-').to_string()
}

fn url_query(value: &str) -> String {
    value.trim().replace(' ', "+")
}
