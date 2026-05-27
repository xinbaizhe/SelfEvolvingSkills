use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use super::file_cleanup::{
    clean_path_contents_best_effort, delete_file_best_effort, disk_usage_for_path,
    scan_full_path_size, DirectoryStats,
};
use super::utils::{
    default_system_root, disk_node_json, existing_reveal_target, is_log_dir_path,
    is_log_like_path, is_protected_delete_target, is_reparse_point, normalize_db_path,
    parent_path_string, path_to_db_string, reveal_path_in_file_manager, unix_timestamp_string,
};
use super::DiskUsageCache;

#[derive(Default)]
struct DiskScanProgress {
    visited: u64,
    size_bytes: u64,
    last_emit: u64,
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

pub(crate) fn disk_usage_cached_view(
    cache: &DiskUsageCache,
    path: &str,
    filter: &str,
) -> Value {
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
        return json!({
            "deleted": false,
            "path": path.to_string_lossy(),
            "message": "路径不存在",
        });
    }
    if is_protected_delete_target(&path) {
        return json!({
            "deleted": false,
            "path": path.to_string_lossy(),
            "message": "该路径属于系统保护位置，不能直接清理",
        });
    }

    let before = if path.is_file() {
        DirectoryStats {
            exists: true,
            size_bytes: fs::metadata(&path)
                .map(|metadata| metadata.len())
                .unwrap_or(0),
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
                format!(
                    "清理失败：{} 项因权限不足或正在使用无法删除",
                    result.failed_count
                )
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

fn scan_disk_usage_tree_with_progress(
    root: &Path,
    app: Option<&AppHandle>,
) -> DiskUsageCache {
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
        .filter(|path| {
            nodes_by_path
                .get(path)
                .map(|node| node.is_dir)
                .unwrap_or(false)
        })
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

fn collect_tree_node(
    path: &Path,
    parent_path: Option<String>,
    depth: usize,
    nodes_by_path: &mut HashMap<String, super::DiskUsageNode>,
    children_by_parent: &mut HashMap<String, Vec<String>>,
    progress: &mut DiskScanProgress,
    app: Option<&AppHandle>,
) -> DirectoryStats {
    let path_string = path_to_db_string(path);
    progress.visited = progress.visited.saturating_add(1);
    if progress.visited.saturating_sub(progress.last_emit) >= 500 {
        progress.last_emit = progress.visited;
        emit_disk_usage_progress(
            app,
            "running",
            progress.visited,
            progress.size_bytes,
            &path_string,
        );
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path_string.clone());
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) => {
            let node = super::DiskUsageNode {
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
                children_by_parent
                    .entry(parent)
                    .or_default()
                    .push(path_string);
            }
            return DirectoryStats {
                error: Some(err.to_string()),
                ..DirectoryStats::default()
            };
        }
    };

    if !metadata.is_dir() || is_reparse_point(path) {
        let size = if metadata.is_file() {
            metadata.len()
        } else {
            0
        };
        let node = super::DiskUsageNode {
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
            children_by_parent
                .entry(parent)
                .or_default()
                .push(path_string);
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

    let node = super::DiskUsageNode {
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
        children_by_parent
            .entry(parent)
            .or_default()
            .push(path_string);
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
