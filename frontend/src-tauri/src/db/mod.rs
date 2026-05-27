use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;

use crate::utils::time::now_string;

pub(crate) fn open_conn(db_path: &Path) -> Result<Connection> {
    Connection::open(db_path).with_context(|| format!("open sqlite db {}", db_path.display()))
}

pub(crate) fn init_db(db_path: &Path) -> Result<()> {
    let conn = open_conn(db_path)?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS skills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT,
            origin TEXT,
            source_type TEXT NOT NULL DEFAULT 'user',
            agent_source TEXT,
            plugin_name TEXT,
            file_path TEXT NOT NULL,
            yaml_raw TEXT,
            body_text TEXT,
            body_size INTEGER DEFAULT 0,
            line_count INTEGER DEFAULT 0,
            file_mtime REAL,
            file_size INTEGER DEFAULT 0,
            file_hash TEXT,
            category TEXT,
            category_tags TEXT,
            usage_count INTEGER DEFAULT 0,
            session_count INTEGER DEFAULT 0,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(name, source_type)
        );

        CREATE TABLE IF NOT EXISTS agents (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            description TEXT,
            tools TEXT,
            model TEXT,
            agent_source TEXT,
            file_path TEXT NOT NULL,
            yaml_raw TEXT,
            body_text TEXT,
            body_size INTEGER DEFAULT 0,
            line_count INTEGER DEFAULT 0,
            file_mtime REAL,
            file_size INTEGER DEFAULT 0,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL UNIQUE,
            pid INTEGER,
            cwd TEXT,
            project_name TEXT,
            agent_source TEXT,
            entrypoint TEXT,
            version TEXT,
            kind TEXT,
            started_at TEXT,
            message_count INTEGER DEFAULT 0,
            first_prompt TEXT,
            compressed_summary TEXT,
            jsonl_path TEXT,
            jsonl_size INTEGER DEFAULT 0,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS memories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT,
            description TEXT,
            mem_type TEXT,
            origin_session_id TEXT,
            project_name TEXT,
            agent_source TEXT,
            file_path TEXT NOT NULL,
            body_text TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS skill_usage (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            skill_name TEXT NOT NULL,
            skill_source_type TEXT NOT NULL DEFAULT 'user',
            session_id TEXT NOT NULL,
            usage_type TEXT NOT NULL DEFAULT 'listed',
            mention_count INTEGER DEFAULT 0,
            first_used_at TEXT,
            jsonl_path TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS scan_jobs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            scan_type TEXT NOT NULL DEFAULT 'full',
            status TEXT NOT NULL DEFAULT 'running',
            started_at TEXT DEFAULT CURRENT_TIMESTAMP,
            completed_at TEXT,
            skills_found INTEGER DEFAULT 0,
            agents_found INTEGER DEFAULT 0,
            sessions_found INTEGER DEFAULT 0,
            conversations_analyzed INTEGER DEFAULT 0,
            memories_found INTEGER DEFAULT 0,
            sources_scanned INTEGER DEFAULT 0,
            errors TEXT,
            config_snapshot TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS source_configs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_id TEXT NOT NULL UNIQUE,
            agent_name TEXT NOT NULL,
            detected_path TEXT,
            custom_paths TEXT,
            is_enabled INTEGER DEFAULT 1,
            is_available INTEGER DEFAULT 0,
            record_count INTEGER DEFAULT 0,
            last_activity TEXT,
            last_scan_at TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS admin_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL DEFAULT '',
            is_active INTEGER DEFAULT 1,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS app_config (
            key TEXT PRIMARY KEY,
            value TEXT,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS evolution_jobs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id INTEGER NOT NULL,
            phase TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            progress INTEGER DEFAULT 0,
            message TEXT,
            data TEXT,
            started_at TEXT,
            completed_at TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS evolution_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            status TEXT NOT NULL DEFAULT 'running',
            agent_ids TEXT,
            started_at TEXT NOT NULL,
            completed_at TEXT,
            summary TEXT
        );

        CREATE TABLE IF NOT EXISTS evolution_steps (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id INTEGER NOT NULL,
            phase TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            progress INTEGER DEFAULT 0,
            started_at TEXT,
            completed_at TEXT,
            error_message TEXT,
            degraded_reason TEXT,
            summary TEXT,
            FOREIGN KEY (run_id) REFERENCES evolution_runs(id)
        );

        CREATE TABLE IF NOT EXISTS evolution_artifacts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            step_id INTEGER NOT NULL,
            agent_name TEXT NOT NULL,
            status TEXT NOT NULL,
            input_summary TEXT,
            output_summary TEXT,
            error_message TEXT,
            duration_ms INTEGER,
            created_at TEXT NOT NULL,
            FOREIGN KEY (step_id) REFERENCES evolution_steps(id)
        );

        CREATE TABLE IF NOT EXISTS community_candidates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            step_id INTEGER NOT NULL,
            source TEXT NOT NULL,
            name TEXT NOT NULL,
            url TEXT,
            description TEXT,
            summary TEXT,
            frontmatter TEXT,
            stars INTEGER DEFAULT 0,
            license TEXT,
            relevance_score INTEGER,
            selected INTEGER DEFAULT 0,
            raw_data TEXT,
            FOREIGN KEY (step_id) REFERENCES evolution_steps(id)
        );

        CREATE TABLE IF NOT EXISTS workflow_recommendations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            step_id INTEGER NOT NULL,
            workflow_id INTEGER NOT NULL,
            recommendation_type TEXT NOT NULL,
            target_type TEXT,
            target_community_id INTEGER,
            target_local_skill_id INTEGER,
            confidence REAL DEFAULT 0,
            reason TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (step_id) REFERENCES evolution_steps(id)
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS sessions_fts USING fts5(
            first_prompt, compressed_summary, project_name, content='sessions', content_rowid='id'
        );

        CREATE TABLE IF NOT EXISTS community_skills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            repo_full_name TEXT NOT NULL,
            repo_url TEXT NOT NULL,
            stars INTEGER DEFAULT 0,
            description TEXT,
            skill_md_content TEXT,
            file_url TEXT,
            installed INTEGER DEFAULT 0,
            verified INTEGER DEFAULT 0,
            relevance_score REAL DEFAULT 0,
            quality_score REAL DEFAULT 0,
            weighted_score REAL DEFAULT 0,
            license TEXT,
            pushed_at TEXT,
            matched_file TEXT,
            readme_excerpt TEXT,
            source TEXT DEFAULT 'github',
            recommendation_reason TEXT,
            topic TEXT DEFAULT 'AI coding agent skills',
            fetched_at TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(name, repo_full_name)
        );

        CREATE TABLE IF NOT EXISTS workflow_clusters (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT,
            frequency INTEGER DEFAULT 1,
            source_agents TEXT,
            estimated_time_saved TEXT,
            can_generate_skill INTEGER DEFAULT 0,
            skill_score INTEGER DEFAULT 0,
            status TEXT DEFAULT 'pending',
            draft_body TEXT,
            sample_tasks TEXT,
            recommendation_source TEXT DEFAULT 'local-frequency',
            confidence REAL,
            reasoning TEXT,
            source_skills TEXT,
            similar_skills TEXT,
            review_score INTEGER,
            review_summary TEXT,
            review_feedback TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE INDEX IF NOT EXISTS idx_skills_name ON skills(name);
        CREATE INDEX IF NOT EXISTS idx_skills_category ON skills(category);
        CREATE INDEX IF NOT EXISTS idx_skills_agent_source ON skills(agent_source);
        CREATE INDEX IF NOT EXISTS idx_agents_agent_source ON agents(agent_source);
        CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_name);
        CREATE INDEX IF NOT EXISTS idx_source_configs_agent_id ON source_configs(agent_id);
        CREATE INDEX IF NOT EXISTS idx_evolution_jobs_status ON evolution_jobs(status);
        CREATE INDEX IF NOT EXISTS idx_community_skills_name ON community_skills(name);
        CREATE INDEX IF NOT EXISTS idx_workflow_clusters_status ON workflow_clusters(status);
        CREATE INDEX IF NOT EXISTS idx_evolution_steps_run ON evolution_steps(run_id);
        CREATE INDEX IF NOT EXISTS idx_evolution_artifacts_step ON evolution_artifacts(step_id);
        CREATE INDEX IF NOT EXISTS idx_community_candidates_step ON community_candidates(step_id);
        CREATE INDEX IF NOT EXISTS idx_workflow_recommendations_step ON workflow_recommendations(step_id);

        CREATE TABLE IF NOT EXISTS skill_variants (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            skill_name TEXT NOT NULL,
            variant_label TEXT NOT NULL,
            draft_body TEXT NOT NULL,
            status TEXT DEFAULT 'active',
            generation_run_id INTEGER,
            usage_count INTEGER DEFAULT 0,
            avg_session_messages REAL DEFAULT 0,
            performance_score REAL DEFAULT 0,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(skill_name, variant_label)
        );

        CREATE TABLE IF NOT EXISTS skill_iterations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            skill_name TEXT NOT NULL,
            iteration_num INTEGER NOT NULL DEFAULT 1,
            previous_draft TEXT,
            new_draft TEXT,
            status TEXT DEFAULT 'pending',
            trigger_reason TEXT,
            judge_result TEXT,
            critic_result TEXT,
            improvement_summary TEXT,
            run_id INTEGER,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS team_skills_cache (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            category TEXT,
            source_type TEXT,
            origin_agent TEXT,
            body_md TEXT,
            author_id INTEGER,
            dept_id INTEGER,
            compatible_models TEXT,
            compatible_agents TEXT,
            usage_count INTEGER DEFAULT 0,
            avg_score REAL DEFAULT 0,
            status TEXT DEFAULT 'published',
            version INTEGER DEFAULT 1,
            server_created_at TEXT,
            server_updated_at TEXT,
            cached_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS pending_operations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            op_type TEXT NOT NULL,
            path TEXT NOT NULL,
            method TEXT NOT NULL,
            body TEXT,
            status TEXT DEFAULT 'pending',
            error_message TEXT,
            retry_count INTEGER DEFAULT 0,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        "#,
    )?;
    let _ = conn.execute(
        "ALTER TABLE sessions ADD COLUMN compressed_summary TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE workflow_clusters ADD COLUMN review_score INTEGER",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE workflow_clusters ADD COLUMN review_summary TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE workflow_clusters ADD COLUMN review_feedback TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE community_skills ADD COLUMN verified INTEGER DEFAULT 0",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE community_skills ADD COLUMN relevance_score REAL DEFAULT 0",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE community_skills ADD COLUMN quality_score REAL DEFAULT 0",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE community_skills ADD COLUMN weighted_score REAL DEFAULT 0",
        [],
    );
    let _ = conn.execute("ALTER TABLE community_skills ADD COLUMN license TEXT", []);
    let _ = conn.execute("ALTER TABLE community_skills ADD COLUMN pushed_at TEXT", []);
    let _ = conn.execute(
        "ALTER TABLE community_skills ADD COLUMN matched_file TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE community_skills ADD COLUMN readme_excerpt TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE community_skills ADD COLUMN source TEXT DEFAULT 'github'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE community_skills ADD COLUMN recommendation_reason TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE community_skills ADD COLUMN topic TEXT DEFAULT 'AI coding agent skills'",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_community_skills_source ON community_skills(source)",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_community_skills_topic ON community_skills(topic)",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE workflow_clusters ADD COLUMN optimize_failed INTEGER DEFAULT 0",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE workflow_clusters ADD COLUMN review_failed INTEGER DEFAULT 0",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE skills ADD COLUMN iteration_num INTEGER DEFAULT 1",
        [],
    );
    let _ = conn.execute("ALTER TABLE skills ADD COLUMN last_evaluated_at TEXT", []);
    let _ = conn.execute(
        "ALTER TABLE skills ADD COLUMN effectiveness_score REAL DEFAULT 0",
        [],
    );
    let _ = conn.execute("ALTER TABLE sessions ADD COLUMN matched_skill TEXT", []);
    let _ = conn.execute(
        "ALTER TABLE workflow_clusters ADD COLUMN evolves_skill TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE workflow_clusters ADD COLUMN iteration_num INTEGER",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE workflow_clusters ADD COLUMN parent_workflow_id INTEGER",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE workflow_clusters ADD COLUMN installed_agent_id TEXT",
        [],
    );
    conn.execute(
        "INSERT OR IGNORE INTO admin_users (id, username, password_hash, is_active, created_at)
         VALUES (1, 'local', '', 1, ?1)",
        [now_string()],
    )?;
    migrate_evolution_jobs(&conn)?;
    cleanup_disk_scan_cache(&conn)?;
    Ok(())
}

