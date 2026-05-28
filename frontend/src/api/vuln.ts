import { teamApiGet, teamApiPost } from './team'

export interface VulnFinding {
  severity: string
  type: string
  location: string
  description: string
  suggestion: string
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
  findings: VulnFinding[]
  userId: number
  deptId: number
  createdAt: string
}

export async function scanUrl(url: string): Promise<VulnScanJob> {
  const res = await teamApiPost<{ code: number; msg: string; data: VulnScanJob }>(
    '/vuln/scan-url',
    { url },
  )
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '扫描失败')
  return res.data
}

export async function scanCode(path: string): Promise<VulnScanJob> {
  const res = await teamApiPost<{ code: number; msg: string; data: VulnScanJob }>(
    '/vuln/scan-code',
    { path },
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

export async function fetchScanHistory(): Promise<VulnScanJob[]> {
  const res = await teamApiGet<{ code: number; msg: string; data: VulnScanJob[] }>(
    '/vuln/history',
  )
  if (res.code !== 200 || !res.data) throw new Error(res.msg || '获取历史失败')
  return res.data
}
