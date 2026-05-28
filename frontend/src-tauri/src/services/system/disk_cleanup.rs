use std::{collections::HashSet, env, path::PathBuf};

use serde_json::{json, Value};

use super::file_cleanup::{clean_directory_contents, clean_download_installers, scan_path_size};

struct CleanupCandidate {
    id: &'static str,
    title: &'static str,
    description: &'static str,
    category: &'static str,
    risk: &'static str,
    path: PathBuf,
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
            path: PathBuf::from(format!(
                r"{system_drive}\Windows\SoftwareDistribution\Download"
            )),
        },
        CleanupCandidate {
            id: "delivery_optimization",
            title: "传递优化缓存",
            description: "Windows 更新和商店应用分发缓存，空间占用较大时可清理。",
            category: "系统缓存",
            risk: "review",
            path: PathBuf::from(format!(
                r"{system_drive}\ProgramData\Microsoft\Windows\DeliveryOptimization\Cache"
            )),
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
            path: path
                .join("Microsoft")
                .join("Edge")
                .join("User Data")
                .join("Default")
                .join("Cache"),
        });
        items.push(CleanupCandidate {
            id: "chrome_cache",
            title: "Chrome 缓存",
            description: "Chrome 默认用户缓存文件，清理后网页会按需重新缓存。",
            category: "应用缓存",
            risk: "safe",
            path: path
                .join("Google")
                .join("Chrome")
                .join("User Data")
                .join("Default")
                .join("Cache"),
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
