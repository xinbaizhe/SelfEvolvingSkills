//! Progress messages the evolution pipeline reports.
//!
//! A message travels to two consumers that need different things. The event
//! stream renders it live and has to follow the interface language; the
//! `evolution_jobs` table stores it and is replayed later by the history view.
//! Carrying only Chinese prose satisfies neither.
//!
//! So every message carries three parts:
//!
//! * `code` - the frontend's catalog key, so the frontend can render the
//!   message in the interface language.
//! * `params` - the values that fill the code's placeholders.
//! * `text` - the Chinese prose, kept because a code the frontend does not
//!   recognise has to fall back to something readable, and because the stored
//!   rows and the logs should stay legible on their own.
//!
//! The code is deliberately the catalog key rather than a domain identifier
//! needing a second lookup table. A key is an identifier, not locale knowledge,
//! so the backend still holds no language of its own; and one name for one
//! thing cannot drift the way a mapping table can.

use serde_json::{json, Value};

/// A progress message in all three of the forms its consumers need.
pub(super) struct ProgressMsg {
    /// Catalog key the frontend renders.
    pub code: &'static str,
    /// Values filling the placeholders in the catalog entry.
    pub params: Value,
    /// Chinese prose, for unrecognised codes, stored rows and logs.
    pub text: String,
}

impl ProgressMsg {
    /// A message whose text has no placeholders.
    pub(super) fn new(code: &'static str, text: impl Into<String>) -> Self {
        Self {
            code,
            params: json!({}),
            text: text.into(),
        }
    }

    /// A message whose text interpolates `params`.
    pub(super) fn with(code: &'static str, params: Value, text: impl Into<String>) -> Self {
        Self {
            code,
            params,
            text: text.into(),
        }
    }
}

// The catalog keys below are asserted against both locale files by the
// `progress_codes_have_catalog_entries` test, so a typo fails the build rather
// than silently falling back to Chinese on an English screen.
pub(super) const DISCOVERING: &str = "core.pipeline.progress.discovering";
pub(super) const DB_OPEN_FAILED: &str = "core.pipeline.progress.dbOpenFailed";
pub(super) const DETECT_FAILED: &str = "core.pipeline.progress.detectFailed";
pub(super) const DETECT_DONE: &str = "core.pipeline.progress.detectDone";
pub(super) const SKILLS_FOUND: &str = "core.pipeline.progress.skillsFound";
pub(super) const AGENTS_FOUND: &str = "core.pipeline.progress.agentsFound";
pub(super) const HISTORY_SUMMARIZED: &str = "core.pipeline.progress.historySummarized";
pub(super) const COMMUNITY_SEARCHING: &str = "core.pipeline.progress.communitySearching";
pub(super) const COMMUNITY_DONE: &str = "core.pipeline.progress.communityDone";
pub(super) const COMMUNITY_FAILED: &str = "core.pipeline.progress.communityFailed";
pub(super) const CLUSTERING: &str = "core.pipeline.progress.clustering";
pub(super) const CLUSTER_DONE: &str = "core.pipeline.progress.clusterDone";
pub(super) const CLUSTER_REVIEWED: &str = "core.pipeline.progress.clusterReviewed";
pub(super) const CLUSTER_LOCAL_ONLY: &str = "core.pipeline.progress.clusterLocalOnly";
pub(super) const CLUSTER_REVIEW_FAILED: &str = "core.pipeline.progress.clusterReviewFailed";
pub(super) const CLUSTER_SAVED: &str = "core.pipeline.progress.clusterSaved";
pub(super) const DRAFTING: &str = "core.pipeline.progress.drafting";
pub(super) const DRAFTS_GENERATED: &str = "core.pipeline.progress.draftsGenerated";
pub(super) const OPTIMIZING: &str = "core.pipeline.progress.optimizing";
pub(super) const DRAFTS_OPTIMIZED: &str = "core.pipeline.progress.draftsOptimized";
pub(super) const OPTIMIZE_LOCAL_ONLY: &str = "core.pipeline.progress.optimizeLocalOnly";
pub(super) const OPTIMIZE_FAILED: &str = "core.pipeline.progress.optimizeFailed";
pub(super) const VARIANTS_GENERATED: &str = "core.pipeline.progress.variantsGenerated";
pub(super) const REVIEWING: &str = "core.pipeline.progress.reviewing";
pub(super) const REVIEW_DONE: &str = "core.pipeline.progress.reviewDone";
pub(super) const REVIEW_SKIPPED: &str = "core.pipeline.progress.reviewSkipped";
pub(super) const REVIEW_FAILED: &str = "core.pipeline.progress.reviewFailed";
pub(super) const COMPARING: &str = "core.pipeline.progress.comparing";
pub(super) const COMPARE_DONE: &str = "core.pipeline.progress.compareDone";
pub(super) const RECOMMEND_DONE: &str = "core.pipeline.progress.recommendDone";
pub(super) const RUN_COMPLETED: &str = "core.pipeline.progress.runCompleted";
pub(super) const HEARTBEAT_OPTIMIZING: &str = "core.pipeline.progress.heartbeatOptimizing";
pub(super) const HEARTBEAT_REVIEWING: &str = "core.pipeline.progress.heartbeatReviewing";
pub(super) const HEARTBEAT_RETRY: &str = "core.pipeline.progress.heartbeatRetry";
/// Appended by the stale-job sweep to whatever message the phase last wrote.
pub(super) const SUFFIX_TIMEOUT: &str = "core.pipeline.progress.suffixTimeout";
/// Appended by a manual reset, for the same reason as [`SUFFIX_TIMEOUT`].
pub(super) const SUFFIX_RESET: &str = "core.pipeline.progress.suffixReset";
pub(super) const ABORTED: &str = "core.pipeline.aborted";

