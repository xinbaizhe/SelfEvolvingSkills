use super::*;

// ============================
// sanitize_skill_name
// ============================
#[test]
fn sanitize_lowercases_and_replaces_spaces() {
    assert_eq!(utils::sanitize_skill_name("My Cool Skill"), "my-cool-skill");
}

#[test]
fn sanitize_preserves_kebab_case() {
    assert_eq!(
        utils::sanitize_skill_name("rust-code-review"),
        "rust-code-review"
    );
}

#[test]
fn sanitize_removes_special_chars() {
    assert_eq!(utils::sanitize_skill_name("hello world!"), "hello-world!");
}

#[test]
fn sanitize_empty_returns_default() {
    assert_eq!(utils::sanitize_skill_name(""), "generated-skill");
}

#[test]
fn sanitize_underscores_to_dashes() {
    assert_eq!(utils::sanitize_skill_name("my_skill_name"), "my-skill-name");
}

#[test]
fn sanitize_collapses_multiple_dashes() {
    assert_eq!(utils::sanitize_skill_name("a---b"), "a-b");
}

// ============================
// strip_markdown_fence
// ============================
#[test]
fn strip_removes_fence() {
    let input = "```\nsome content\n```";
    assert_eq!(utils::strip_markdown_fence(input), "some content");
}

#[test]
fn strip_removes_fence_with_language_tag() {
    let input = "```markdown\n# Title\n```";
    assert_eq!(utils::strip_markdown_fence(input), "# Title");
}

#[test]
fn strip_preserves_non_fenced_text() {
    let input = "plain text without fence";
    assert_eq!(utils::strip_markdown_fence(input), "plain text without fence");
}

#[test]
fn strip_handles_empty_string() {
    assert_eq!(utils::strip_markdown_fence(""), "");
}

#[test]
fn strip_handles_inner_backticks() {
    let input = "```\nuse `code` here\n```";
    assert_eq!(utils::strip_markdown_fence(input), "use `code` here");
}

