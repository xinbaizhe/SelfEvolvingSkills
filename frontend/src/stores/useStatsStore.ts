import { defineStore } from 'pinia'
import { ref } from 'vue'
import { fetchSummary, fetchTopSkills, fetchSkillsByCategory, type SummaryStats } from '../api/stats'

interface TopSkill {
  name: string
  usage_count: number
}

interface CategoryCount {
  category: string
  count: number
}

export const useStatsStore = defineStore('stats', () => {
  const summary = ref<SummaryStats | null>(null)
  const topSkills = ref<TopSkill[]>([])
  const categoryData = ref<CategoryCount[]>([])
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
      topSkills.value = res.data as TopSkill[]
    }
  }

  async function loadCategoryData() {
    const res = await fetchSkillsByCategory()
    if (res.success && res.data) {
      categoryData.value = res.data as CategoryCount[]
    }
  }

  return { summary, topSkills, categoryData, loading, loadSummary, loadTopSkills, loadCategoryData }
})
