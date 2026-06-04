use rusqlite::{params_from_iter, Connection};
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
            "evolution_runs": crate::db::count_table(conn, "evolution_runs").unwrap_or(0),
            "evolution_steps": crate::db::count_table(conn, "evolution_steps").unwrap_or(0),
            "evolution_artifacts": crate::db::count_table(conn, "evolution_artifacts").unwrap_or(0),
            "community_skills": crate::db::count_table(conn, "community_skills").unwrap_or(0),
            "workflow_clusters": crate::db::count_table(conn, "workflow_clusters").unwrap_or(0),
            "workflow_recommendations": crate::db::count_table(conn, "workflow_recommendations").unwrap_or(0),
            "daily_reports": crate::db::count_table(conn, "daily_reports").unwrap_or(0),
            "skill_variants": crate::db::count_table(conn, "skill_variants").unwrap_or(0),
            "skill_iterations": crate::db::count_table(conn, "skill_iterations").unwrap_or(0),
            "team_skills_cache": crate::db::count_table(conn, "team_skills_cache").unwrap_or(0),
            "source_configs": crate::db::count_table(conn, "source_configs").unwrap_or(0),
            "admin_users": crate::db::count_table(conn, "admin_users").unwrap_or(0),
        },
    })
}

pub(crate) fn get_table_detail(
    conn: &Connection,
    table: &str,
    search: &str,
    page: u32,
    size: u32,
) -> anyhow::Result<Value> {
    if !validate_table(table) {
        return Ok(json!({ "error": "invalid table" }));
    }

    let columns: Vec<Value> = {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info(\"{}\")", table))?;
        let result: Vec<Value> = stmt
            .query_map([], |row| {
                Ok(json!({
                    "cid": row.get::<_, i64>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "type": row.get::<_, String>(2)?,
                }))
            })?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    let col_names: Vec<String> = columns
        .iter()
        .filter_map(|c| c["name"].as_str().map(String::from))
        .collect();

    let offset = (page.saturating_sub(1) * size) as i64;
    let size_i64 = size as i64;
    let base_from = format!("FROM \"{}\"", table);

    let (count_sql, data_sql, like_params) = if search.is_empty() {
        (
            format!("SELECT COUNT(*) {}", base_from),
            format!("SELECT rowid, * {} ORDER BY rowid DESC LIMIT ? OFFSET ?", base_from),
            Vec::new(),
        )
    } else {
        let like_val = format!("%{}%", search);
        let conditions: Vec<String> = col_names
            .iter()
            .map(|n| format!("\"{}\" LIKE ?", n))
            .collect();
        let where_clause = conditions.join(" OR ");
        let like_params: Vec<String> = col_names.iter().map(|_| like_val.clone()).collect();
        (
            format!("SELECT COUNT(*) {} WHERE {}", base_from, where_clause),
            format!(
                "SELECT rowid, * {} WHERE {} ORDER BY rowid DESC LIMIT ? OFFSET ?",
                base_from, where_clause
            ),
            like_params,
        )
    };

    let total: u64 = {
        let mut stmt = conn.prepare(&count_sql)?;
        if like_params.is_empty() {
            stmt.query_row([], |row| row.get::<_, i64>(0))
        } else {
            let refs: Vec<&dyn rusqlite::types::ToSql> =
                like_params.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
            stmt.query_row(params_from_iter(&refs), |row| row.get::<_, i64>(0))
        }? as u64
    };

    let mut all_params: Vec<Box<dyn rusqlite::types::ToSql>> = like_params
        .into_iter()
        .map(|s| Box::new(s) as Box<dyn rusqlite::types::ToSql>)
        .collect();
    all_params.push(Box::new(size_i64));
    all_params.push(Box::new(offset));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        all_params.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&data_sql)?;
    let rows: Vec<Value> = stmt
        .query_map(params_from_iter(&param_refs), |row| {
            let mut map = serde_json::Map::new();
            for (i, col) in col_names.iter().enumerate() {
                let cell = row
                    .get::<_, String>(i)
                    .map(|s| {
                        if s.len() > 500 {
                            Value::String(format!("{}...", &s[..500]))
                        } else {
                            Value::String(s)
                        }
                    })
                    .or_else(|_| {
                        row.get::<_, i64>(i)
                            .map(|n| json!(n))
                            .or_else(|_| row.get::<_, f64>(i).map(|n| json!(n)))
                    })
                    .unwrap_or(Value::Null);
                map.insert(col.clone(), cell);
            }
            Ok(Value::Object(map))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(json!({
        "table": table,
        "columns": columns,
        "rows": rows,
        "total": total,
        "page": page,
        "size": size,
    }))
}

const ALLOWED_TABLES: &[&str] = &[
    "skills", "agents", "sessions", "memories", "skill_usage",
    "scan_jobs", "evolution_jobs", "evolution_runs", "evolution_steps", "evolution_artifacts",
    "community_skills", "community_candidates",
    "workflow_clusters", "workflow_recommendations",
    "daily_reports", "skill_variants", "skill_iterations", "team_skills_cache",
    "source_configs", "admin_users",
];

fn validate_table(table: &str) -> bool {
    ALLOWED_TABLES.contains(&table)
}

/// Insert a new row into a table. `data` is a JSON object of column → value.
pub(crate) fn insert_row(conn: &Connection, table: &str, data: &Value) -> anyhow::Result<Value> {
    if !validate_table(table) {
        return Err(anyhow::anyhow!("不允许操作此表: {}", table));
    }
    let obj = data.as_object().ok_or_else(|| anyhow::anyhow!("数据格式错误"))?;
    let cols: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();

    let placeholders: Vec<String> = cols.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
    let sql = format!(
        "INSERT INTO \"{}\" ({}) VALUES ({})",
        table,
        cols.iter().map(|c| format!("\"{}\"", c)).collect::<Vec<_>>().join(", "),
        placeholders.join(", ")
    );

    let string_vals: Vec<String> = cols
        .iter()
        .map(|c| value_to_string(&obj[*c]))
        .collect();
    let refs: Vec<&dyn rusqlite::types::ToSql> = string_vals.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();

    conn.execute(&sql, params_from_iter(&refs))?;
    let new_id = conn.last_insert_rowid();
    Ok(json!({ "rowid": new_id, "message": "新增成功" }))
}

/// Update a row identified by rowid. `data` is a JSON object of column → new_value.
pub(crate) fn update_row(conn: &Connection, table: &str, rowid: i64, data: &Value) -> anyhow::Result<Value> {
    if !validate_table(table) {
        return Err(anyhow::anyhow!("不允许操作此表: {}", table));
    }
    let obj = data.as_object().ok_or_else(|| anyhow::anyhow!("数据格式错误"))?;
    let sets: Vec<String> = obj
        .keys()
        .enumerate()
        .map(|(i, k)| format!("\"{}\" = ?{}", k, i + 1))
        .collect();

    let sql = format!("UPDATE \"{}\" SET {} WHERE rowid = ?{}", table, sets.join(", "), obj.len() + 1);

    let mut string_vals: Vec<String> = obj.values().map(value_to_string).collect();
    string_vals.push(rowid.to_string());

    let refs: Vec<&dyn rusqlite::types::ToSql> = string_vals.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
    let affected = conn.execute(&sql, params_from_iter(&refs))?;
    Ok(json!({ "affected": affected, "message": "更新成功" }))
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        _ => v.to_string(),
    }
}

/// Delete a row identified by rowid.
pub(crate) fn delete_row(conn: &Connection, table: &str, rowid: i64) -> anyhow::Result<Value> {
    if !validate_table(table) {
        return Err(anyhow::anyhow!("不允许操作此表: {}", table));
    }
    let sql = format!("DELETE FROM \"{}\" WHERE rowid = ?1", table);
    let affected = conn.execute(&sql, [rowid])?;
    Ok(json!({ "affected": affected, "message": "删除成功" }))
}
