use serde::{Deserialize, Serialize};

mod optimize;
mod pipeline;
mod status;
mod utils;

pub(crate) use pipeline::start_evolution_pipeline;
pub(crate) use status::{
    cleanup_stale_jobs, get_evolution_status, has_incomplete_run, list_evolution_history,
    reset_stuck_evolution,
};

#[derive(Debug, Clone)]
pub(super) struct LlmConfig {
    pub(super) base_url: String,
    pub(super) api_key: String,
    pub(super) model: String,
    pub(super) api_format: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[allow(dead_code)]
pub(super) struct SkillReviewResponse {
    pub(super) name: Option<String>,
    pub(super) description: Option<String>,
    pub(super) draft_body: Option<String>,
    pub(super) score: Option<i64>,
    pub(super) verdict: Option<String>,
    pub(super) summary: Option<String>,
    pub(super) safety: Option<Vec<String>>,
    pub(super) performance: Option<Vec<String>>,
    pub(super) functionality: Option<Vec<String>>,
    pub(super) writing: Option<Vec<String>>,
    pub(super) improvements: Option<Vec<String>>,
}

pub(super) const PHASES: &[(&str, &str, i64, i64)] = &[
    ("discover", "扫描发现", 0, 20),
    ("reference_retrieval", "参考检索", 20, 32),
    ("cluster", "聚类分析", 32, 50),
    ("draft_generate", "生成草稿", 50, 62),
    ("optimize", "智能优化", 62, 78),
    ("qa_review", "质量评审", 78, 90),
    ("diff_recommend", "差异推荐", 90, 100),
];

/// Minimum QA review score before auto re-optimization kicks in
pub(super) const MIN_REVIEW_SCORE: i64 = 75;
/// Maximum feedback-fix retry cycles
pub(super) const MAX_REVIEW_RETRIES: u32 = 3;

#[cfg(test)]
mod tests;
