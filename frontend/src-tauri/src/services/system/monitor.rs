use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::Path;

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
            "skills": crate::db::count_table(conn, "skills").unwrap_or(0),
            "agents": crate::db::count_table(conn, "agents").unwrap_or(0),
            "sessions": crate::db::count_table(conn, "sessions").unwrap_or(0),
            "memories": crate::db::count_table(conn, "memories").unwrap_or(0),
            "skill_usage": crate::db::count_table(conn, "skill_usage").unwrap_or(0),
            "scan_jobs": crate::db::count_table(conn, "scan_jobs").unwrap_or(0),
            "evolution_jobs": crate::db::count_table(conn, "evolution_jobs").unwrap_or(0),
            "community_skills": crate::db::count_table(conn, "community_skills").unwrap_or(0),
            "workflow_clusters": crate::db::count_table(conn, "workflow_clusters").unwrap_or(0),
            "source_configs": crate::db::count_table(conn, "source_configs").unwrap_or(0),
            "admin_users": crate::db::count_table(conn, "admin_users").unwrap_or(0),
        },
    })
}
