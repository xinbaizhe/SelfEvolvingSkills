import { api } from './tauri'
import type { ApiResponse } from './skills'

export interface ExportPayload {
  content: string
  filename: string
  content_type: string
}

export async function exportSkills(format: 'json' | 'csv', agentSource?: string): Promise<ExportPayload> {
  const params: Record<string, string> = {}
  if (format === 'csv') params.format = 'csv'
  if (agentSource) params.agent_source = agentSource
  const res = await api<ApiResponse<string>>('GET', '/export/skills', params)
  if (res.success && res.data) {
    const suffix = agentSource ? `-${agentSource}` : ''
    return {
      content: res.data,
      filename: format === 'csv' ? `skills${suffix}.csv` : `skills${suffix}.json`,
      content_type: format === 'csv' ? 'text/csv' : 'application/json',
    }
  }
  throw new Error(res.error || '导出失败')
}

export async function exportAgents(): Promise<ExportPayload> {
  const res = await api<ApiResponse<string>>('GET', '/export/agents')
  if (res.success && res.data) {
    return { content: res.data, filename: 'agents.json', content_type: 'application/json' }
  }
  throw new Error(res.error || 'Export failed')
}
