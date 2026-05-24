import { api } from './tauri'
import type { ApiResponse, PaginatedResult } from './skills'

export interface Agent {
  id: number
  name: string
  description: string | null
  tools: unknown | null
  model: string | null
  agent_source: string | null
  file_path: string
  body_size: number
  line_count: number
  file_mtime: number | null
}

export interface AgentDetail extends Agent {
  yaml_raw: string | null
  body_text: string | null
  file_size: number
}

export function fetchAgents(params: Record<string, any> = {}) {
  return api<ApiResponse<PaginatedResult<Agent>>>('GET', '/agents', {
    page: params.page || 1,
    size: params.size || 50,
    agent_source: params.agent_source || null,
    search: params.search || null,
  })
}

export function fetchAgentDetail(name: string) {
  return api<ApiResponse<AgentDetail>>('GET', `/agents/${encodeURIComponent(name)}`)
}

export function updateAgent(name: string, data: { name?: string; description?: string | null; model?: string | null; tools?: unknown }) {
  return api<ApiResponse<AgentDetail>>('PUT', `/agents/${encodeURIComponent(name)}`, null, data as unknown as Record<string, unknown>)
}

export function deleteAgent(name: string) {
  return api<ApiResponse<{ deleted: boolean; name: string; file_path: string }>>('DELETE', `/agents/${encodeURIComponent(name)}`)
}

export function evolveAgent(name: string) {
  return api<ApiResponse<any>>('POST', `/agents/${encodeURIComponent(name)}/evolve`)
}
