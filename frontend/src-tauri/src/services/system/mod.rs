use std::collections::HashMap;

// ---- Re-exports ----
pub(crate) use disk_cleanup::*;
pub(crate) use disk_usage::*;
pub(crate) use maintenance::*;
pub(crate) use monitor::*;

mod disk_cleanup;
mod disk_usage;
mod file_cleanup;
mod maintenance;
mod monitor;
mod utils;

// ---- Public structs ----

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
