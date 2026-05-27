import { invoke } from '@tauri-apps/api/core'

export interface TeamSession {
  username: string
  user_id: number
  dept_name: string
  dept_id: number
  server_url: string
}

export interface TeamSkill {
  id: number
  name: string
  description: string
  category: string
  sourceType: string
  originAgent: string
  bodyMd: string
  authorId: number
  deptId: number
  compatibleModels: string
  compatibleAgents: string
  usageCount: number
  avgScore: number
  status: string
  version: number
  createdAt: string
  updatedAt: string
}

export interface TeamModelConfig {
  id: number
  name: string
  provider: string
  baseUrl: string
  model: string
  deptId: number
  isActive: number
  createdAt: string
}

export interface TeamProfile {
  username: string
  user_id: number
  dept_name: string
  dept_id: number
  available_models: TeamModelConfig[]
}

interface RuoyiResponse<T> {
  code: number
  msg: string
  data?: T
}

interface RuoyiPageResponse<T> {
  code: number
  msg: string
  rows: T[]
  total: number
}

export interface ConnectionStatus {
  connected: boolean
  authenticated: boolean
  refreshed?: boolean
  error?: string
}

export async function checkTeamConnection(): Promise<ConnectionStatus> {
  return invoke<ConnectionStatus>('check_team_connection')
}

export async function loginTeam(serverUrl: string, username: string, password: string): Promise<TeamSession> {
  const res = await invoke<{ username: string; user_id: number; dept_name: string; dept_id: number; token: string }>('login_team', { serverUrl, username, password })
  return { ...res, server_url: serverUrl }
}

export async function logoutTeam(): Promise<void> {
  return invoke<void>('logout_team')
}

export async function getTeamSession(): Promise<TeamSession | null> {
  return invoke<TeamSession | null>('get_team_session')
}

export async function teamApiGet<T>(path: string): Promise<T> {
  return invoke<T>('team_api_get', { path })
}

export async function teamApiPost<T>(path: string, body: unknown): Promise<T> {
  return invoke<T>('team_api_post', { path, body })
}

export async function teamApiPut<T>(path: string, body: unknown): Promise<T> {
  return invoke<T>('team_api_put', { path, body })
}

export async function teamApiDelete<T>(path: string): Promise<T> {
  return invoke<T>('team_api_delete', { path })
}

export interface DeptTreeNode {
  id: number
  label: string
  children?: DeptTreeNode[]
}

export interface RoleItem {
  roleId: number
  roleName: string
  roleKey: string
  roleSort: number
  status: string
}

export interface PostItem {
  postId: number
  postCode: string
  postName: string
  postSort: number
  status: string
}

export async function fetchDeptTree() {
  const res = await teamApiGet<RuoyiResponse<DeptTreeNode[]>>('/team/deptTree')
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '查询失败')
  return res.data
}

export async function fetchAllPosts() {
  const res = await teamApiGet<RuoyiResponse<PostItem[]>>('/team/posts')
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '查询失败')
  return res.data
}

export async function fetchAllRoles() {
  const res = await teamApiGet<RuoyiResponse<RoleItem[]>>('/team/roles')
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '查询失败')
  return res.data
}

export async function fetchTeamSkills(params?: Record<string, string>) {
  const query = params ? '?' + new URLSearchParams(params).toString() : ''
  const res = await teamApiGet<RuoyiPageResponse<TeamSkill>>(`/team/skills/list${query}`)
  return { items: res.rows ?? [], total: res.total ?? 0 }
}

export async function fetchTeamSkillDetail(id: number) {
  const res = await teamApiGet<RuoyiResponse<TeamSkill>>(`/team/skills/${id}`)
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '查询失败')
  return res.data
}

export async function shareSkillToTeam(skill: {
  name: string; description: string; category: string;
  bodyMd: string; originAgent?: string;
  compatibleModels?: string; compatibleAgents?: string
}) {
  const res = await teamApiPost<RuoyiResponse<TeamSkill>>('/team/skills', skill)
  if (res.code !== 200) throw new Error(res.msg || '分享失败')
  return res.data
}

export async function fetchTeamStats() {
  const res = await teamApiGet<RuoyiResponse<{ totalSkills: number; totalInstalls: number }>>('/team/skills/stats')
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '查询失败')
  return res.data
}

