import { api } from './tauri'

export interface SkillItem {
  id: number
  name: string
  description: string | null
  category: string | null
  source_type: string
  plugin_name: string | null
  origin: string | null
  usage_count: number
  session_count: number
  body_size: number
  line_count: number
  file_mtime: number | null
}

export interface SkillDetail extends SkillItem {
  category_tags: string[]
  file_path: string
  yaml_raw: string | null
  body_text: string | null
  file_size: number
  created_at: string | null
  updated_at: string | null
}

export interface ApiResponse<T> {
  success: boolean
  data: T | null
  error: string | null
}

export interface PaginatedResult<T> {
  items: T[]
  total: number
  page: number
  size: number
}

export function fetchSkills(params: Record<string, any> = {}) {
  return api<ApiResponse<PaginatedResult<SkillItem>>>('GET', '/skills', {
    page: params.page || 1,
    size: params.size || 50,
    category: params.category || null,
    source: params.source || null,
    agent_source: params.agent_source || null,
    search: params.search || null,
  })
}

export function fetchSkillDetail(name: string) {
  return api<ApiResponse<SkillDetail>>('GET', `/skills/${encodeURIComponent(name)}`)
}

export function fetchCategories() {
  return api<ApiResponse<{ category: string; count: number }[]>>('GET', '/skills/categories')
}

export function importSkills(agentId: string, filename: string, content: string, contentBase64?: string) {
  return api<ApiResponse<{ agent_id: string; agent_name: string; skills_path: string; count: number; imported: { name: string; path: string }[]; skipped?: { path: string; reason: string }[] }>>(
    'POST',
    '/skills/import',
    null,
    { agent_id: agentId, filename, content, content_base64: contentBase64 || null },
  )
}

export function evolveSkill(name: string) {
  return api<ApiResponse<any>>('POST', `/skills/${encodeURIComponent(name)}/evolve`)
}

export function updateSkill(name: string, data: { name?: string; description?: string | null; category?: string | null }) {
  return api<ApiResponse<SkillDetail>>('PUT', `/skills/${encodeURIComponent(name)}`, null, data as unknown as Record<string, unknown>)
}

export function deleteSkill(name: string) {
  return api<ApiResponse<{ deleted: boolean; name: string; file_path: string }>>('DELETE', `/skills/${encodeURIComponent(name)}`)
}
