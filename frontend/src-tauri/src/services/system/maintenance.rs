use rusqlite::Connection;
use serde_json::{json, Value};

pub(crate) fn clear_all_data(conn: &Connection) -> Value {
    let tables = [
        "skills",
        "agents",
        "sessions",
        "memories",
        "skill_usage",
        "scan_jobs",
        "evolution_jobs",
        "evolution_runs",
        "evolution_steps",
        "evolution_artifacts",
        "workflow_clusters",
        "workflow_recommendations",
    ];
    let cleared = clear_tables(conn, &tables);
    json!({
        "cleared_tables": cleared,
        "message": "所有扫描数据、会话记录、进化作业、工作流聚类已清空。配置、管理员、社区 Skills 和日报已保留。"
    })
}

/// Clear scan history, evolution records, clusters, and usage logs.
/// BUSINESS DATA IS PRESERVED: skills, agents, sessions, memories, daily_reports.
pub(crate) fn clear_logs(conn: &Connection) -> Value {
    let tables = [
        "scan_jobs",
        "evolution_jobs",
        "evolution_runs",
        "evolution_steps",
        "evolution_artifacts",
        "workflow_clusters",
        "workflow_recommendations",
        "skill_usage",
    ];
    let cleared = clear_tables(conn, &tables);
    json!({
        "cleared_tables": cleared,
        "message": "扫描历史、进化记录、工作流聚类、使用记录已清空。Skills、Agents、会话、记忆、日报、社区缓存和所有配置已保留。"
    })
}

pub(crate) fn initialize_database(conn: &Connection) -> Value {
    let tables = [
        "skills",
        "agents",
        "sessions",
        "memories",
        "skill_usage",
        "scan_jobs",
        "evolution_jobs",
        "evolution_runs",
        "evolution_steps",
        "evolution_artifacts",
        "workflow_clusters",
        "workflow_recommendations",
        "community_skills",
        "community_candidates",
        "skill_variants",
        "skill_iterations",
        "team_skills_cache",
        "daily_reports",
    ];
    let cleared = clear_tables(conn, &tables);

    let _ = conn.execute(
        "UPDATE source_configs
         SET detected_path = NULL, is_available = 0, record_count = 0,
             last_activity = NULL, last_scan_at = NULL, updated_at = CURRENT_TIMESTAMP",
        [],
    );

    json!({
        "cleared_tables": cleared,
        "message": "数据库已初始化。已保留管理员、系统配置和数据源配置。业务数据、Skills、Agents、会话、记忆、社区缓存与历史记录已清空。"
    })
}

fn clear_tables(conn: &Connection, tables: &[&str]) -> Vec<String> {
    let mut cleared = Vec::new();
    for table in tables {
        if conn.execute(&format!("DELETE FROM {table}"), []).is_ok() {
            cleared.push((*table).to_string());
        }
    }
    cleared
}
