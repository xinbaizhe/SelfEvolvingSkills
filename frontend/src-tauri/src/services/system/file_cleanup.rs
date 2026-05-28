use std::{fs, path::Path};
use walkdir::WalkDir;

use super::utils::{is_reparse_point, should_count_file};

#[derive(Default)]
pub(crate) struct DirectoryStats {
    pub(crate) exists: bool,
    pub(crate) size_bytes: u64,
    pub(crate) file_count: u64,
    pub(crate) scan_limited: bool,
    pub(crate) error: Option<String>,
}

pub(crate) struct DiskUsageInfo {
    pub(crate) total_bytes: u64,
    pub(crate) available_bytes: u64,
    pub(crate) used_bytes: u64,
}

#[derive(Default)]
pub(crate) struct DeleteSummary {
    pub(crate) deleted_count: u64,
    pub(crate) failed_count: u64,
    pub(crate) size_bytes: u64,
    pub(crate) failed_paths: Vec<String>,
}

pub(crate) fn scan_path_size(path: &Path) -> DirectoryStats {
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

pub(crate) fn scan_full_path_size(path: &Path) -> DirectoryStats {
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

pub(crate) fn disk_usage_for_path(path: &Path) -> Option<DiskUsageInfo> {
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

pub(crate) fn clean_directory_contents(path: &Path) -> anyhow::Result<()> {
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

pub(crate) fn clean_download_installers(path: &Path) -> anyhow::Result<()> {
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

pub(crate) fn delete_file_best_effort(path: &Path) -> DeleteSummary {
    let mut summary = DeleteSummary::default();
    let size = fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    match fs::remove_file(path) {
        Ok(()) => {
            summary.deleted_count = 1;
            summary.size_bytes = size;
        }
        Err(_) => record_delete_failure(&mut summary, path),
    }
    summary
}

pub(crate) fn clean_path_contents_best_effort(path: &Path) -> DeleteSummary {
    let mut summary = DeleteSummary::default();
    clean_directory_children_best_effort(path, &mut summary);
    summary
}

pub(crate) fn clean_directory_children_best_effort(path: &Path, summary: &mut DeleteSummary) {
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
        summary
            .failed_paths
            .push(path.to_string_lossy().to_string());
    }
}
