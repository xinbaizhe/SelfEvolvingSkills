use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use walkdir::WalkDir;

use crate::db;
use rusqlite::Connection;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Debug)]
pub(crate) struct DiskUsageNode {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) parent_path: Option<String>,
    pub(crate) is_dir: bool,
    pub(crate) size_bytes: u64,
    pub(crate) file_count: u64,
    pub(crate) depth: usize,
    pub(crate) scan_limited: bool,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct DiskUsageCache {
    pub(crate) root_path: String,
    pub(crate) scanned_at: String,
    pub(crate) total_size_bytes: u64,
    pub(crate) file_count: u64,
    pub(crate) scan_limited: bool,
    pub(crate) disk_total_bytes: Option<u64>,
    pub(crate) disk_available_bytes: Option<u64>,
    pub(crate) disk_used_bytes: Option<u64>,
    pub(crate) nodes_by_path: HashMap<String, DiskUsageNode>,
    pub(crate) children_by_parent: HashMap<String, Vec<String>>,
    pub(crate) log_paths: Vec<String>,
}

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

pub(crate) fn scan_system_disk_cleanup() -> Value {
    let candidates = cleanup_candidates();
    let items: Vec<Value> = candidates
        .iter()
        .map(|candidate| {
            let stats = scan_path_size(&candidate.path);
            json!({
                "id": candidate.id,
                "title": candidate.title,
                "description": candidate.description,
                "path": candidate.path.to_string_lossy(),
                "category": candidate.category,
                "risk": candidate.risk,
                "size_bytes": stats.size_bytes,
                "file_count": stats.file_count,
                "exists": stats.exists,
                "scan_limited": stats.scan_limited,
                "error": stats.error,
            })
        })
        .collect();
    let total_size_bytes = items
        .iter()
        .filter_map(|item| item.get("size_bytes").and_then(Value::as_u64))
        .sum::<u64>();

    json!({
        "items": items,
        "total_size_bytes": total_size_bytes,
        "message": "系统盘清理候选项扫描完成。"
    })
}