export async function fetchTeamProfile() {
  const res = await teamApiGet<RuoyiResponse<TeamProfile>>('/team/profile')
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '查询失败')
  return res.data
}

export async function fetchAvailableModels() {
  const res = await teamApiGet<RuoyiResponse<TeamModelConfig[]>>('/team/models/available')
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '查询失败')
  return res.data
}

export async function fetchModelConfigs(params?: Record<string, string>) {
  const query = params ? '?' + new URLSearchParams(params).toString() : ''
  const res = await teamApiGet<RuoyiPageResponse<TeamModelConfig>>(`/team/models/list${query}`)
  return { items: res.rows ?? [], total: res.total ?? 0 }
}

export async function createModelConfig(config: Partial<TeamModelConfig>) {
  const res = await teamApiPost<RuoyiResponse<TeamModelConfig>>('/team/models', config)
  if (res.code !== 200) throw new Error(res.msg || '创建失败')
  return res.data
}

export async function updateModelConfig(config: Partial<TeamModelConfig>) {
  const res = await teamApiPut<RuoyiResponse<TeamModelConfig>>('/team/models', config)
  if (res.code !== 200) throw new Error(res.msg || '更新失败')
  return res.data
}

export async function deleteModelConfigs(ids: number[]) {
  const res = await teamApiDelete<RuoyiResponse<null>>(`/team/models/${ids.join(',')}`)
  if (res.code !== 200) throw new Error(res.msg || '删除失败')
}

export interface TeamEvolution {
  id: number
  skillId: number
  proposerId: number
  previousVersion: string
  proposedChange: string
  reason: string
  status: string
  reviewerId: number
  reviewComment: string
  reviewedAt: string
  createdAt: string
}

export async function fetchEvolutions(params?: Record<string, string>) {
  const query = params ? '?' + new URLSearchParams(params).toString() : ''
  const res = await teamApiGet<RuoyiPageResponse<TeamEvolution>>(`/team/evolutions/list${query}`)
  return { items: res.rows ?? [], total: res.total ?? 0 }
}

export async function fetchEvolutionsBySkill(skillId: number) {
  const res = await teamApiGet<RuoyiResponse<TeamEvolution[]>>(`/team/evolutions/skill/${skillId}`)
  if (res.code !== 200) throw new Error(res.msg || '查询失败')
  return res.data ?? []
}

export async function submitEvolution(evolution: {
  skillId: number; proposedChange: string; reason: string; previousVersion?: string
}) {
  const res = await teamApiPost<RuoyiResponse<TeamEvolution>>('/team/evolutions', evolution)
  if (res.code !== 200) throw new Error(res.msg || '提交失败')
  return res.data
}

export async function approveEvolution(id: number, body: { status: string; reviewComment?: string }) {
  const res = await teamApiPost<RuoyiResponse<null>>(`/team/evolutions/${id}/approve`, body)
  if (res.code !== 200) throw new Error(res.msg || '操作失败')
}

// ---- Offline cache ----

export interface CacheSummary {
  has_cache: boolean
  count: number
  cached_at: string | null
}

export interface CachedSkills {
  skills: TeamSkill[]
  cached_at: string
  count: number
}

export interface PendingOps {
  ops: PendingOp[]
  count: number
}

export interface PendingOp {
  id: number
  opType: string
  path: string
  method: string
  body: string | null
  status: string
  errorMessage: string | null
  retryCount: number
  createdAt: string
}

export async function cacheTeamSkills(skills: TeamSkill[]): Promise<{ cached: number }> {
  return invoke<{ cached: number }>('cache_team_skills', { skills })
}

export async function getCachedTeamSkills(): Promise<CachedSkills> {
  return invoke<CachedSkills>('get_cached_team_skills')
}

export async function getTeamCacheSummary(): Promise<CacheSummary> {
  return invoke<CacheSummary>('get_team_cache_summary')
}

export async function getPendingOperations(): Promise<PendingOps> {
  return invoke<PendingOps>('get_pending_operations')
}

export async function flushPendingOperations(): Promise<{ synced: number; failed: number }> {
  return invoke<{ synced: number; failed: number }>('flush_pending_operations')
}

export async function queueTeamOperation(op: {
  opType: string; path: string; method: string; body?: unknown
}): Promise<{ id: number }> {
  return invoke<{ id: number }>('queue_team_operation', {
    opType: op.opType,
    path: op.path,
    method: op.method,
    body: op.body ?? null,
  })
}