#[cfg(test)]
mod tests {
    use super::*;

    /// Every code declared above.
    ///
    /// A list rather than a computation because Rust cannot enumerate its own
    /// constants. It cannot fall behind: `declared_codes_are_all_listed` reads
    /// this file back and fails on a declaration the list does not carry.
    const ALL: &[&str] = &[
        DISCOVERING,
        DB_OPEN_FAILED,
        DETECT_FAILED,
        DETECT_DONE,
        SKILLS_FOUND,
        AGENTS_FOUND,
        HISTORY_SUMMARIZED,
        COMMUNITY_SEARCHING,
        COMMUNITY_DONE,
        COMMUNITY_FAILED,
        CLUSTERING,
        CLUSTER_DONE,
        CLUSTER_REVIEWED,
        CLUSTER_LOCAL_ONLY,
        CLUSTER_REVIEW_FAILED,
        CLUSTER_SAVED,
        DRAFTING,
        DRAFTS_GENERATED,
        OPTIMIZING,
        DRAFTS_OPTIMIZED,
        OPTIMIZE_LOCAL_ONLY,
        OPTIMIZE_FAILED,
        VARIANTS_GENERATED,
        REVIEWING,
        REVIEW_DONE,
        REVIEW_SKIPPED,
        REVIEW_FAILED,
        COMPARING,
        COMPARE_DONE,
        RECOMMEND_DONE,
        RUN_COMPLETED,
        HEARTBEAT_OPTIMIZING,
        HEARTBEAT_REVIEWING,
        HEARTBEAT_RETRY,
        SUFFIX_TIMEOUT,
        SUFFIX_RESET,
        ABORTED,
    ];

    const SOURCE: &str = include_str!("msg.rs");

    /// The `pub(super) const NAME: &str = "value";` declarations in this file.
    fn declared_codes() -> Vec<(&'static str, &'static str)> {
        SOURCE
            .lines()
            .filter_map(|line| line.strip_prefix("pub(super) const "))
            .filter_map(|rest| {
                let (name, tail) = rest.split_once(": &str = \"")?;
                let value = tail.strip_suffix("\";")?;
                Some((name, value))
            })
            .collect()
    }

    #[test]
    fn declared_codes_are_all_listed() {
        let declared = declared_codes();
        assert_eq!(
            declared.len(),
            ALL.len(),
            "a code was declared or removed without updating ALL"
        );
        for (name, value) in declared {
            assert!(
                ALL.contains(&value),
                "`{name}` is declared but missing from ALL"
            );
        }
        let unique: std::collections::HashSet<_> = ALL.iter().collect();
        assert_eq!(ALL.len(), unique.len(), "ALL lists the same code twice");
    }

    fn catalog(locale: &str) -> Value {
        let path = format!(
            "{}/../src/i18n/locales/{locale}/core.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let raw =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("cannot parse {path}: {e}"))
    }

    /// Walks a dotted key, so `core.pipeline.aborted` finds a nested entry.
    fn entry<'a>(catalog: &'a Value, code: &str) -> Option<&'a str> {
        let mut node = catalog;
        for part in code.split('.') {
            node = node.get(part)?;
        }
        node.as_str()
    }

    /// The `{placeholders}` a catalog entry interpolates.
    fn placeholders(text: &str) -> std::collections::BTreeSet<&str> {
        text.split('{')
            .skip(1)
            .filter_map(|rest| rest.split_once('}').map(|(name, _)| name))
            .collect()
    }

    /// A code with no catalog entry would silently fall back to Chinese prose
    /// on an English screen - the exact failure this whole mechanism exists to
    /// prevent. Both locales, because a key present in only one of them is just
    /// as invisible.
    #[test]
    fn progress_codes_have_catalog_entries() {
        for locale in ["en", "zh-CN"] {
            let catalog = catalog(locale);
            for code in ALL {
                assert!(
                    entry(&catalog, code).is_some(),
                    "{locale}/core.json has no entry for `{code}`"
                );
            }
        }
    }

    /// The two locales have to agree on placeholders. A mismatch renders a
    /// literal `{count}` on screen in whichever language is wrong, and the key
    /// parity check the frontend runs cannot see it.
    #[test]
    fn catalog_entries_agree_on_placeholders() {
        let (en, zh) = (catalog("en"), catalog("zh-CN"));
        for code in ALL {
            let (en_text, zh_text) = (entry(&en, code).unwrap(), entry(&zh, code).unwrap());
            assert_eq!(
                placeholders(en_text),
                placeholders(zh_text),
                "`{code}` interpolates different values in each locale"
            );
        }
    }
}
