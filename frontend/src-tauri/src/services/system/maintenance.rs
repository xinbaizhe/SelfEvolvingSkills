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
        "workflow_clusters",
    ];
    let cleared = clear_tables(conn, &tables);
    json!({
        "cleared_tables": cleared,
        "message": "所有扫描数据、会话记录、进化作业和工作流聚类已清空。配置和社区 Skills 已保留。"
    })
}

pub(crate) fn clear_logs(conn: &Connection) -> Value {
    let tables = [
        "scan_jobs",
        "evolution_jobs",
        "workflow_clusters",
        "memories",
        "skill_usage",
    ];
    let cleared = clear_tables(conn, &tables);
    json!({
        "cleared_tables": cleared,
        "message": "扫描历史、进化管道记录、工作流聚类、记忆数据和使用记录已清空。Skills、Agents、会话、社区缓存和所有配置已保留。"
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
        "workflow_clusters",
        "community_skills",
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
        "message": "数据库已初始化。已保留管理员、系统配置和数据源配置，扫描数据、会话、Skills、Agents、社区缓存与历史记录已清空。"
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
