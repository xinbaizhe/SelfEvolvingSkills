use std::collections::HashMap;

pub(super) fn tokenize(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_alphanumeric() && ch != '-' && ch != '_')
        .filter(|word| word.len() >= 2)
        .map(|word| word.to_lowercase())
        .collect()
}

pub(super) fn jaccard_similarity(a: &[String], b: &[String]) -> f64 {
    let set_a = a.iter().collect::<std::collections::HashSet<_>>();
    let set_b = b.iter().collect::<std::collections::HashSet<_>>();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}

fn is_stop_word(word: &str) -> bool {
    matches!(
        word,
        "the" | "and" | "for" | "with" | "this" | "that" | "请" | "帮我" | "一个" | "这个"
    )
}

pub(super) fn classify_prompt(prompt: &str) -> Option<String> {
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

pub(super) fn extract_keywords(prompts: &[&str]) -> Vec<String> {
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

pub(super) fn skill_name_from_keywords(
    base_name: &str,
    keywords: &[String],
    texts: &[&str],
) -> String {
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
