use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;

use crate::db;
use crate::services::workflow::{self, WorkflowCluster};
use crate::utils::time::now_string;

use super::status;
use super::utils;
use super::SkillReviewResponse;

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

pub(super) async fn refine_clusters_with_llm(
    db_path: &Path,
    clusters: &mut [WorkflowCluster],
) -> Result<bool> {
    if clusters.is_empty() {
        return Ok(false);
    }
    if utils::llm_config(db_path)?.is_none() {
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

    let content = utils::call_llm(
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

    let parsed: LlmClusterResponse = serde_json::from_str(&utils::extract_json_object(&content)?)?;
    for decision in parsed.clusters {
        if let Some(cluster) = clusters
            .iter_mut()
            .find(|cluster| cluster.name == decision.name)
        {
            if let Some(new_name) = decision.new_name.filter(|value| !value.trim().is_empty()) {
                cluster.name = utils::sanitize_skill_name(&new_name);
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
                let target = utils::sanitize_skill_name(&merge_into);
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

pub(super) async fn multi_agent_review_draft(
    db_path: &Path,
    cluster: &WorkflowCluster,
    draft_body: &str,
    review_feedback: Option<&str>,
) -> Result<String> {
    let context = serde_json::to_string(&json!({
        "name": cluster.name,
        "description": cluster.description,
        "frequency": cluster.frequency,
        "source_agents": cluster.source_agents,
        "estimated_time_saved": cluster.estimated_time_saved,
        "sample_tasks": cluster.sample_tasks,
    }))?;

    let feedback_section = review_feedback
        .map(|fb| format!("\n\n上次 QA 评审反馈（请重点修正这些问题）：\n{}\n", fb))
        .unwrap_or_default();
    let judge_prompt = format!(
        "请审查以下 Skill 草稿。\n\n工作流上下文：\n{}\n\n当前草稿：\n{}\n{}{}\
         返回格式：只返回一个 JSON 对象，格式为：\n\
         {{\"issues\":[\"问题1\",\"问题2\"],\"suggestions\":[\"建议1\",\"建议2\"],\"score\":0-100,\"verdict\":\"keep|revise|discard\"}}\n\
         \n审查维度（逐条检查，不遗漏）：\n\
         1. 名称是否具体、唯一（不能是泛化大类名如 frontend-ui/code-review/data-processing）\n\
         2. 描述是否清晰描述了触发场景和解决的问题\n\
         3. 步骤是否可执行、不空洞（不能写\"根据需求做X\"，每个步骤必须有明确输入输出）\n\
         4. 是否缺少安全注意事项段落\n\
         5. 是否有虚假工具或危险默认操作\n\
         6. frontmatter 是否完整（name, description, allowed-tools 等必要字段）\n\
         7. 是否包含\"不要使用时机\"或等效的边界说明",
        context, draft_body, feedback_section,
        if review_feedback.is_some() { "\n\n注意：这是二次修正，必须解决上次评审反馈中提到的所有问题。\n" } else { "" }
    );
    let judge_raw = utils::call_llm(
        db_path,
        "你是严格的 Skill 评审专家。仔细审查草稿中的每个问题，不要遗漏。你的评审将被另一个 Agent 批判检查。",
        &judge_prompt,
    )
    .await?;
    let judge_json = utils::extract_json_object(&judge_raw).unwrap_or_else(|_| judge_raw.clone());

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
    let critic_raw = utils::call_llm(
        db_path,
        "你是严格的评审批判者。你的任务是找出评审中的漏洞、误判和遗漏。即使评审看起来不错，也必须找出可以改进的地方。",
        &critic_prompt,
    )
    .await?;
    let critic_json =
        utils::extract_json_object(&critic_raw).unwrap_or_else(|_| critic_raw.clone());

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
         5. 不编造工具或流程，所有内容只能基于给定的工作流上下文\n\
         \n\
         输出前自查清单（逐条确认，全部满足才输出）：\n\
         a. YAML frontmatter 包含 name, description, allowed-tools 字段且值正确\n\
         b. name 是具体的 kebab-case 技能名，不是泛化类别名\n\
         c. description 一句话说清触发场景和解决什么问题\n\
         d. 每个执行步骤都有明确的输入和预期输出，不含\"根据需求\"等模糊表述\n\
         e. \"安全注意事项\"段落不可为空，至少要列出1条\n\
         f. \"不要使用时机\"/\"边界说明\"段落标明什么时候不应该用这个 Skill\n\
         g. 没有编造任何不存在的工具名称或 API\n\
         h. 步骤数量在 3-8 之间，不过少也不过长",
        context, draft_body, judge_json, critic_json
    );
    let fixer_raw = utils::call_llm(
        db_path,
        "你是资深 AI Agent Skill 修复专家。你的任务是综合评审和批判意见，生成最优质的 SKILL.md。直接输出 Markdown，不要解释。",
        &fixer_prompt,
    )
    .await?;

    let fixed = utils::strip_markdown_fence(&fixer_raw);
    if fixed.trim().len() < 80 {
        return Err(anyhow!(
            "Multi-agent review produced too-short output ({} chars)",
            fixed.trim().len()
        ));
    }
    Ok(fixed)
}

pub(super) async fn optimize_drafts_with_llm(
    db_path: &Path,
    clusters: &[WorkflowCluster],
    run_id: i64,
) -> Result<i64> {
    if utils::llm_config(db_path)?.is_none() {
        return Ok(0);
    }

    let mut optimized = 0;
    for cluster in clusters
        .iter()
        .filter(|cluster| cluster.can_generate_skill && cluster.skill_score >= 60)
        .take(5)
    {
        status::heartbeat_phase(
            db_path,
            run_id,
            "optimize",
            &format!("正在优化：{}...", cluster.name),
        );

        let conn = db::open_conn(db_path)?;
        let id = workflow::workflow_id_for_cluster(&conn, cluster)?;
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

        let fixed = match multi_agent_review_draft(db_path, cluster, &draft_body, None).await {
            Ok(f) => f,
            Err(_) => draft_body.clone(),
        };

        if fixed != draft_body {
            let iter_num: i64 = conn
                .query_row(
                    "SELECT COALESCE(MAX(iteration_num), 0) + 1 FROM skill_iterations WHERE skill_name = ?1",
                    [&cluster.name],
                    |row| row.get(0),
                )
                .unwrap_or(2);
            status::record_skill_iteration(
                db_path,
                &cluster.name,
                iter_num,
                Some(&draft_body),
                &fixed,
                "多Agent优化（Judge→Critic→Fixer）",
                None,
                None,
                Some("三Agent审查完成Skill草稿优化"),
                run_id,
            );
        }

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

pub(super) async fn qa_drafts_with_llm(
    db_path: &Path,
    clusters: &mut [WorkflowCluster],
    run_id: i64,
) -> Result<i64> {
    if utils::llm_config(db_path)?.is_none() {
        return Ok(0);
    }

    let mut fixed = 0;
    let mut seen_names = std::collections::HashSet::new();
    for cluster in clusters
        .iter_mut()
        .filter(|cluster| cluster.can_generate_skill && cluster.skill_score >= 60)
        .take(8)
    {
        status::heartbeat_phase(
            db_path,
            run_id,
            "qa_review",
            &format!("正在评审：{}...", cluster.name),
        );
        let conn = db::open_conn(db_path)?;
        let id = workflow::workflow_id_for_cluster(&conn, cluster)?;
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

        let draft_for_prompt = if draft_body.chars().count() > 8000 {
            let truncated: String = draft_body.chars().take(8000).collect();
            format!("{}\n\n[... 草稿过长，已截断 ...]", truncated)
        } else {
            draft_body.clone()
        };

        let content = utils::call_llm(
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

        let (meta_json, mut best_draft) = utils::parse_qa_response(&content, &draft_body);
        let initial_parsed: SkillReviewResponse = match utils::extract_json_object(&meta_json) {
            Ok(json_str) => match serde_json::from_str(&json_str) {
                Ok(p) => p,
                Err(e) => {
                    match utils::extract_json_object(&content)
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

        let mut best_score = initial_parsed.score.unwrap_or(0).clamp(0, 100);
        let mut best_parsed = initial_parsed;

        // ---- Feedback-Fix Loop ----
        let mut retries: u32 = 0;
        while best_score < super::MIN_REVIEW_SCORE && retries < super::MAX_REVIEW_RETRIES {
            retries += 1;
            status::heartbeat_phase(
                db_path,
                run_id,
                "qa_review",
                &format!(
                    "反馈修正第 {}/{} 轮：{}...",
                    retries,
                    super::MAX_REVIEW_RETRIES,
                    cluster.name
                ),
            );
            let review_feedback_str = serde_json::to_string(&json!({
                "score": best_score,
                "verdict": best_parsed.verdict,
                "improvements": best_parsed.improvements,
                "safety": best_parsed.safety,
                "functionality": best_parsed.functionality,
                "writing": best_parsed.writing,
            }))
            .unwrap_or_default();

            let improved = match multi_agent_review_draft(
                db_path,
                cluster,
                &best_draft,
                Some(&review_feedback_str),
            )
            .await
            {
                Ok(d) => d,
                Err(_) => continue,
            };

            let improved_for_prompt = if improved.chars().count() > 8000 {
                let truncated: String = improved.chars().take(8000).collect();
                format!("{}\n\n[... 草稿过长，已截断 ...]", truncated)
            } else {
                improved.clone()
            };
            let re_review_content = match utils::call_llm(
                db_path,
                "你是一个严格的 Draft QA Agent。这是二次修正后的草稿，请重新评审。",
                &format!(
                    "请检查并修正下面的 SKILL.md 草稿（第{}次修正）。\n\
                     返回格式：先返回一行 JSON 元数据，然后一个 ===DRAFT=== 分隔行，最后是完整的修正后 SKILL.md。\n\
                     JSON 格式：{{\"name\":\"kebab-case-skill-name\",\"description\":\"一句清晰描述\",\"score\":0-100,\"verdict\":\"install|revise|merge|discard\",\"summary\":\"一句中文总结\",\"safety\":[\"...\"],\"performance\":[\"...\"],\"functionality\":[\"...\"],\"writing\":[\"...\"],\"improvements\":[\"...\"]}}\n\
                     \n修正要求同上次。\n\
                     \n工作流上下文：{}\n\
                     修正后草稿：\n{}",
                    retries + 1,
                    serde_json::to_string(&json!({
                        "name": cluster.name,
                        "description": cluster.description,
                        "frequency": cluster.frequency,
                        "source_agents": cluster.source_agents,
                        "sample_tasks": cluster.sample_tasks,
                    }))?,
                    improved_for_prompt
                ),
            )
            .await
            {
                Ok(c) => c,
                Err(_) => continue,
            };

            let (re_meta_json, re_draft) = utils::parse_qa_response(&re_review_content, &improved);
            let re_parsed: SkillReviewResponse = match utils::extract_json_object(&re_meta_json)
                .ok()
                .and_then(|j| serde_json::from_str(&j).ok())
            {
                Some(p) => p,
                None => continue,
            };

            let re_score = re_parsed.score.unwrap_or(0).clamp(0, 100);
            if re_score > best_score {
                best_score = re_score;
                best_parsed = re_parsed;
                best_draft = re_draft;
            }
        }

        let mut fixed_name = best_parsed
            .name
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| cluster.name.clone());
        fixed_name = utils::sanitize_skill_name(&fixed_name);
        if !seen_names.insert(fixed_name.clone()) {
            fixed_name = format!("{}-{}", fixed_name, id);
            seen_names.insert(fixed_name.clone());
        }
        let fixed_description = best_parsed
            .description
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| cluster.description.clone());
        let feedback = json!({
            "verdict": best_parsed.verdict.clone().unwrap_or_else(|| "revise".to_string()),
            "safety": best_parsed.safety.clone().unwrap_or_default(),
            "performance": best_parsed.performance.clone().unwrap_or_default(),
            "functionality": best_parsed.functionality.clone().unwrap_or_default(),
            "writing": best_parsed.writing.clone().unwrap_or_default(),
            "improvements": best_parsed.improvements.clone().unwrap_or_default(),
            "feedback_loops": retries,
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
                utils::strip_markdown_fence(&best_draft),
                best_score,
                best_parsed.summary.clone().unwrap_or_else(|| {
                    if retries > 0 {
                        format!("Draft QA Agent 已完成检测并修正草稿（经过 {} 轮反馈修正）", retries)
                    } else {
                        "Draft QA Agent 已完成检测并修正草稿。".to_string()
                    }
                }),
                serde_json::to_string(&feedback)?,
                now_string(),
            ],
        )?;
        cluster.name = fixed_name;
        cluster.description = fixed_description;

        {
            let conn = db::open_conn(db_path)?;
            let iter_num: i64 = conn
                .query_row(
                    "SELECT COALESCE(MAX(iteration_num), 0) + 1 FROM skill_iterations WHERE skill_name = ?1",
                    [&cluster.name],
                    |row| row.get(0),
                )
                .unwrap_or(2);
            let iter_summary = best_parsed.summary.clone().unwrap_or_else(|| {
                if retries > 0 {
                    format!("QA评审完成，经过{}轮反馈修正，评分{}", retries, best_score)
                } else {
                    format!("QA评审完成，评分{}", best_score)
                }
            });
            let trigger = if retries > 0 {
                format!("QA评审 + {}轮反馈修正", retries)
            } else {
                "QA评审与修正".to_string()
            };
            let judge_json = serde_json::to_string(&best_parsed).unwrap_or_default();
            let final_draft = utils::strip_markdown_fence(&best_draft);
            status::record_skill_iteration(
                db_path,
                &cluster.name,
                iter_num,
                Some(&draft_body),
                &final_draft,
                &trigger,
                Some(&judge_json),
                None,
                Some(&iter_summary),
                run_id,
            );
        }

        fixed += 1;
    }
    Ok(fixed)
}

pub(super) async fn generate_ab_variants(
    db_path: &Path,
    clusters: &[WorkflowCluster],
) -> Result<i64> {
    if utils::llm_config(db_path)?.is_none() {
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
        let id = workflow::workflow_id_for_cluster(&conn, cluster)?;
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

        let run_id: Option<i64> = conn
            .query_row("SELECT MAX(run_id) FROM evolution_jobs", [], |row| {
                row.get(0)
            })
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
                draft_body,
                label,
                strategy,
                variant = label.to_lowercase()
            );

            let variant_raw = match utils::call_llm(
                db_path,
                "你是资深 Skill 变体生成专家。根据指定策略生成不同风格的 Skill 版本。直接输出 Markdown。",
                &variant_prompt,
            )
            .await
            {
                Ok(content) => content,
                Err(_) => continue,
            };

            let variant_md = utils::strip_markdown_fence(&variant_raw);
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

pub(super) fn generate_recommendations(
    conn: &Connection,
    step_id: i64,
    clusters: &[WorkflowCluster],
) {
    for cluster in clusters.iter().filter(|c| c.can_generate_skill) {
        let Ok(id) = workflow::workflow_id_for_cluster(conn, cluster) else {
            continue;
        };

        let community_match: Option<(i64, String, i64)> = conn
            .query_row(
                "SELECT id, repo_full_name, stars FROM community_skills
                 WHERE name LIKE ?1 OR ?1 LIKE '%' || name || '%'
                 ORDER BY stars DESC LIMIT 1",
                params![cluster.name],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok();

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