pub(crate) fn clean_system_disk_items(body: Option<Value>) -> Value {
    let requested_ids = body
        .as_ref()
        .and_then(|value| value.get("ids"))
        .and_then(Value::as_array)
        .map(|ids| {
            ids.iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();

    let candidates = cleanup_candidates();
    let mut cleaned = Vec::new();
    let mut skipped = Vec::new();
    let mut errors = Vec::new();
    let mut total_size_bytes = 0_u64;

    for candidate in candidates {
        if !requested_ids.contains(candidate.id) {
            continue;
        }
        let before = scan_path_size(&candidate.path);
        if !before.exists {
            skipped.push(json!({ "id": candidate.id, "reason": "路径不存在" }));
            continue;
        }

        let clean_result = if candidate.id == "downloads_installers" {
            clean_download_installers(&candidate.path)
        } else {
            clean_directory_contents(&candidate.path)
        };

        match clean_result {
            Ok(()) => {
                total_size_bytes = total_size_bytes.saturating_add(before.size_bytes);
                cleaned.push(json!({
                    "id": candidate.id,
                    "title": candidate.title,
                    "size_bytes": before.size_bytes,
                    "file_count": before.file_count,
                }));
            }
            Err(err) => errors.push(json!({
                "id": candidate.id,
                "title": candidate.title,
                "error": err.to_string(),
            })),
        }
    }

    json!({
        "cleaned": cleaned,
        "skipped": skipped,
        "errors": errors,
        "total_size_bytes": total_size_bytes,
        "message": "清理操作已完成。部分系统目录可能需要管理员权限。"
    })
}

pub(crate) fn build_disk_usage_cache_with_progress(
    app: AppHandle,
    body: Option<Value>,
) -> (DiskUsageCache, Value) {
    let requested_path = body
        .as_ref()
        .and_then(|value| value.get("path"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);
    let root = requested_path.unwrap_or_else(default_system_root);
    let cache = scan_disk_usage_tree_with_progress(&root, Some(&app));
    let value = disk_usage_cached_view(&cache, &cache.root_path, "all");
    emit_disk_usage_progress(
        Some(&app),
        "completed",
        cache.nodes_by_path.len() as u64,
        cache.total_size_bytes,
        &cache.root_path,
    );
    (cache, value)
}

pub(crate) fn disk_usage_cache_status(cache: Option<&DiskUsageCache>) -> Value {
    match cache {
        Some(cache) => json!({
            "available": true,
            "root_path": cache.root_path,
            "scanned_at": cache.scanned_at,
            "total_size_bytes": cache.total_size_bytes,
            "file_count": cache.file_count,
            "node_count": cache.nodes_by_path.len(),
            "scan_limited": cache.scan_limited,
            "disk_total_bytes": cache.disk_total_bytes,
            "disk_available_bytes": cache.disk_available_bytes,
            "disk_used_bytes": cache.disk_used_bytes,
        }),
        None => json!({ "available": false }),
    }
}

pub(crate) fn disk_usage_cached_view(cache: &DiskUsageCache, path: &str, filter: &str) -> Value {
    disk_usage_cached_view_paged(cache, path, filter, 1, 200)
}

pub(crate) fn disk_usage_cached_view_paged(
    cache: &DiskUsageCache,
    path: &str,
    filter: &str,
    page: i64,
    size: i64,
) -> Value {
    let current = normalize_db_path(path);
    let current_node = cache.nodes_by_path.get(&current);
    let parent = current_node
        .and_then(|node| node.parent_path.clone())
        .or_else(|| parent_path_string(Path::new(&current)));
    let page = page.max(1);
    let size = size.clamp(50, 500);
    let start = ((page - 1) * size) as usize;

    let (total_entries, entries) = if filter == "logs" {
        let total_entries = cache.log_paths.len();
        let entries = cache
            .log_paths
            .iter()
            .skip(start)
            .take(size as usize)
            .filter_map(|path| cache.nodes_by_path.get(path))
            .cloned()
            .collect::<Vec<_>>();
        (total_entries, entries)
    } else {
        let mut entries = cache
            .children_by_parent
            .get(&current)
            .into_iter()
            .flatten()
            .filter_map(|path| cache.nodes_by_path.get(path))
            .cloned()
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes).then_with(|| a.name.cmp(&b.name)));
        let total_entries = entries.len();
        let entries = entries
            .into_iter()
            .skip(start)
            .take(size as usize)
            .collect::<Vec<_>>();
        (total_entries, entries)
    };

    let total_size_bytes = current_node
        .map(|node| node.size_bytes)
        .unwrap_or(cache.total_size_bytes);
    let file_count = current_node
        .map(|node| node.file_count)
        .unwrap_or(cache.file_count);
    let scan_limited = current_node
        .map(|node| node.scan_limited)
        .unwrap_or(cache.scan_limited);

    json!({
        "path": current,
        "parent": parent,
        "filter": filter,
        "entries": entries.into_iter().map(disk_node_json).collect::<Vec<_>>(),
        "entry_total": total_entries,
        "page": page,
        "size": size,
        "total_size_bytes": total_size_bytes,
        "file_count": file_count,
        "scan_limited": scan_limited,
        "disk_total_bytes": cache.disk_total_bytes,
        "disk_available_bytes": cache.disk_available_bytes,
        "disk_used_bytes": cache.disk_used_bytes,
        "scanned_at": cache.scanned_at,
        "node_count": cache.nodes_by_path.len(),
        "error": Value::Null,
    })
}

pub(crate) fn delete_disk_usage_path(body: Option<Value>) -> Value {
    let Some(path) = body
        .as_ref()
        .and_then(|value| value.get("path"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
    else {
        return json!({ "deleted": false, "message": "缺少路径" });
    };

    if !path.exists() {
        return json!({ "deleted": false, "path": path.to_string_lossy(), "message": "路径不存在" });
    }
    if is_protected_delete_target(&path) {
        return json!({ "deleted": false, "path": path.to_string_lossy(), "message": "该路径属于系统保护位置，不能直接清理" });
    }

    let before = if path.is_file() {
        DirectoryStats {
            exists: true,
            size_bytes: fs::metadata(&path).map(|metadata| metadata.len()).unwrap_or(0),
            file_count: 1,
            scan_limited: false,
            error: None,
        }
    } else {
        scan_full_path_size(&path)
    };

    let result = if path.is_dir() {
        clean_path_contents_best_effort(&path)
    } else {
        delete_file_best_effort(&path)
    };

    if result.deleted_count > 0 && result.failed_count == 0 {
        json!({
            "deleted": true,
            "partial": false,
            "path": path.to_string_lossy(),
            "size_bytes": result.size_bytes,
            "file_count": result.deleted_count,
            "deleted_count": result.deleted_count,
            "failed_count": result.failed_count,
            "failed_paths": result.failed_paths,
            "message": "清理完成",
        })
    } else if result.deleted_count > 0 {
        json!({
            "deleted": true,
            "partial": true,
            "path": path.to_string_lossy(),
            "size_bytes": result.size_bytes,
            "file_count": result.deleted_count,
            "deleted_count": result.deleted_count,
            "failed_count": result.failed_count,
            "failed_paths": result.failed_paths,
            "message": format!(
                "部分清理完成，已清理 {} 项，{} 项因权限不足或正在使用被跳过",
                result.deleted_count, result.failed_count
            ),
        })
    } else {
        json!({
            "deleted": false,
            "path": path.to_string_lossy(),
            "size_bytes": before.size_bytes,
            "file_count": before.file_count,
            "deleted_count": result.deleted_count,
            "failed_count": result.failed_count,
            "failed_paths": result.failed_paths,
            "message": if result.failed_count > 0 {
                format!("清理失败：{} 项因权限不足或正在使用无法删除", result.failed_count)
            } else {
                "清理失败：没有可删除的内容".to_string()
            },
        })
    }
}

pub(crate) fn reveal_disk_usage_path(body: Option<Value>) -> Value {
    let Some(path) = body
        .as_ref()
        .and_then(|value| value.get("path"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
    else {
        return json!({ "opened": false, "message": "缺少路径" });
    };

    let target = existing_reveal_target(&path);
    let Some(target) = target else {
        return json!({
            "opened": false,
            "path": path.to_string_lossy(),
            "message": "路径不存在，无法打开所在目录",
        });
    };

    match reveal_path_in_file_manager(&target) {
        Ok(()) => json!({
            "opened": true,
            "path": target.to_string_lossy(),
            "message": "已打开所在目录",
        }),
        Err(err) => json!({
            "opened": false,
            "path": target.to_string_lossy(),
            "message": format!("打开所在目录失败：{err}"),
        }),
    }
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

struct CleanupCandidate {
    id: &'static str,
    title: &'static str,
    description: &'static str,
    category: &'static str,
    risk: &'static str,
    path: PathBuf,
}

#[derive(Default)]
struct DirectoryStats {
    exists: bool,
    size_bytes: u64,
    file_count: u64,
    scan_limited: bool,
    error: Option<String>,
}

struct DiskUsageInfo {
    total_bytes: u64,
    available_bytes: u64,
    used_bytes: u64,
}

#[derive(Default)]
struct DeleteSummary {
    deleted_count: u64,
    failed_count: u64,
    size_bytes: u64,
    failed_paths: Vec<String>,
}

fn cleanup_candidates() -> Vec<CleanupCandidate> {
    let system_drive = env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
    let user_profile = env::var("USERPROFILE").ok().map(PathBuf::from);
    let local_app_data = env::var("LOCALAPPDATA").ok().map(PathBuf::from);
    let temp_dir = env::temp_dir();

    let mut items = vec![
        CleanupCandidate {
            id: "user_temp",
            title: "当前用户临时文件",
            description: "应用安装包、解压残留和临时运行文件，通常可以清理。",
            category: "临时文件",
            risk: "safe",
            path: temp_dir,
        },
        CleanupCandidate {
            id: "windows_temp",
            title: "Windows 临时文件",
            description: "系统临时目录，部分正在使用的文件会自动跳过或清理失败。",
            category: "系统缓存",
            risk: "review",
            path: PathBuf::from(format!(r"{system_drive}\Windows\Temp")),
        },
        CleanupCandidate {
            id: "windows_update_download",
            title: "Windows 更新下载缓存",
            description: "Windows Update 已下载的安装缓存，清理后必要时会重新下载。",
            category: "系统缓存",
            risk: "review",
            path: PathBuf::from(format!(r"{system_drive}\Windows\SoftwareDistribution\Download")),
        },
        CleanupCandidate {
            id: "delivery_optimization",
            title: "传递优化缓存",
            description: "Windows 更新和商店应用分发缓存，空间占用较大时可清理。",
            category: "系统缓存",
            risk: "review",
            path: PathBuf::from(format!(r"{system_drive}\ProgramData\Microsoft\Windows\DeliveryOptimization\Cache")),
        },
        CleanupCandidate {
            id: "recycle_bin",
            title: "回收站",
            description: "系统盘回收站内容。清理后无法从回收站还原。",
            category: "回收站",
            risk: "danger",
            path: PathBuf::from(format!(r"{system_drive}\$Recycle.Bin")),
        },
    ];

    if let Some(path) = local_app_data {
        items.push(CleanupCandidate {
            id: "browser_cache",
            title: "浏览器缓存",
            description: "Edge、Chrome 等浏览器缓存文件，清理后网页会按需重新缓存。",
            category: "应用缓存",
            risk: "safe",
            path: path.join("Microsoft").join("Edge").join("User Data").join("Default").join("Cache"),
        });
        items.push(CleanupCandidate {
            id: "chrome_cache",
            title: "Chrome 缓存",
            description: "Chrome 默认用户缓存文件，清理后网页会按需重新缓存。",
            category: "应用缓存",
            risk: "safe",
            path: path.join("Google").join("Chrome").join("User Data").join("Default").join("Cache"),
        });
    }

    if let Some(path) = user_profile {
        items.push(CleanupCandidate {
            id: "downloads_installers",
            title: "下载目录安装包",
            description: "仅扫描下载目录根部的安装包、压缩包和镜像文件；清理前建议人工确认。",
            category: "用户文件",
            risk: "danger",
            path: path.join("Downloads"),
        });
    }

    items
}

fn scan_path_size(path: &Path) -> DirectoryStats {
    if !path.exists() {
        return DirectoryStats::default();
    }

    let mut stats = DirectoryStats {
        exists: true,
        ..DirectoryStats::default()
    };
    let mut visited = 0_u64;
    const MAX_VISITED: u64 = 120_000;

    let walker = WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !is_reparse_point(entry.path()));

    for entry in walker {
        if visited >= MAX_VISITED {
            stats.scan_limited = true;
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                if stats.error.is_none() {
                    stats.error = Some(err.to_string());
                }
                continue;
            }
        };
        if entry.depth() == 0 {
            continue;
        }
        visited += 1;
        if entry.file_type().is_file() && should_count_file(path, entry.path()) {
            stats.file_count += 1;
            stats.size_bytes = stats
                .size_bytes
                .saturating_add(entry.metadata().map(|m| m.len()).unwrap_or(0));
        }
    }

    stats
}

fn scan_full_path_size(path: &Path) -> DirectoryStats {
    if !path.exists() {
        return DirectoryStats::default();
    }

    let mut stats = DirectoryStats {
        exists: true,
        ..DirectoryStats::default()
    };
    let mut visited = 0_u64;
    const MAX_VISITED: u64 = 1_000_000;

    let walker = WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !is_reparse_point(entry.path()));

    for entry in walker {
        if visited >= MAX_VISITED {
            stats.scan_limited = true;
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                if stats.error.is_none() {
                    stats.error = Some(err.to_string());
                }
                continue;
            }
        };
        if entry.depth() == 0 {
            continue;
        }
        visited += 1;
        if entry.file_type().is_file() {
            stats.file_count += 1;
            stats.size_bytes = stats
                .size_bytes
                .saturating_add(entry.metadata().map(|m| m.len()).unwrap_or(0));
        }
    }

    stats
}

fn disk_usage_for_path(path: &Path) -> Option<DiskUsageInfo> {
    use sysinfo::Disks;
    let disks = Disks::new_with_refreshed_list();
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let disk = disks
        .iter()
        .filter(|disk| canonical.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())?;
    let total_bytes = disk.total_space();
    let available_bytes = disk.available_space();
    Some(DiskUsageInfo {
        total_bytes,
        available_bytes,
        used_bytes: total_bytes.saturating_sub(available_bytes),
    })
}

fn scan_disk_usage_tree_with_progress(root: &Path, app: Option<&AppHandle>) -> DiskUsageCache {
    let root_string = path_to_db_string(root);
    let disk_usage = disk_usage_for_path(root);
    let mut nodes_by_path = HashMap::new();
    let mut children_by_parent = HashMap::<String, Vec<String>>::new();
    let mut root_stats = DirectoryStats::default();
    let mut progress = DiskScanProgress::default();
    emit_disk_usage_progress(app, "running", 0, 0, &root_string);

    if root.exists() {
        root_stats = collect_tree_node(
            root,
            None,
            0,
            &mut nodes_by_path,
            &mut children_by_parent,
            &mut progress,
            app,
        );
    }
    let mut log_path_set = HashSet::new();
    for node in nodes_by_path.values() {
        if node.is_dir || !is_log_like_path(&node.path) {
            continue;
        }
        if let Some(parent) = node.parent_path.as_ref() {
            log_path_set.insert(parent.clone());
        }
    }
    let mut log_paths = log_path_set
        .into_iter()
        .filter(|path| nodes_by_path.get(path).map(|node| node.is_dir).unwrap_or(false))
        .collect::<Vec<_>>();
    log_paths.sort_by(|left, right| {
        let left_node = nodes_by_path.get(left);
        let right_node = nodes_by_path.get(right);
        right_node
            .map(|node| node.size_bytes)
            .unwrap_or(0)
            .cmp(&left_node.map(|node| node.size_bytes).unwrap_or(0))
            .then_with(|| left.cmp(right))
    });
    log_paths.dedup();
    if log_paths.is_empty() {
        log_paths = nodes_by_path
        .values()
            .filter(|node| node.is_dir && is_log_dir_path(&node.path))
        .map(|node| node.path.clone())
        .collect::<Vec<_>>();
        log_paths.sort_by(|left, right| {
            let left_node = nodes_by_path.get(left);
            let right_node = nodes_by_path.get(right);
            right_node
                .map(|node| node.size_bytes)
                .unwrap_or(0)
                .cmp(&left_node.map(|node| node.size_bytes).unwrap_or(0))
                .then_with(|| left.cmp(right))
        });
    }

    DiskUsageCache {
        root_path: root_string,
        scanned_at: unix_timestamp_string(),
        total_size_bytes: root_stats.size_bytes,
        file_count: root_stats.file_count,
        scan_limited: root_stats.scan_limited,
        disk_total_bytes: disk_usage.as_ref().map(|usage| usage.total_bytes),
        disk_available_bytes: disk_usage.as_ref().map(|usage| usage.available_bytes),
        disk_used_bytes: disk_usage.as_ref().map(|usage| usage.used_bytes),
        nodes_by_path,
        children_by_parent,
        log_paths,
    }
}

#[derive(Default)]
struct DiskScanProgress {
    visited: u64,
    size_bytes: u64,
    last_emit: u64,
}

fn collect_tree_node(
    path: &Path,
    parent_path: Option<String>,
    depth: usize,
    nodes_by_path: &mut HashMap<String, DiskUsageNode>,
    children_by_parent: &mut HashMap<String, Vec<String>>,
    progress: &mut DiskScanProgress,
    app: Option<&AppHandle>,
) -> DirectoryStats {
    let path_string = path_to_db_string(path);
    progress.visited = progress.visited.saturating_add(1);
    if progress.visited.saturating_sub(progress.last_emit) >= 500 {
        progress.last_emit = progress.visited;
        emit_disk_usage_progress(app, "running", progress.visited, progress.size_bytes, &path_string);
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path_string.clone());
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) => {
            let node = DiskUsageNode {
                name,
                path: path_string.clone(),
                parent_path: parent_path.clone(),
                is_dir: false,
                size_bytes: 0,
                file_count: 0,
                depth,
                scan_limited: false,
                error: Some(err.to_string()),
            };
            progress.size_bytes = progress.size_bytes.saturating_add(node.size_bytes);
            nodes_by_path.insert(path_string.clone(), node);
            if let Some(parent) = parent_path {
                children_by_parent.entry(parent).or_default().push(path_string);
            }
            return DirectoryStats {
                error: Some(err.to_string()),
                ..DirectoryStats::default()
            };
        }
    };

    if !metadata.is_dir() || is_reparse_point(path) {
        let size = if metadata.is_file() { metadata.len() } else { 0 };
        let node = DiskUsageNode {
            name,
            path: path_string.clone(),
            parent_path: parent_path.clone(),
            is_dir: false,
            size_bytes: size,
            file_count: if metadata.is_file() { 1 } else { 0 },
            depth,
            scan_limited: false,
            error: None,
        };
        progress.size_bytes = progress.size_bytes.saturating_add(node.size_bytes);
        nodes_by_path.insert(path_string.clone(), node);
        if let Some(parent) = parent_path {
            children_by_parent.entry(parent).or_default().push(path_string);
        }
        return DirectoryStats {
            exists: true,
            size_bytes: size,
            file_count: if metadata.is_file() { 1 } else { 0 },
            scan_limited: false,
            error: None,
        };
    }

    let mut stats = DirectoryStats {
        exists: true,
        ..DirectoryStats::default()
    };
    match fs::read_dir(path) {
        Ok(read_dir) => {
            for entry in read_dir.flatten() {
                let child_stats = collect_tree_node(
                    &entry.path(),
                    Some(path_string.clone()),
                    depth + 1,
                    nodes_by_path,
                    children_by_parent,
                    progress,
                    app,
                );
                stats.size_bytes = stats.size_bytes.saturating_add(child_stats.size_bytes);
                stats.file_count = stats.file_count.saturating_add(child_stats.file_count);
                stats.scan_limited = stats.scan_limited || child_stats.scan_limited;
                if stats.error.is_none() {
                    stats.error = child_stats.error;
                }
            }
        }
        Err(err) => stats.error = Some(err.to_string()),
    }

    let node = DiskUsageNode {
        name,
        path: path_string.clone(),
        parent_path: parent_path.clone(),
        is_dir: true,
        size_bytes: stats.size_bytes,
        file_count: stats.file_count,
        depth,
        scan_limited: stats.scan_limited,
        error: stats.error.clone(),
    };
    nodes_by_path.insert(path_string.clone(), node);
    if let Some(parent) = parent_path {
        children_by_parent.entry(parent).or_default().push(path_string);
    }
    stats
}