fn cleanup_disk_scan_cache(conn: &Connection) -> Result<()> {
    let _ = conn.execute_batch(
        "DROP TABLE IF EXISTS disk_scan_entries;
         DROP TABLE IF EXISTS disk_scan_jobs;
         VACUUM;",
    );
    Ok(())
}

fn migrate_evolution_jobs(conn: &Connection) -> Result<()> {
    let has_run_id: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('evolution_jobs') WHERE name = 'run_id'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(false);
    if !has_run_id {
        conn.execute_batch(
            "ALTER TABLE evolution_jobs ADD COLUMN run_id INTEGER;
             UPDATE evolution_jobs SET run_id = id WHERE run_id IS NULL;",
        )?;
    }
    Ok(())
}

pub(crate) fn count_table(conn: &Connection, table: &str) -> Result<i64> {
    conn.query_row(&format!("SELECT COUNT(id) FROM {table}"), [], |row| {
        row.get(0)
    })
    .map_err(Into::into)
}

#[allow(dead_code)]
pub(crate) fn count_where(conn: &Connection, table: &str, clause: &str) -> Result<i64> {
    conn.query_row(
        &format!("SELECT COUNT(id) FROM {table} WHERE {clause}"),
        [],
        |row| row.get(0),
    )
    .map_err(Into::into)
}
