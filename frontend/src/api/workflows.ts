import { api } from './tauri'
import type { ApiResponse } from './skills'

export interface Workflow {
  id: number
  name: string
  description: string | null
  frequency: number
  source_agents: string | null
  estimated_time_saved: string | null
  can_generate_skill: boolean
  skill_score: number
  status: string
  draft_body: string | null
  sample_tasks: string | null
  recommendation_source?: string
  confidence?: number | null
  reasoning?: string | null
  source_skills?: string[]
  similar_skills?: { name: string; source: string; url?: string }[]
  review_score?: number | null
  review_summary?: string | null
  review_feedback?: SkillReviewFeedback | null
  evolves_skill?: string | null
  iteration_num?: number | null
  installed_agent_id?: string | null
  created_at: string
  updated_at: string
}

export interface SkillVariant {
  id: number
  skill_name: string
  variant_label: string
  status: string
  usage_count: number
  avg_session_messages: number
  performance_score: number
  created_at: string
  updated_at: string
}

export interface SkillReviewFeedback {
  verdict?: string
  safety?: string[]
  performance?: string[]
  functionality?: string[]
  writing?: string[]
  improvements?: string[]
}

export interface SkillInstallTarget {
  agent_id: string
  agent_name: string
  skills_path: string
  is_enabled: boolean
  is_available: boolean
}

export function fetchWorkflows() {
  return api<ApiResponse<Workflow[]>>('GET', '/workflows')
}

export function clusterWorkflows() {
  return api<ApiResponse<Workflow[]>>('POST', '/workflows/cluster')
}

export function fetchWorkflow(id: number) {
  return api<ApiResponse<Workflow>>('GET', `/workflows/${id}`)
}

export function fetchSkillInstallTargets() {
  return api<ApiResponse<SkillInstallTarget[]>>('GET', '/workflows/install-targets')
}

export function installWorkflowSkill(id: number, agentId: string) {
  return api<ApiResponse<{ path: string; agent_id: string; agent_name: string }>>('POST', `/workflows/${id}/install`, null, {
    agent_id: agentId,
  })
}

export function deleteWorkflowSkill(id: number, agentId?: string) {
  return api<ApiResponse<{ id: number }>>('DELETE', `/workflows/${id}`, null, agentId ? { agent_id: agentId } : null)
}

export function updateWorkflowDraft(id: number, data: { draft_body: string; description?: string | null }) {
  return api<ApiResponse<Workflow>>('PUT', `/workflows/${id}`, null, data)
}

export function deleteWorkflowDraft(id: number) {
  return api<ApiResponse<{ id: number }>>('DELETE', `/workflows/${id}`)
}

export function fetchSkillVariants(params: Record<string, any> = {}) {
  return api<ApiResponse<{ items: SkillVariant[]; total: number }>>('GET', '/skill-variants', {
    page: params.page || 1,
    size: params.size || 50,
  })
}