fn emit_disk_usage_progress(
    app: Option<&AppHandle>,
    status: &str,
    visited: u64,
    size_bytes: u64,
    current_path: &str,
) {
    if let Some(app) = app {
        let _ = app.emit(
            "disk-usage-progress",
            json!({
                "status": status,
                "visited": visited,
                "size_bytes": size_bytes,
                "current_path": current_path,
            }),
        );
    }
}

fn disk_node_json(node: DiskUsageNode) -> Value {
    json!({
        "name": node.name,
        "path": node.path,
        "parent_path": node.parent_path,
        "is_dir": node.is_dir,
        "size_bytes": node.size_bytes,
        "file_count": node.file_count,
        "depth": node.depth,
        "scan_limited": node.scan_limited,
        "error": node.error,
        "kind": classify_disk_node(&node.path),
    })
}

fn classify_disk_node(path: &str) -> &'static str {
    if is_log_like_path(path) {
        "log"
    } else {
        "other"
    }
}

fn is_log_like_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".log")
        || lower.ends_with(".etl")
        || lower.ends_with(".trace")
        || lower.ends_with(".dmp")
        || lower.contains("\\logs\\")
        || lower.contains("\\log\\")
}

fn is_log_dir_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with("\\log")
        || lower.ends_with("\\logs")
        || lower.contains("\\logs\\")
        || lower.contains("\\log\\")
}

