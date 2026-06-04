import { api } from './tauri'
import type { ApiResponse } from './skills'

export function login(username: string, password: string) {
  return api<ApiResponse<{ token: string; username: string; user: { id: number; username: string } }>>('POST', '/admin/login', null, { username, password })
}

export function fetchUsers() {
  return api<ApiResponse<{ id: number; username: string; is_active: boolean }[]>>('GET', '/admin/users')
}

export function createUser(username: string, password: string) {
  return api<ApiResponse<unknown>>('POST', '/admin/users', null, { username, password })
}

export function updateUser(id: number, data: Record<string, unknown>) {
  return api<ApiResponse<unknown>>('PUT', `/admin/users/${id}`, null, data)
}

export function deleteUser(id: number) {
  return api<ApiResponse<{ id: number }>>('DELETE', `/admin/users/${id}`)
}

export function fetchSystemInfo() {
  return api<ApiResponse<unknown>>('GET', '/admin/system')
}

export function fetchConfig() {
  return api<ApiResponse<unknown[]>>('GET', '/admin/config')
}

export function updateConfig(key: string, value: string) {
  return api<ApiResponse<null>>('PUT', '/admin/config', null, { key, value })
}

export function fetchLlmConfig() {
  return api<ApiResponse<unknown>>('GET', '/admin/config/llm')
}

export function fetchScanPaths() {
  return api<ApiResponse<unknown[]>>('GET', '/admin/config/scan-paths')
}

export function updateLlmConfig(payload: Record<string, unknown>) {
  return api<ApiResponse<unknown>>('PUT', '/admin/config/llm', null, payload)
}

export function testLlmConnection(payload: Record<string, unknown>) {
  return api<ApiResponse<{ message: string }>>('POST', '/admin/config/llm/test', null, payload)
}

export function evaluateShareResource(payload: Record<string, unknown>) {
  return api<ApiResponse<unknown>>('POST', '/admin/config/llm/evaluate-share', null, payload)
}

export function evaluateSkillDirectory(payload: Record<string, unknown>) {
  return api<ApiResponse<unknown>>('POST', '/admin/config/llm/evaluate-directory', null, payload)
}

export function exportJson(resource: string) {
  return api<ApiResponse<string>>('GET', `/export/${resource}`)
}

export function exportCsv(resource: string) {
  return api<ApiResponse<string>>('GET', `/export/${resource}`, { format: 'csv' })
}

export interface DailyReport {
  report_date: string
  content: string
  source_count: number
  generated_at: string
  from_cache?: boolean
}

export function fetchDailyReport(date?: string) {
  return api<ApiResponse<DailyReport | null>>('GET', '/admin/daily-report', date ? { search: date } : undefined)
}

export interface GenStatus {
  generating: boolean
  target_date?: string
  report_date?: string
  phase?: string      // "scanning" | "summarizing" | "done" | "error"
  progress?: number   // 0-100
  message?: string
  error?: string
  // included when from_cache=true
  content?: string
  source_count?: number
  generated_at?: string
  from_cache?: boolean
}

export function generateDailyReport(date: string) {
  return api<ApiResponse<GenStatus>>('POST', '/admin/daily-report', null, { date })
}

export function fetchGenerationStatus() {
  return api<ApiResponse<GenStatus>>('GET', '/admin/daily-report/status')
}

export function fetchDailyReportHistory(limit = 30) {
  return api<ApiResponse<{ report_date: string; source_count: number; generated_at: string }[]>>('GET', '/admin/daily-report/history', { limit })
}
