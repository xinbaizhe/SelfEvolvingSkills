import { teamApiGet, teamApiPost, teamApiDelete } from './team'
import { invoke } from '@tauri-apps/api/core'

export interface VulnFinding {
  severity: string
  type: string
  location: string
  description: string
  suggestion: string
  confidence?: number
}

export interface VulnScanJob {
  id: number
  scanType: string
  target: string
  status: string
  totalFindings: number
  criticalCount: number
  highCount: number
  mediumCount: number
  lowCount: number
  modelType?: string
  modelId?: number
  progressStep?: string
  progressText?: string
  findings: VulnFinding[]
  userId: number
  deptId: number
  createdAt: string
}

export interface VulnScanOptions {
  modelType?: 'department' | 'personal'
  modelId?: number
  cookie?: string
  authorization?: string
  headers?: string
  scanProfile?: 'quick' | 'standard' | 'deep'
  customPaths?: string
  maxDepth?: number
  maxPages?: number
  portScanEnabled?: boolean
  portSpec?: string
}

export interface VulnIntel {
  id: number
  source: string
  cveId: string
  title: string
  vulnType: string
  severity: string
  vendorProject: string
  product: string
  ecosystem: string | null
  isPoisoning: boolean
  aliases: string | null
  description: string
  referenceUrl: string
  publishedAt: string
  updatedAt: string
  createdAt: string
}

export interface DepSnapshot {
  id: number
  name: string
  ecosystem: string
  packageName: string
  version: string | null
  vulnCount: number
  poisoningCount: number
  lastCheckedAt: string | null
  createdAt: string
}

export interface DepFinding {
  id: number
  depId: number
  cveId: string
  title: string
  severity: string
  isPoisoning: boolean
  referenceUrl: string
  createdAt: string
}

export interface UploadManifestResult {
  snapshot: string
  deps: DepSnapshot[]
}

export interface AgentCredential {
  credId: string
  role: string
  username: string
  cookie: string
  authorization: string
  permissions: string
  sessionValid: boolean
}

export async function scanUrlWithAgent(
  url: string,
  options: VulnScanOptions = {},
  credentials: AgentCredential[] = [],
): Promise<VulnScanJob> {
  const res = await teamApiPost<{ code: number; msg: string; data: VulnScanJob }>(
    '/vuln/scan-url/agent',
    {
      url,
      modelType: options.modelType,
      modelId: options.modelId != null ? String(options.modelId) : undefined,
      agentCredentials: credentials,
    },
  )
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '扫描失败，Agent 未启用')
  return res.data
}

export async function scanUrl(url: string, options: VulnScanOptions = {}): Promise<VulnScanJob> {
  const res = await teamApiPost<{ code: number; msg: string; data: VulnScanJob }>(
    '/vuln/scan-url',
    {
      url,
      modelType: options.modelType,
      modelId: options.modelId != null ? String(options.modelId) : undefined,
      cookie: options.cookie,
      authorization: options.authorization,
      headers: options.headers,
      scanProfile: options.scanProfile,
      customPaths: options.customPaths,
      maxDepth: options.maxDepth != null ? String(options.maxDepth) : undefined,
      maxPages: options.maxPages != null ? String(options.maxPages) : undefined,
      portScanEnabled: options.portScanEnabled != null ? String(options.portScanEnabled) : undefined,
      portSpec: options.portSpec,
    },
  )
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '扫描失败')
  return res.data
}

export async function scanUrlStream(options: {
  url: string
  modelType?: string
  modelId?: string
  cookie?: string
  authorization?: string
  headers?: string
  scanProfile?: string
  customPaths?: string
  maxDepth?: string
  maxPages?: string
  portScanEnabled?: string
  portSpec?: string
}): Promise<VulnScanJob> {
  return invoke<VulnScanJob>('scan_url_stream', {
    url: options.url,
    modelType: options.modelType,
    modelId: options.modelId,
    cookie: options.cookie,
    authorization: options.authorization,
    headers: options.headers,
    scanProfile: options.scanProfile,
    customPaths: options.customPaths,
    maxDepth: options.maxDepth,
    maxPages: options.maxPages,
    portScanEnabled: options.portScanEnabled,
    portSpec: options.portSpec,
  })
}

