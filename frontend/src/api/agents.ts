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
