import { api } from './tauri'
import type { ApiResponse } from './skills'

export interface CommunitySkill {
  id: number
  name: string
  repo: string
  repo_url: string
  stars: number
  description: string | null
  skill_md_content?: string | null
  installed: boolean
  source?: string
  status: string
  fetched_at: string | null
  relevance_score?: number
  quality_score?: number
  weighted_score?: number
  license?: string | null
  pushed_at?: string | null
  matched_file?: string | null
  readme_excerpt?: string | null
  recommendation_reason?: string | null
}

export interface CompareResult {
  dimensions: CompareDimension[]
  suggestions: string[]
  summary?: string
  source: string
}

export interface CompareDimension {
  label: string
  local: string
  community: string
  verdict: 'local_better' | 'community_better' | 'complementary' | 'neutral'
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

export function fetchCommunitySkillDetail(id: number) {
  return api<ApiResponse<CommunitySkill>>('GET', `/community/${id}`)
}

export function compareCommunitySkill(data: {
  draft_body: string
  draft_name: string
  community_name: string
  community_content: string
}) {
  return api<ApiResponse<CompareResult>>('POST', '/community/compare', null, data)
}
