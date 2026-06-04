import { defineStore } from 'pinia'
import { ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  scanUrl,
  scanUrlStream,
  scanUrlAgentStream,
  scanCode,
  fetchScanHistory,
  fetchScanResult,
  fetchVulnIntel,
  uploadManifest,
  fetchDepSnapshots,
  fetchDepDeps,
  fetchDepFindings,
  refreshDepSnapshot,
  deleteDepSnapshot,
  type VulnScanJob,
  type VulnIntel,
  type VulnScanOptions,
  type AgentCredential,
  type DepSnapshot,
  type DepFinding,
  type UploadManifestResult,
} from '../api/vuln'

function isConnectionError(e: unknown): boolean {
  const msg = e instanceof Error ? e.message : String(e)
  return msg.includes('error sending request for url')
      || msg.includes('ConnectError')
      || msg.includes('connection refused')
      || msg.includes('NetworkError')
      || msg.includes('fetch failed')
}

export const useVulnStore = defineStore('vuln', () => {
  const scanning = ref(false)
  const scanProgress = ref<string[]>([])
  const currentResult = ref<VulnScanJob | null>(null)
  const history = ref<VulnScanJob[]>([])
  const historyTotal = ref(0)
  const selectedHistoryJob = ref<VulnScanJob | null>(null)
  const intel = ref<VulnIntel[]>([])
  const intelLoading = ref(false)
  const error = ref('')
  const offline = ref(false)

  // Dep monitor state
  const snapshots = ref<DepSnapshot[]>([])
  const currentSnapshotDeps = ref<DepSnapshot[]>([])
  const depFindings = ref<DepFinding[]>([])
  const depLoading = ref(false)

  async function runUrlScan(url: string, options: VulnScanOptions = {}): Promise<VulnScanJob | null> {
    scanning.value = true
    error.value = ''
    offline.value = false
    currentResult.value = null
    try {
      const job = await scanUrl(url, options)
      currentResult.value = job
      return job
    } catch (e: unknown) {
      if (isConnectionError(e)) {
        offline.value = true
      } else {
        error.value = e instanceof Error ? e.message : String(e)
      }
      return null
    } finally {
      scanning.value = false
    }
  }

  async function runUrlScanStream(url: string, options: VulnScanOptions = {}): Promise<VulnScanJob | null> {
    scanning.value = true
    error.value = ''
    offline.value = false
    currentResult.value = null
    scanProgress.value = []

    let unlisten: UnlistenFn | null = null
    try {
      unlisten = await listen<string>('scan-progress', (event) => {
        scanProgress.value = [...scanProgress.value, event.payload]
      })

      const job = await scanUrlStream({
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
      })
      currentResult.value = job
      return job
    } catch (e: unknown) {
      if (isConnectionError(e)) {
        offline.value = true
      } else {
        error.value = e instanceof Error ? e.message : String(e)
      }
      return null
    } finally {
      scanning.value = false
      if (unlisten) unlisten()
    }
  }

  async function runUrlAgentStream(
    url: string,
    options: { modelType?: string; modelId?: number; agentCredentials?: AgentCredential[] } = {}
  ): Promise<VulnScanJob | null> {
    scanning.value = true
    error.value = ''
    offline.value = false
    currentResult.value = null
    scanProgress.value = []

    let unlisten: UnlistenFn | null = null
    try {
      unlisten = await listen<string>('scan-progress', (event) => {
        scanProgress.value = [...scanProgress.value, event.payload]
      })

      const job = await scanUrlAgentStream({
        url,
        modelType: options.modelType,
        modelId: options.modelId != null ? String(options.modelId) : undefined,
        agentCredentials: options.agentCredentials,
      })
      currentResult.value = job
      return job
    } catch (e: unknown) {
      if (isConnectionError(e)) {
        offline.value = true
      } else {
        error.value = e instanceof Error ? e.message : String(e)
      }
      return null
    } finally {
      scanning.value = false
      if (unlisten) unlisten()
    }
  }

  async function runCodeScan(path: string, options: VulnScanOptions = {}): Promise<VulnScanJob | null> {
    scanning.value = true
    error.value = ''
    offline.value = false
    currentResult.value = null
    try {
      const job = await scanCode(path, options)
      currentResult.value = job
      return job
    } catch (e: unknown) {
      if (isConnectionError(e)) {
        offline.value = true
      } else {
        error.value = e instanceof Error ? e.message : String(e)
      }
      return null
    } finally {
      scanning.value = false
    }
  }

  async function loadHistory(pageNum = 1, pageSize = 10): Promise<void> {
    try {
      const result = await fetchScanHistory(pageNum, pageSize)
      history.value = result.items
      historyTotal.value = result.total
      offline.value = false
    } catch (e: unknown) {
      if (isConnectionError(e)) {
        offline.value = true
      } else {
        error.value = e instanceof Error ? e.message : String(e)
      }
    }
  }

  async function loadHistoryDetail(jobId: number): Promise<void> {
    try {
      selectedHistoryJob.value = await fetchScanResult(jobId)
      offline.value = false
    } catch (e: unknown) {
      if (isConnectionError(e)) {
        offline.value = true
      } else {
        error.value = e instanceof Error ? e.message : String(e)
      }
    }
  }

  function clearHistoryDetail() {
    selectedHistoryJob.value = null
  }

  function clearResult() {
    currentResult.value = null
    error.value = ''
  }

  async function loadIntel(params?: Record<string, string>) {
    intelLoading.value = true
    error.value = ''
    try {
      intel.value = await fetchVulnIntel(params)
      offline.value = false
    } catch (e: unknown) {
      if (isConnectionError(e)) {
        offline.value = true
      } else {
        error.value = e instanceof Error ? e.message : String(e)
      }
    } finally {
      intelLoading.value = false
    }
  }

  // Dep monitor actions
  async function uploadMonitor(name: string, files: { name: string; content: string }[]): Promise<UploadManifestResult | null> {
    depLoading.value = true
    error.value = ''
    try {
      const result = await uploadManifest(name, files)
      await loadSnapshots()
      return result
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
      return null
    } finally {
      depLoading.value = false
    }
  }

  async function loadSnapshots(): Promise<void> {
    depLoading.value = true
    try {
      snapshots.value = await fetchDepSnapshots()
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      depLoading.value = false
    }
  }

  async function loadDepDeps(snapshotId: number, name: string): Promise<void> {
    depLoading.value = true
    try {
      currentSnapshotDeps.value = await fetchDepDeps(snapshotId, name)
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      depLoading.value = false
    }
  }

  async function loadDepFindings(depId: number): Promise<void> {
    try {
      depFindings.value = await fetchDepFindings(depId)
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function refreshSnapshot(id: number): Promise<void> {
    depLoading.value = true
    try {
      await refreshDepSnapshot(id)
      await loadSnapshots()
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      depLoading.value = false
    }
  }

  async function removeSnapshot(id: number): Promise<void> {
    try {
      await deleteDepSnapshot(id)
      await loadSnapshots()
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  function clearDepDetail() {
    currentSnapshotDeps.value = []
    depFindings.value = []
  }

  return {
    scanning,
    scanProgress,
    currentResult,
    history,
    historyTotal,
    selectedHistoryJob,
    intel,
    intelLoading,
    error,
    offline,
    snapshots,
    currentSnapshotDeps,
    depFindings,
    depLoading,
    runUrlScan,
    runUrlScanStream,
    runUrlAgentStream,
    runCodeScan,
    loadHistory,
    loadHistoryDetail,
    clearHistoryDetail,
    loadIntel,
    clearResult,
    uploadMonitor,
    loadSnapshots,
    loadDepDeps,
    loadDepFindings,
    refreshSnapshot,
    removeSnapshot,
    clearDepDetail,
  }
})
