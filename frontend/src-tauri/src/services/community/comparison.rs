use anyhow::{anyhow, Result};
use reqwest::header::ACCEPT;
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

use super::utils::{self, extract_json_object};
use super::{CommunityRecommendation, LlmConfig};

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

pub(crate) fn compare_with_community(
    conn: &Connection,
    clusters: &[crate::services::workflow::WorkflowCluster],
) -> i64 {
    let mut comparisons = 0;

    for cluster in clusters.iter().filter(|cluster| cluster.can_generate_skill) {
        let Ok(workflow_id) =
            crate::services::workflow::workflow_id_for_cluster(conn, cluster)
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
            let now = crate::utils::time::now_string();
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

pub(crate) async fn compare_with_llm(db_path: &Path, body: Option<Value>) -> Result<Value> {
    let body = body.ok_or_else(|| anyhow!("Missing request body"))?;
    let draft_body = body
        .get("draft_body")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Missing draft_body"))?;
    let draft_name = body
        .get("draft_name")
        .and_then(Value::as_str)
        .unwrap_or("本地草稿");
    let community_name = body
        .get("community_name")
        .and_then(Value::as_str)
        .unwrap_or("社区 Skill");
    let community_content = body
        .get("community_content")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Missing community_content"))?;

    let config = match llm_config(db_path)? {
        Some(c) => c,
        None => {
            let draft_sections: Vec<&str> =
                draft_body.lines().filter(|l| l.starts_with('#')).collect();
            let community_sections: Vec<&str> = community_content
                .lines()
                .filter(|l| l.starts_with('#'))
                .collect();
            return Ok(json!({
                "dimensions": [
                    { "label": "结构对比", "local": format!("{} 个章节", draft_sections.len()), "community": format!("{} 个章节", community_sections.len()), "verdict": "neutral" },
                    { "label": "内容量", "local": format!("{} 字符", draft_body.chars().count()), "community": format!("{} 字符", community_content.chars().count()), "verdict": "neutral" },
                ],
                "suggestions": ["需要配置 LLM 才能生成智能对比分析"],
                "source": "heuristic"
            }));
        }
    };

    let prompt = format!(
        "请对比以下两个 Skill 定义，给出详细的结构化分析。\n\n\
         【本地草稿】{}：\n{}\n\n\
         【社区 Skill】{}：\n{}\n\n\
         返回格式：只返回一个 JSON 对象，格式为：\n\
         {{\n\
           \"dimensions\": [\n\
             {{\"label\": \"维度名\", \"local\": \"本地内容摘要\", \"community\": \"社区内容摘要\", \"verdict\": \"local_better|community_better|complementary|neutral\"}}\n\
           ],\n\
           \"suggestions\": [\"可操作的改进建议1\", \"建议2\"],\n\
           \"summary\": \"一段中文总结，指出本地草稿最应该从社区 Skill 学什么\"\n\
         }}\n\n\
         对比维度至少包含：定位与适用场景、结构完整性、步骤可执行性、安全注意事项、示例质量。",
        draft_name, truncate_for_prompt(draft_body),
        community_name, truncate_for_prompt(community_content)
    );

    let content = call_llm(
        &config,
        "你是 Skill 对比分析专家。对比本地草稿和社区 Skill，找出差异和可改进之处。只返回 JSON，不要 Markdown。",
        &prompt,
    )
    .await?;

    let parsed: Value = serde_json::from_str(&extract_json(&content)).unwrap_or_else(|_| {
        json!({
            "dimensions": [],
            "suggestions": ["LLM 返回格式异常，请重试"],
            "summary": "对比分析失败",
            "source": "error"
        })
    });

    Ok(match parsed {
        Value::Object(mut map) => {
            map.insert("source".to_string(), json!("llm"));
            Value::Object(map)
        }
        other => other,
    })
}

pub(crate) async fn fetch_top_community_skills(
    db_path: &Path,
    clusters: &[crate::services::workflow::WorkflowCluster],
) -> Result<i64> {
    let conn = crate::db::open_conn(db_path)?;
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
        let conn = crate::db::open_conn(db_path)?;
        utils::save_recommendations(&conn, query, &llm_items)?;
    }

    let conn = crate::db::open_conn(db_path)?;
    let after = crate::db::count_table(&conn, "community_skills").unwrap_or(before);
    Ok((after - before).max(0))
}

pub(super) async fn llm_recommend_community_skills(
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

    let parsed: LlmRecommendationResponse =
        serde_json::from_str(&extract_json_object(&content)?)?;
    let mut recommendations = parsed
        .skills
        .into_iter()
        .take(5)
        .enumerate()
        .map(|(index, item)| {
            let safe_name = item.name.trim().chars().take(120).collect::<String>();
            let fallback_repo =
                format!("llm-recommendation/{}", utils::slug(&safe_name));
            let repo_full_name = item
                .repo_full_name
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(fallback_repo);
            let repo_url = item
                .repo_url
                .filter(|value| value.starts_with("https://github.com/"))
                .unwrap_or_else(|| {
                    format!(
                        "https://github.com/search?q={}",
                        utils::url_query(&safe_name)
                    )
                });
            let description = item
                .description
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| {
                    "LLM recommended community Skill reference.".to_string()
                });
            let action = item.action.unwrap_or_else(|| "add".to_string());
            let reason = item
                .reason
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| {
                    "它是什么：这是大模型判断适合当前主题的社区 Skill 或 Agent 指令参考。\n\n对 Skills 工作台有什么用：可用于补充社区参考来源，帮助对比本地生成草稿和外部成熟实践。\n\n适合什么时候看：当你想判断当前本地 Skill 是否应该合并、替换、重写或增加步骤时，可以打开来源核对结构。\n\n建议优先看什么：README、示例 prompt、agent instructions、workflow 或 rules 文件。".to_string()
                });
            let description = format!(
                "{} 推荐动作：{}。",
                description,
                if action == "replace" {
                    "替换"
                } else {
                    "新增"
                }
            );
            let score = item
                .score
                .unwrap_or(0.92 - index as f64 * 0.04)
                .clamp(0.0, 1.0);
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

pub(super) fn llm_config(db_path: &Path) -> Result<Option<LlmConfig>> {
    let conn = crate::db::open_conn(db_path)?;
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

pub(super) async fn call_llm(
    config: &LlmConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
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
                last_error = format!("大模型网络连接失败：{}", err);
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
            last_error = format!("大模型 API 返回空响应 (HTTP {})", status.as_u16());
            continue;
        }

        let text = String::from_utf8_lossy(&bytes).into_owned();

        if !status.is_success() {
            let preview: String = text.chars().take(500).collect();
            last_error = format!(
                "大模型 API 请求失败：HTTP {} {}",
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
                        last_error = "大模型返回了空的 content 字段".to_string();
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

pub(super) async fn enrich_recommendations_with_github_metadata(
    items: &mut [CommunityRecommendation],
) {
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
        item.weighted_score = utils::weighted_score(
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

fn truncate_for_prompt(text: &str) -> String {
    if text.chars().count() > 6000 {
        format!(
            "{}...（已截断）",
            text.chars().take(6000).collect::<String>()
        )
    } else {
        text.to_string()
    }
}

fn extract_json(text: &str) -> String {
    let start = text.find('{').unwrap_or(0);
    let end = text.rfind('}').unwrap_or(text.len() - 1);
    text[start..=end].to_string()
}