fn unix_timestamp_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn path_to_db_string(path: &Path) -> String {
    normalize_db_path(&path.to_string_lossy())
}

fn normalize_db_path(path: &str) -> String {
    let replaced = path.replace('/', "\\");
    if replaced.len() > 3 {
        replaced.trim_end_matches('\\').to_string()
    } else {
        replaced
    }
}

fn parent_path_string(path: &Path) -> Option<String> {
    path.parent().map(path_to_db_string)
}

fn should_count_file(root: &Path, path: &Path) -> bool {
    if root.ends_with("Downloads") {
        let Some(parent) = path.parent() else {
            return false;
        };
        if parent != root {
            return false;
        }
        return path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| {
                matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "exe" | "msi" | "msix" | "zip" | "7z" | "rar" | "iso" | "dmg" | "pkg"
                )
            })
            .unwrap_or(false);
    }
    true
}

fn clean_directory_contents(path: &Path) -> anyhow::Result<()> {
    if !path.exists() || !path.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        let file_type = entry.file_type()?;
        let result = if file_type.is_dir() {
            fs::remove_dir_all(&entry_path)
        } else {
            fs::remove_file(&entry_path)
        };
        if let Err(err) = result {
            log::warn!("failed to clean {}: {}", entry_path.display(), err);
        }
    }
    Ok(())
}

