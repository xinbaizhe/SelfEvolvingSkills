import { api } from './tauri'
import type { ApiResponse } from './skills'

export interface CommunitySkill {
  id: number
  name: string
  repo: string
  repo_url: string
  stars: number
  description: string | null
  installed: boolean
  source?: string
  status: string
  fetched_at: string | null
}

export interface PaginatedResult<T> {
  items: T[]
  total: number
  page: number
  per_page: number
}

export function searchCommunitySkills(query: string, page = 1, perPage = 20) {
  return api<ApiResponse<PaginatedResult<CommunitySkill>>>('GET', '/community/search', { search: query, page, per_page: perPage, size: perPage })
}

export function installCommunitySkill(skillId: number, agentId: string) {
  return api<ApiResponse<{ status: string; id: number; path: string; agent_id: string; agent_name: string }>>(
    'POST',
    '/community/install',
    null,
    { id: skillId, agent_id: agentId },
  )
}

export function getInstalledSkills() {
  return api<ApiResponse<{ items: CommunitySkill[]; total: number }>>('GET', '/community/installed')
}
