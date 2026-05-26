import { defineStore } from 'pinia'
import { ref } from 'vue'
import { triggerScan, fetchScanHistory, type ScanJob } from '../api/scan'

export const useScanStore = defineStore('scan', () => {
  const scanning = ref(false)
  const history = ref<ScanJob[]>([])
  const total = ref(0)

  async function runScan() {
    scanning.value = true
    try {
      const res = await triggerScan()
      return res.data
    } finally {
      scanning.value = false
    }
  }

  async function loadHistory(params: Record<string, number | string> = {}) {
    const res = await fetchScanHistory(params)
    if (res.success && res.data) {
      history.value = res.data.items
      total.value = res.data.total
    }
  }

  return { scanning, history, total, runScan, loadHistory }
})