fn clean_download_installers(path: &Path) -> anyhow::Result<()> {
    if !path.exists() || !path.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry.file_type()?.is_file() && should_count_file(path, &entry_path) {
            if let Err(err) = fs::remove_file(&entry_path) {
                log::warn!("failed to clean {}: {}", entry_path.display(), err);
            }
        }
    }
    Ok(())
}

fn delete_file_best_effort(path: &Path) -> DeleteSummary {
    let mut summary = DeleteSummary::default();
    let size = fs::metadata(path).map(|metadata| metadata.len()).unwrap_or(0);
    match fs::remove_file(path) {
        Ok(()) => {
            summary.deleted_count = 1;
            summary.size_bytes = size;
        }
        Err(_) => record_delete_failure(&mut summary, path),
    }
    summary
}

fn clean_path_contents_best_effort(path: &Path) -> DeleteSummary {
    let mut summary = DeleteSummary::default();
    clean_directory_children_best_effort(path, &mut summary);
    summary
}

fn clean_directory_children_best_effort(path: &Path, summary: &mut DeleteSummary) {
    let read_dir = match fs::read_dir(path) {
        Ok(read_dir) => read_dir,
        Err(_) => {
            record_delete_failure(summary, path);
            return;
        }
    };

    for entry in read_dir {
        let Ok(entry) = entry else {
            summary.failed_count = summary.failed_count.saturating_add(1);
            continue;
        };
        let entry_path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            record_delete_failure(summary, &entry_path);
            continue;
        };

        if file_type.is_dir() {
            clean_directory_children_best_effort(&entry_path, summary);
            match fs::remove_dir(&entry_path) {
                Ok(()) => summary.deleted_count = summary.deleted_count.saturating_add(1),
                Err(_) => {
                    if directory_is_empty(&entry_path) {
                        record_delete_failure(summary, &entry_path);
                    }
                }
            }
        } else {
            let size = entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
            match fs::remove_file(&entry_path) {
                Ok(()) => {
                    summary.deleted_count = summary.deleted_count.saturating_add(1);
                    summary.size_bytes = summary.size_bytes.saturating_add(size);
                }
                Err(_) => record_delete_failure(summary, &entry_path),
            }
        }
    }
}