// ============================
// extract_json_object
// ============================
#[test]
fn extract_simple_json() {
    let result = utils::extract_json_object(r#"{"key": "value"}"#).unwrap();
    assert_eq!(result, r#"{"key": "value"}"#);
}

#[test]
fn extract_json_in_markdown() {
    let result = utils::extract_json_object("some text\n{\"a\": 1}\nmore text").unwrap();
    assert_eq!(result, r#"{"a": 1}"#);
}

#[test]
fn extract_json_in_fence() {
    let input = "```json\n{\"name\": \"test\"}\n```";
    let result = utils::extract_json_object(input).unwrap();
    assert_eq!(result, r#"{"name": "test"}"#);
}

#[test]
fn extract_nested_json() {
    let input = r#"{"outer": {"inner": [1,2,3]}}"#;
    let result = utils::extract_json_object(input).unwrap();
    assert_eq!(result, r#"{"outer": {"inner": [1,2,3]}}"#);
}

#[test]
fn extract_no_json_errors() {
    assert!(utils::extract_json_object("no json here").is_err());
}

// ============================
// parse_qa_response
// ============================
#[test]
fn parse_with_draft_separator() {
    let content = r#"{"score": 85}
===DRAFT===
# Fixed Skill

This is a very detailed content section that is long enough to exceed the minimum 80 character length check in the parser function. It needs to be sufficiently verbose.
"#;
    let (meta, draft) = utils::parse_qa_response(content, "fallback");
    assert_eq!(meta, r#"{"score": 85}"#);
    assert!(draft.contains("# Fixed Skill"));
    assert!(draft.contains("sufficiently verbose"));
}

#[test]
fn parse_without_separator_returns_fallback() {
    let content = "{\"score\": 70, \"verdict\": \"revise\"}";
    let (meta, draft) = utils::parse_qa_response(content, "original draft");
    assert_eq!(meta, content);
    assert_eq!(draft, "original draft");
}

#[test]
fn parse_rejects_short_draft() {
    let content = "{\"score\": 50}\n===DRAFT===\nshort";
    let (_meta, draft) = utils::parse_qa_response(content, "fallback draft");
    assert_eq!(draft, "fallback draft");
}

#[test]
fn parse_strips_fence_from_draft() {
    let content = "{\"score\": 90}\n===DRAFT===\n```\n# Great Skill\n\nVery detailed content here that is long enough to pass the minimum length check.\n```";
    let (meta, draft) = utils::parse_qa_response(content, "fallback");
    assert_eq!(meta, "{\"score\": 90}");
    assert_eq!(
        draft,
        "# Great Skill\n\nVery detailed content here that is long enough to pass the minimum length check."
    );
}

// ============================
// PHASES constant
// ============================
#[test]
fn phases_have_seven_steps() {
    assert_eq!(PHASES.len(), 7);
}

#[test]
fn phases_cover_zero_to_hundred() {
    assert_eq!(PHASES[0].2, 0);
    assert_eq!(PHASES[6].3, 100);
}

#[test]
fn phases_are_contiguous() {
    for i in 1..PHASES.len() {
        assert_eq!(
            PHASES[i].2,
            PHASES[i - 1].3,
            "gap after phase {}",
            PHASES[i - 1].0
        );
    }
}

// ============================
// Heartbeat + stale cleanup (DB-dependent)
// ============================
fn test_db() -> (rusqlite::Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let conn = crate::db::open_conn(&db_path).unwrap();
    crate::db::init_db(&db_path).unwrap();
    (conn, dir)
}

fn insert_running_job(conn: &rusqlite::Connection, run_id: i64, phase: &str, started_at: &str) {
    conn.execute(
        "INSERT INTO evolution_jobs (run_id, phase, status, progress, message, started_at, created_at)
         VALUES (?1, ?2, 'running', 50, 'test message', ?3, ?3)",
        rusqlite::params![run_id, phase, started_at],
    )
    .unwrap();
}

#[test]
fn heartbeat_updates_started_at() {
    let (conn, _dir) = test_db();
    let old_time = "2025-01-01 00:00:00";
    insert_running_job(&conn, 1, "optimize", old_time);

    let db_path = _dir.path().join("test.db");
    status::heartbeat_phase(&db_path, 1, "optimize", "heartbeat test");

    let new_time: String = conn
        .query_row(
            "SELECT started_at FROM evolution_jobs WHERE run_id = 1 AND phase = 'optimize'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_ne!(new_time, old_time, "heartbeat should update started_at");
}

#[test]
fn stale_cleanup_fails_old_jobs() {
    let (conn, _dir) = test_db();
    insert_running_job(&conn, 1, "optimize", "2020-01-01 00:00:00");
    insert_running_job(&conn, 1, "qa_review", "2020-01-01 00:00:00");

    let failed = status::cleanup_stale_jobs(&conn);
    assert_eq!(failed, 2);

    let status_val: String = conn
        .query_row(
            "SELECT status FROM evolution_jobs WHERE run_id = 1 AND phase = 'optimize'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status_val, "failed");
}

#[test]
fn stale_cleanup_preserves_recent_jobs() {
    let (conn, _dir) = test_db();
    let now = crate::utils::time::now_string();
    insert_running_job(&conn, 1, "optimize", &now);

    let failed = status::cleanup_stale_jobs(&conn);
    assert_eq!(failed, 0);

    let status_val: String = conn
        .query_row(
            "SELECT status FROM evolution_jobs WHERE run_id = 1 AND phase = 'optimize'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status_val, "running");
}

#[test]
fn has_incomplete_detects_running_jobs() {
    let (conn, _dir) = test_db();
    assert!(!status::has_incomplete_run(&conn));

    let now = crate::utils::time::now_string();
    insert_running_job(&conn, 1, "discover", &now);
    assert!(status::has_incomplete_run(&conn));
}
