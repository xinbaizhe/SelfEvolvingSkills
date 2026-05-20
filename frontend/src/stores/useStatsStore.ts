import { defineStore } from 'pinia'
import { ref } from 'vue'
import { fetchSummary, fetchTopSkills, fetchSkillsByCategory, fetchSessions } from '../api/stats'

export const useStatsStore = defineStore('stats', () => {
  const summary = ref<any>(null)
  const topSkills = ref<any[]>([])
  const categoryData = ref<any[]>([])
  const loading = ref(false)

  async function loadSummary() {
    loading.value = true
    try {
      const res = await fetchSummary()
      summary.value = res.data
    } finally {
      loading.value = false
    }
  }

  async function loadTopSkills(limit = 20) {
    const res = await fetchTopSkills(limit)
    if (res.success && res.data) {
      topSkills.value = res.data as any[]
    }
  }

  async function loadCategoryData() {
    const res = await fetchSkillsByCategory()
    if (res.success && res.data) {
      categoryData.value = res.data as any[]
    }
  }

  return { summary, topSkills, categoryData, loading, loadSummary, loadTopSkills, loadCategoryData }
})