fn directory_is_empty(path: &Path) -> bool {
    fs::read_dir(path)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
}

fn record_delete_failure(summary: &mut DeleteSummary, path: &Path) {
    summary.failed_count = summary.failed_count.saturating_add(1);
    if summary.failed_paths.len() < 5 {
        summary.failed_paths.push(path.to_string_lossy().to_string());
    }
}

fn existing_reveal_target(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        return Some(path.to_path_buf());
    }
    path.parent()
        .filter(|parent| parent.exists())
        .map(Path::to_path_buf)
}

#[cfg(windows)]
fn reveal_path_in_file_manager(path: &Path) -> anyhow::Result<()> {
    let arg = if path.is_file() {
        format!("/select,{}", path.to_string_lossy())
    } else {
        path.to_string_lossy().to_string()
    };
    Command::new("explorer.exe").arg(arg).spawn()?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn reveal_path_in_file_manager(path: &Path) -> anyhow::Result<()> {
    if path.is_file() {
        Command::new("open").arg("-R").arg(path).spawn()?;
    } else {
        Command::new("open").arg(path).spawn()?;
    }
    Ok(())
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn reveal_path_in_file_manager(path: &Path) -> anyhow::Result<()> {
    let target = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    Command::new("xdg-open").arg(target).spawn()?;
    Ok(())
}

fn default_system_root() -> PathBuf {
    let system_drive = env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
    PathBuf::from(format!("{system_drive}\\"))
}

fn is_protected_delete_target(path: &Path) -> bool {
    let normalized = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .to_ascii_lowercase();
    let system_drive = env::var("SystemDrive")
        .unwrap_or_else(|_| "C:".to_string())
        .to_ascii_lowercase();
    let protected = [
        system_drive.clone(),
        format!(r"{system_drive}\windows"),
        format!(r"{system_drive}\program files"),
        format!(r"{system_drive}\program files (x86)"),
        format!(r"{system_drive}\programdata"),
        format!(r"{system_drive}\users"),
    ];
    protected.contains(&normalized)
}

#[cfg(windows)]
fn is_reparse_point(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn is_reparse_point(_path: &Path) -> bool {
    false
}
