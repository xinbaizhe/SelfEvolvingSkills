use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{json, Value};

use super::DiskUsageNode;

pub(crate) fn disk_node_json(node: DiskUsageNode) -> Value {
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

pub(crate) fn classify_disk_node(path: &str) -> &'static str {
    if is_log_like_path(path) {
        "log"
    } else {
        "other"
    }
}

pub(crate) fn is_log_like_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".log")
        || lower.ends_with(".etl")
        || lower.ends_with(".trace")
        || lower.ends_with(".dmp")
        || lower.contains("\\logs\\")
        || lower.contains("\\log\\")
}

pub(crate) fn is_log_dir_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with("\\log")
        || lower.ends_with("\\logs")
        || lower.contains("\\logs\\")
        || lower.contains("\\log\\")
}

pub(crate) fn unix_timestamp_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

pub(crate) fn path_to_db_string(path: &Path) -> String {
    normalize_db_path(&path.to_string_lossy())
}

pub(crate) fn normalize_db_path(path: &str) -> String {
    let replaced = path.replace('/', "\\");
    if replaced.len() > 3 {
        replaced.trim_end_matches('\\').to_string()
    } else {
        replaced
    }
}

pub(crate) fn parent_path_string(path: &Path) -> Option<String> {
    path.parent().map(path_to_db_string)
}

pub(crate) fn should_count_file(root: &Path, path: &Path) -> bool {
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

pub(crate) fn existing_reveal_target(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        return Some(path.to_path_buf());
    }
    path.parent()
        .filter(|parent| parent.exists())
        .map(Path::to_path_buf)
}

#[cfg(windows)]
pub(crate) fn reveal_path_in_file_manager(path: &Path) -> anyhow::Result<()> {
    let arg = if path.is_file() {
        format!("/select,{}", path.to_string_lossy())
    } else {
        path.to_string_lossy().to_string()
    };
    Command::new("explorer.exe").arg(arg).spawn()?;
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn reveal_path_in_file_manager(path: &Path) -> anyhow::Result<()> {
    if path.is_file() {
        Command::new("open").arg("-R").arg(path).spawn()?;
    } else {
        Command::new("open").arg(path).spawn()?;
    }
    Ok(())
}

#[cfg(all(not(windows), not(target_os = "macos")))]
pub(crate) fn reveal_path_in_file_manager(path: &Path) -> anyhow::Result<()> {
    let target = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    Command::new("xdg-open").arg(target).spawn()?;
    Ok(())
}

pub(crate) fn default_system_root() -> PathBuf {
    let system_drive = env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
    PathBuf::from(format!("{system_drive}\\"))
}

pub(crate) fn is_protected_delete_target(path: &Path) -> bool {
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
pub(crate) fn is_reparse_point(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
        .unwrap_or(false)
}

#[cfg(not(windows))]
pub(crate) fn is_reparse_point(_path: &Path) -> bool {
    false
}
