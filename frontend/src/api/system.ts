import { api } from './tauri'
import type { ApiResponse } from './skills'

export interface SystemMonitor {
  cpu_usage_percent: string
  cpu_brand: string
  uptime_seconds: number
  memory: {
    total_mb: number
    used_mb: number
    free_mb: number
    usage_percent: string
  }
  disks: DiskInfo[]
}

export interface DiskInfo {
  mount_point: string
  available_gb: string
  total_gb: string
  usage_pct: number
}

export interface DatabaseInfo {
  db_path: string
  db_size_bytes: number
  db_size_mb: string
  wal_size_bytes: number
  wal_size_mb: string
  table_counts: Record<string, number>
}

export interface DiskCleanupCandidate {
  id: string
  title: string
  description: string
  path: string
  category: string
  risk: 'safe' | 'review' | 'danger'
  size_bytes: number
  file_count: number
  exists: boolean
  scan_limited: boolean
  error?: string | null
}

export interface DiskCleanupScanResult {
  items: DiskCleanupCandidate[]
  total_size_bytes: number
  message: string
}

export interface DiskCleanupResult {
  cleaned: Array<{ id: string; title: string; size_bytes: number; file_count: number }>
  skipped: Array<{ id: string; reason: string }>
  errors: Array<{ id: string; title: string; error: string }>
  total_size_bytes: number
  message: string
}

export interface DiskUsageEntry {
  name: string
  path: string
  parent_path?: string | null
  is_dir: boolean
  size_bytes: number
  file_count: number
  depth?: number
  kind?: string
  scan_limited: boolean
  error?: string | null
}

export interface DiskUsageScanResult {
  path: string
  parent?: string | null
  filter?: string
  entries: DiskUsageEntry[]
  entry_total?: number
  page?: number
  size?: number
  total_size_bytes: number
  file_count: number
  scan_limited?: boolean
  disk_total_bytes?: number | null
  disk_available_bytes?: number | null
  disk_used_bytes?: number | null
  job_id?: number
  scanned_at?: string | null
  error?: string | null
}

export interface DiskUsageStatus {
  available: boolean
  root_path?: string
  scanned_at?: string
  total_size_bytes?: number
  file_count?: number
  node_count?: number
  scan_limited?: boolean
  disk_total_bytes?: number | null
  disk_available_bytes?: number | null
  disk_used_bytes?: number | null
}

export interface DiskUsageDeleteResult {
  deleted: boolean
  partial?: boolean
  path: string
  size_bytes?: number
  file_count?: number
  deleted_count?: number
  failed_count?: number
  failed_paths?: string[]
  message: string
}

export interface DiskUsageRevealResult {
  opened: boolean
  path?: string
  message: string
}

export function getSystemMonitor() {
  return api<ApiResponse<SystemMonitor>>('GET', '/system/monitor')
}

export function getDatabaseInfo() {
  return api<ApiResponse<DatabaseInfo>>('GET', '/system/database')
}

export function scanDiskCleanup() {
  return api<ApiResponse<DiskCleanupScanResult>>('GET', '/system/disk-cleanup/scan')
}

export function cleanDiskItems(ids: string[]) {
  return api<ApiResponse<DiskCleanupResult>>('POST', '/system/disk-cleanup/clean', { ids })
}

export function scanDiskUsage(path?: string | null) {
  return api<ApiResponse<DiskUsageScanResult>>('POST', '/system/disk-usage/scan', null, path ? { path } : {})
}

export function listDiskUsage(path?: string | null, filter = 'all', page = 1, size = 200) {
  return api<ApiResponse<DiskUsageScanResult>>('POST', '/system/disk-usage/list', null, { path, filter, page, size })
}

export function getDiskUsageStatus() {
  return api<ApiResponse<DiskUsageStatus>>('GET', '/system/disk-usage/status')
}

export function deleteDiskUsagePath(path: string) {
  return api<ApiResponse<DiskUsageDeleteResult>>('POST', '/system/disk-usage/delete', null, { path })
}

export function revealDiskUsagePath(path: string) {
  return api<ApiResponse<DiskUsageRevealResult>>('POST', '/system/disk-usage/reveal', null, { path })
}

export function clearAllData() {
  return api<ApiResponse<{ cleared_tables: string[]; message: string }>>('POST', '/system/clear-data')
}

export function clearLogs() {
  return api<ApiResponse<{ cleared_tables: string[]; message: string }>>('POST', '/system/clear-logs')
}

export function initializeDatabase() {
  return api<ApiResponse<{ cleared_tables: string[]; message: string }>>('POST', '/system/initialize-database')
}
