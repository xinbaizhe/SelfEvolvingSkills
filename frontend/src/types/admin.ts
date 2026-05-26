export interface AdminUser {
  id: number
  username: string
  is_active: boolean
  created_at?: string
}

export interface SystemInfo {
  total_skills: number
  total_agents: number
  total_sessions: number
}

export interface ScanPathSource {
  agent_id: string
  agent_name: string
  is_available: boolean
  record_count: number
  paths: Record<string, string | null>
}

export interface ScanPathEntry {
  type: string
  path: string
}

export interface LlmConfigData {
  enabled?: boolean
  provider?: string
  base_url?: string
  model?: string
  api_format?: string
  api_key_configured?: boolean
  has_api_key?: boolean
}
