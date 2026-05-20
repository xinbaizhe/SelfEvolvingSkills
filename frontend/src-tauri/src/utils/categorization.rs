pub(crate) fn derive_category(name: &str) -> String {
    let lower = name.to_lowercase();
    let prefix = lower.split('-').next().unwrap_or(&lower);
    let groups: [(&str, &[&str]); 10] = [
        ("python", &["python"]),
        ("golang", &["golang", "go"]),
        ("java", &[
            "java", "kotlin", "springboot", "jpa", "android", "compose", "gradle",
        ]),
        ("cpp", &["cpp"]),
        ("rust", &["rust"]),
        ("frontend", &[
            "frontend", "vue", "react", "nextjs", "nuxt", "typescript", "javascript",
            "flutter", "swiftui", "swift",
        ]),
        ("document", &[
            "document", "docx", "pptx", "xlsx", "pdf", "internal-comms",
        ]),
        ("workflow", &[
            "tdd", "e2e", "verification", "code-review", "debugging", "build",
            "refactor",
        ]),
        ("meta", &[
            "colleague", "continuous-learning", "skill", "configure", "eval",
            "harness", "project", "rules", "sessions", "strategic", "brainstorm",
        ]),
        ("backend", &["django", "laravel", "ktor", "exposed", "api"]),
    ];
    for (category, values) in groups {
        if values.contains(&prefix) || values.iter().any(|v| lower.contains(v)) {
            return category.to_string();
        }
    }
    "other".into()
}

