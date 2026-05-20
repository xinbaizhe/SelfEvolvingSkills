import { api } from './tauri'
import type { ApiResponse, PaginatedResult } from './skills'

export interface SummaryStats {
  total_skills: number
  total_agents: number
  total_sessions: number
  total_memories: number
  total_skill_usages: number
  enabled_sources: number
  available_sources: number
}

export function fetchSummary() {
  return api<ApiResponse<SummaryStats>>('GET', '/stats/summary')
}

export function fetchTopSkills(limit = 20) {
  return api<ApiResponse<unknown[]>>('GET', '/stats/skills/top', { limit })
}

export function fetchSkillsByCategory() {
  return api<ApiResponse<{ category: string; count: number }[]>>('GET', '/stats/skills/by-category')
}

export function fetchSessions(params: Record<string, any> = {}) {
  return api<ApiResponse<PaginatedResult<unknown>>>('GET', '/sessions', {
    page: params.page || 1,
    size: params.size || 50,
    project: params.project_name || null,
  })
}

export function fetchSessionDetail(sessionId: string) {
  return api<ApiResponse<unknown>>('GET', `/sessions/${encodeURIComponent(sessionId)}`)
}
