use serde_json::{json, Value};
use std::path::Path;

use crate::db;
use rusqlite::Connection;

pub(crate) fn get_system_monitor() -> Value {
    use sysinfo::{CpuRefreshKind, Disks, RefreshKind, System};

    let mut sys = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(sysinfo::MemoryRefreshKind::everything()),
    );
    std::thread::sleep(std::time::Duration::from_millis(300));
    sys.refresh_cpu_all();
    sys.refresh_memory();

    let disks = Disks::new_with_refreshed_list();

    let cpu_usage = sys.global_cpu_usage();
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_default();
    let uptime_secs = System::uptime();
    let total_memory_mb = sys.total_memory() / 1024 / 1024;
    let used_memory_mb = sys.used_memory() / 1024 / 1024;

    let disk_info: Vec<Value> = disks
        .iter()
        .map(|disk| {
            let total = disk.total_space() as f64;
            let available = disk.available_space() as f64;
            let usage = if total > 0.0 {
                ((1.0 - available / total) * 100.0).round()
            } else {
                0.0
            };
            json!({
                "mount_point": disk.mount_point().to_string_lossy(),
                "available_gb": format!("{:.1}", available / 1024.0 / 1024.0 / 1024.0),
                "total_gb": format!("{:.1}", total / 1024.0 / 1024.0 / 1024.0),
                "usage_pct": usage,
            })
        })
        .collect();

    json!({
        "cpu_usage_percent": format!("{:.1}", cpu_usage),
        "cpu_brand": cpu_brand,
        "uptime_seconds": uptime_secs,
        "memory": {
            "total_mb": total_memory_mb,
            "used_mb": used_memory_mb,
            "usage_percent": if total_memory_mb > 0 {
                format!("{:.1}", (used_memory_mb as f64 / total_memory_mb as f64) * 100.0)
            } else { "0.0".into() },
            "free_mb": total_memory_mb.saturating_sub(used_memory_mb),
        },
        "disks": disk_info,
    })
}

pub(crate) fn get_database_info(db_path: &Path, conn: &Connection) -> Value {
    let db_size = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);
    let wal_path = db_path.with_extension("db-wal");
    let wal_size = std::fs::metadata(&wal_path).map(|m| m.len()).unwrap_or(0);

    json!({
        "db_path": db_path.to_string_lossy().replace('\\', "/"),
        "db_size_bytes": db_size,
        "db_size_mb": format!("{:.2}", db_size as f64 / 1024.0 / 1024.0),
        "wal_size_bytes": wal_size,
        "wal_size_mb": format!("{:.2}", wal_size as f64 / 1024.0 / 1024.0),
        "table_counts": {
            "skills": db::count_table(conn, "skills").unwrap_or(0),
            "agents": db::count_table(conn, "agents").unwrap_or(0),
            "sessions": db::count_table(conn, "sessions").unwrap_or(0),
            "memories": db::count_table(conn, "memories").unwrap_or(0),
            "skill_usage": db::count_table(conn, "skill_usage").unwrap_or(0),
            "scan_jobs": db::count_table(conn, "scan_jobs").unwrap_or(0),
            "evolution_jobs": db::count_table(conn, "evolution_jobs").unwrap_or(0),
            "community_skills": db::count_table(conn, "community_skills").unwrap_or(0),
            "workflow_clusters": db::count_table(conn, "workflow_clusters").unwrap_or(0),
            "source_configs": db::count_table(conn, "source_configs").unwrap_or(0),
            "admin_users": db::count_table(conn, "admin_users").unwrap_or(0),
        },
    })
}

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
