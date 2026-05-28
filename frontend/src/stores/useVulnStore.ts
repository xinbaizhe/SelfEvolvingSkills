import { defineStore } from 'pinia'
import { ref } from 'vue'
import {
  scanUrl,
  scanCode,
  fetchScanHistory,
  type VulnScanJob,
  type VulnFinding,
} from '../api/vuln'

export const useVulnStore = defineStore('vuln', () => {
  const scanning = ref(false)
  const currentResult = ref<VulnScanJob | null>(null)
  const history = ref<VulnScanJob[]>([])
  const error = ref('')

  async function runUrlScan(url: string): Promise<VulnScanJob | null> {
    scanning.value = true
    error.value = ''
    currentResult.value = null
    try {
      const job = await scanUrl(url)
      currentResult.value = job
      return job
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
      return null
    } finally {
      scanning.value = false
    }
  }

  async function runCodeScan(path: string): Promise<VulnScanJob | null> {
    scanning.value = true
    error.value = ''
    currentResult.value = null
    try {
      const job = await scanCode(path)
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

  return {
    scanning,
    currentResult,
    history,
    error,
    runUrlScan,
    runCodeScan,
    loadHistory,
    clearResult,
  }
})
