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