export async function scanUrlAgentStream(options: {
  url: string
  modelType?: string
  modelId?: string
  agentCredentials?: AgentCredential[]
}): Promise<VulnScanJob> {
  return invoke<VulnScanJob>('scan_url_agent_stream', {
    url: options.url,
    modelType: options.modelType,
    modelId: options.modelId,
    agentCredentials: options.agentCredentials || undefined,
  })
}

export async function scanCode(path: string, options: VulnScanOptions = {}): Promise<VulnScanJob> {
  const res = await teamApiPost<{ code: number; msg: string; data: VulnScanJob }>(
    '/vuln/scan-code',
    { path, modelType: options.modelType, modelId: options.modelId ? String(options.modelId) : undefined },
  )
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '扫描失败')
  return res.data
}

export async function fetchScanResult(jobId: number): Promise<VulnScanJob> {
  const res = await teamApiGet<{ code: number; msg: string; data: VulnScanJob }>(
    `/vuln/scan/${jobId}`,
  )
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '获取失败')
  return res.data
}

export interface PaginatedHistory {
  items: VulnScanJob[]
  total: number
}

export async function fetchScanHistory(pageNum = 1, pageSize = 10): Promise<PaginatedHistory> {
  const res = await teamApiGet<{ code: number; msg: string; rows: VulnScanJob[]; total: number }>(
    `/vuln/history?pageNum=${pageNum}&pageSize=${pageSize}`,
  )
  if (res.code !== 200) throw new Error(res.msg || '获取历史失败')
  return { items: res.rows ?? [], total: res.total ?? 0 }
}

export async function fetchVulnIntel(params?: Record<string, string>): Promise<VulnIntel[]> {
  const query = params ? '?' + new URLSearchParams(params).toString() : ''
  const res = await teamApiGet<{ code: number; msg: string; data: VulnIntel[] }>(`/vuln/intel/list${query}`)
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '查询失败')
  return res.data
}

export async function uploadManifest(name: string, files: { name: string; content: string }[]): Promise<UploadManifestResult> {
  const res = await teamApiPost<{ code: number; msg: string; data: UploadManifestResult }>(
    '/vuln/monitor/upload',
    { name, files },
  )
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '上传失败')
  return res.data
}

export async function fetchDepSnapshots(): Promise<DepSnapshot[]> {
  const res = await teamApiGet<{ code: number; msg: string; data: DepSnapshot[] }>(
    '/vuln/monitor/snapshots',
  )
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '获取失败')
  return res.data
}

export async function fetchDepDeps(snapshotId: number, name: string): Promise<DepSnapshot[]> {
  const res = await teamApiGet<{ code: number; msg: string; data: DepSnapshot[] }>(
    `/vuln/monitor/${snapshotId}/deps?name=${encodeURIComponent(name)}`,
  )
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '获取失败')
  return res.data
}

export async function fetchDepFindings(depId: number): Promise<DepFinding[]> {
  const res = await teamApiGet<{ code: number; msg: string; data: DepFinding[] }>(
    `/vuln/monitor/dep/${depId}/findings`,
  )
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '获取失败')
  return res.data
}

export async function refreshDepSnapshot(id: number): Promise<DepFinding[]> {
  const res = await teamApiPost<{ code: number; msg: string; data: DepFinding[] }>(
    `/vuln/monitor/${id}/refresh`,
    {},
  )
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '刷新失败')
  return res.data
}

export async function deleteDepSnapshot(id: number): Promise<void> {
  const res = await teamApiDelete<{ code: number; msg: string }>(
    `/vuln/monitor/${id}`,
  )
  if (res.code !== 200) throw new Error(res.msg || '删除失败')
}
