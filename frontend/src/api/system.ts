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

export function getSystemMonitor() {
  return api<ApiResponse<SystemMonitor>>('GET', '/system/monitor')
}

export function getDatabaseInfo() {
  return api<ApiResponse<DatabaseInfo>>('GET', '/system/database')
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
