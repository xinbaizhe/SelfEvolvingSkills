import { api } from './tauri'
import type { ApiResponse } from './skills'

export interface EvolutionPhase {
  id: number
  phase: string
  status: string
  progress: number
  message: string | null
  started_at: string | null
  completed_at: string | null
}

export interface EvolutionRun {
  run_id: number
  status: string
  phases: EvolutionPhase[]
  started_at: string | null
  completed_at: string | null
}

export interface EvolutionJob {
  run_id: number
  phases: EvolutionPhase[]
  steps: EvolutionStep[]
  running: boolean
  current_phase?: string | null
  last_completed?: {
    run_id: number
    completed_at: string | null
  } | null
}

export interface EvolutionStep {
  phase: string
  label: string
  start: number
  end: number
}

export interface PaginatedResult<T> {
  items: T[]
  total: number
  page: number
  size: number
}

export function startEvolution(agentIds?: string[]) {
  return api<ApiResponse<{ run_id: number; status: string; started_at: string }>>('POST', '/evolution/start', null, {
    agent_ids: agentIds && agentIds.length > 0 ? agentIds : null,
  })
}

export function getEvolutionStatus() {
  return api<ApiResponse<EvolutionJob>>('GET', '/evolution/status')
}

export function getEvolutionHistory(page = 1, size = 20) {
  return api<ApiResponse<PaginatedResult<EvolutionRun>>>('GET', '/evolution/history', { page, size })
}
