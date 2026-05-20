import { api } from './tauri'
import type { ApiResponse } from './skills'

export interface SourceConfig {
  id: number
  agent_id: string
  agent_name: string
  detected_path: string | null
  custom_paths: string | null
  paths: Record<string, string | null>
  is_enabled: boolean
  is_available: boolean
  record_count: number
  last_activity: string | null
  last_scan_at: string | null
}

export interface ScanJob {
  id: number
  scan_type: string
  status: string
  started_at: string
  completed_at: string | null
  skills_found: number
  agents_found: number
  sessions_found: number
  conversations_analyzed: number
  memories_found: number
  sources_scanned: number
  errors: string | null
}

export function triggerScan(agentIds?: string[]) {
  return api<ApiResponse<ScanJob>>('POST', '/scan', null, {
    agent_ids: agentIds && agentIds.length > 0 ? agentIds : null,
  })
}

export function fetchScanHistory(params: Record<string, any> = {}) {
  return api<ApiResponse<{ items: ScanJob[]; total: number; page: number; size: number }>>('GET', '/scan/history', {
    page: params.page || 1,
    size: params.size || 20,
  })
}

export function fetchSources() {
  return api<ApiResponse<SourceConfig[]>>('GET', '/scan/sources')
}

export function detectSources() {
  return api<ApiResponse<SourceConfig[]>>('POST', '/scan/sources/detect')
}

export function updateSource(agentId: string, data: { is_enabled?: boolean; custom_paths?: string | null; paths?: Record<string, string | null> }) {
  return api<ApiResponse<SourceConfig>>('PUT', `/scan/sources/${encodeURIComponent(agentId)}`, null, {
    is_enabled: data.is_enabled,
    paths: data.paths ?? null,
    detected_path: data.custom_paths ?? null,
  } as Record<string, unknown>)
}

export function deleteSource(agentId: string) {
  return api<ApiResponse<{ agent_id: string; message: string }>>('DELETE', `/scan/sources/${encodeURIComponent(agentId)}`)
}
