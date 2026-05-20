import { defineStore } from 'pinia'
import { ref } from 'vue'
import { fetchSkills, fetchSkillDetail, fetchCategories, type SkillItem, type SkillDetail } from '../api/skills'

export const useSkillStore = defineStore('skill', () => {
  const skills = ref<SkillItem[]>([])
  const total = ref(0)
  const loading = ref(false)
  const currentDetail = ref<SkillDetail | null>(null)
  const categories = ref<{ category: string; count: number }[]>([])

  async function loadSkills(params: Record<string, any> = {}) {
    loading.value = true
    try {
      const res = await fetchSkills(params)
      if (res.success && res.data) {
        skills.value = res.data.items
        total.value = res.data.total
      }
    } finally {
      loading.value = false
    }
  }

  async function loadDetail(name: string) {
    const res = await fetchSkillDetail(name)
    if (res.success) {
      currentDetail.value = res.data
    }
  }

  async function loadCategories() {
    const res = await fetchCategories()
    if (res.success && res.data) {
      categories.value = res.data
    }
  }

  return { skills, total, loading, currentDetail, categories, loadSkills, loadDetail, loadCategories }
})
