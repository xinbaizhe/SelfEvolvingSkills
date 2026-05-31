import { defineStore } from 'pinia'
import { ref } from 'vue'
import {
  scanUrl,
  scanCode,
  fetchScanHistory,
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
  type DepSnapshot,
  type DepFinding,
  type UploadManifestResult,
} from '../api/vuln'

export const useVulnStore = defineStore('vuln', () => {
  const scanning = ref(false)
  const currentResult = ref<VulnScanJob | null>(null)
  const history = ref<VulnScanJob[]>([])
  const intel = ref<VulnIntel[]>([])
  const intelLoading = ref(false)
  const error = ref('')

  // Dep monitor state
  const snapshots = ref<DepSnapshot[]>([])
  const currentSnapshotDeps = ref<DepSnapshot[]>([])
  const depFindings = ref<DepFinding[]>([])
  const depLoading = ref(false)

  async function runUrlScan(url: string, options: VulnScanOptions = {}): Promise<VulnScanJob | null> {
    scanning.value = true
    error.value = ''
    currentResult.value = null
    try {
      const job = await scanUrl(url, options)
      currentResult.value = job
      return job
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
      return null
    } finally {
      scanning.value = false
    }
  }

  async function runCodeScan(path: string, options: VulnScanOptions = {}): Promise<VulnScanJob | null> {
    scanning.value = true
    error.value = ''
    currentResult.value = null
    try {
      const job = await scanCode(path, options)
      currentResult.value = job
      return job
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
      return null
    } finally {
      scanning.value = false
    }
  }

  async function loadHistory(): Promise<void> {
    try {
      history.value = await fetchScanHistory()
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  function clearResult() {
    currentResult.value = null
    error.value = ''
  }

  async function loadIntel(params?: Record<string, string>) {
    intelLoading.value = true
    try {
      intel.value = await fetchVulnIntel(params)
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
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
    currentResult,
    history,
    intel,
    intelLoading,
    error,
    snapshots,
    currentSnapshotDeps,
    depFindings,
    depLoading,
    runUrlScan,
    runCodeScan,
    loadHistory,
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
