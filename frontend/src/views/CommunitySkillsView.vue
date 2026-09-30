<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { searchCommunitySkills, type CommunitySkill, type PaginatedResult } from '../api/community'
import { getErrorMessage } from '../utils/error'
import { useBackendText } from '../composables/useBackendText'

const { t } = useI18n()
const backendText = useBackendText()
const query = ref('')
const skills = ref<CommunitySkill[]>([])
const total = ref(0)
const page = ref(1)
const perPage = 10
const loading = ref(false)
const refreshTimer = ref<number | null>(null)
const lastUpdatedAt = ref<string | null>(null)

const recommendedSkills = computed(() => skills.value)

async function search(showErrors = true) {
  loading.value = true
  try {
    const res = await searchCommunitySkills(query.value || 'AI coding agent skills', page.value, perPage)
    if (res.success) {
      skills.value = (res.data as PaginatedResult<CommunitySkill>).items ?? []
      total.value = (res.data as PaginatedResult<CommunitySkill>).total ?? 0
      lastUpdatedAt.value = formatDateTime(new Date())
    } else if (showErrors) {
      ElMessage.error(res.error || t('workbench.community.recommendFailed'))
    }
  } catch (e: unknown) {
    if (showErrors) {
      ElMessage.error(getErrorMessage(e, t('workbench.community.recommendFailedHint')))
    }
  } finally {
    loading.value = false
  }
}

function formatStars(value: number) {
  if (value >= 1000) return `${(value / 1000).toFixed(1)}k`
  return value.toString()
}

function formatScore(value?: number) {
  if (value == null) return '-'
  return Math.round(value * 100).toString()
}

function formatDateTime(value: Date) {
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${value.getFullYear()}-${pad(value.getMonth() + 1)}-${pad(value.getDate())} ${pad(value.getHours())}:${pad(value.getMinutes())}:${pad(value.getSeconds())}`
}

function sourceType(source?: string) {
  return source === 'LLM' ? 'success' : 'info'
}

function hasRepositoryUpdatedAt(skill: CommunitySkill) {
  return skill.source === 'GitHub' && Boolean(skill.pushed_at)
}

function scheduleFollowupRefresh() {
  window.setTimeout(() => search(false), 3000)
}

function showReason(skill: CommunitySkill) {
  const body = (skill.recommendation_reason ? backendText(skill.recommendation_reason) : t('workbench.community.noReason'))
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => `<p>${escapeHtml(line)}</p>`)
    .join('')
  ElMessageBox.alert(
    body,
    t('workbench.community.reasonTitle', { name: backendText(skill.name) }),
    {
      confirmButtonText: t('workbench.community.gotIt'),
      dangerouslyUseHTMLString: true,
    },
  )
}

function escapeHtml(value: string) {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;')
}

function handlePageChange(nextPage: number) {
  page.value = nextPage
  search()
  scheduleFollowupRefresh()
}

onMounted(() => {
  search()
  scheduleFollowupRefresh()
  refreshTimer.value = window.setInterval(() => {
    search(false)
    scheduleFollowupRefresh()
  }, 5 * 60 * 1000)
})

onUnmounted(() => {
  if (refreshTimer.value != null) window.clearInterval(refreshTimer.value)
})
</script>

<template>
  <div class="community-skills">
    <div class="page-header">
      <div>
        <h2>{{ t('workbench.community.title') }}</h2>
        <p class="subtitle">
          {{ t('workbench.community.subtitle') }}
        </p>
      </div>
      <span v-if="lastUpdatedAt" class="updated">{{ t('workbench.community.updatedAt', { time: lastUpdatedAt }) }}</span>
    </div>

    <div class="toolbar">
      <el-input
        v-model="query"
        class="search-input"
        :placeholder="t('workbench.community.searchPlaceholder')"
        clearable
        @keyup.enter="search()"
      />
      <el-button type="primary" :loading="loading" @click="search().then(scheduleFollowupRefresh)">{{ t('workbench.community.refreshSuggestions') }}</el-button>
    </div>

    <div v-loading="loading" class="skills-grid">
      <article
        v-for="skill in recommendedSkills"
        :key="skill.id"
        class="skill-card"
      >
        <div class="card-header">
          <h4 class="skill-name">{{ backendText(skill.name) }}</h4>
          <el-tag :type="sourceType(skill.source)" size="small">{{ skill.source || 'GitHub' }}</el-tag>
        </div>

        <p class="skill-desc">{{ skill.description ? backendText(skill.description) : t('workbench.common.noDescription') }}</p>

        <div class="card-meta">
          <span class="repo">{{ skill.repo }}</span>
          <span class="stars">★ {{ formatStars(skill.stars || 0) }}</span>
          <span v-if="skill.license">License {{ skill.license }}</span>
        </div>

        <div class="score-line">
          <span>{{ t('workbench.community.weighted', { score: formatScore(skill.weighted_score) }) }}</span>
          <span>{{ t('workbench.community.relevance', { score: formatScore(skill.relevance_score) }) }}</span>
          <span>{{ t('workbench.community.quality', { score: formatScore(skill.quality_score) }) }}</span>
        </div>

        <div class="source-line">
          <span v-if="skill.fetched_at">{{ t('workbench.community.fetchedAt', { time: skill.fetched_at }) }}</span>
          <span v-if="hasRepositoryUpdatedAt(skill)">{{ t('workbench.community.repoUpdated', { time: skill.pushed_at }) }}</span>
        </div>

        <div class="card-actions">
          <a :href="skill.repo_url" target="_blank" class="repo-link" rel="noreferrer">
            <el-button size="small" text>{{ t('workbench.community.viewSource') }}</el-button>
          </a>
          <el-button size="small" text type="primary" @click="showReason(skill)">{{ t('workbench.community.reason') }}</el-button>
        </div>
      </article>

      <div v-if="!loading && recommendedSkills.length === 0" class="empty-state">
        <p>{{ t('workbench.community.empty') }}</p>
        <p class="hint">{{ t('workbench.community.emptyHint') }}</p>
      </div>
    </div>

    <div v-if="total > perPage" class="pagination-row">
      <el-pagination
        background
        layout="prev, pager, next"
        :current-page="page"
        :page-size="perPage"
        :total="total"
        @current-change="handlePageChange"
      />
    </div>
  </div>
</template>

<style scoped>
.community-skills {
  max-width: 1200px;
  margin: 0 auto;
  padding: 24px;
}

.page-header {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  align-items: flex-start;
  margin-bottom: 20px;
}

.page-header h2 {
  margin: 0;
  font-size: 22px;
  font-weight: 700;
  color: var(--el-text-color-primary);
}

.subtitle,
.updated {
  margin: 6px 0 0;
  font-size: 13px;
  color: var(--el-text-color-secondary);
}

.updated {
  white-space: nowrap;
}

.toolbar {
  display: flex;
  gap: 12px;
  align-items: center;
  margin-bottom: 18px;
}

.search-input {
  max-width: 520px;
}

.skills-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 16px;
  min-height: 200px;
}

.skill-card {
  padding: 18px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  background: var(--el-bg-color);
}

.card-header,
.card-meta,
.card-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.card-meta,
.score-line,
.source-line {
  flex-wrap: wrap;
  justify-content: flex-start;
}

.skill-name {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  overflow-wrap: anywhere;
}

.skill-desc {
  min-height: 40px;
  margin: 12px 0;
  font-size: 13px;
  line-height: 1.5;
  color: var(--el-text-color-secondary);
}

.card-meta,
.score-line,
.source-line {
  display: flex;
  gap: 10px;
  margin-bottom: 12px;
  font-size: 12px;
  color: var(--el-text-color-placeholder);
}

.repo {
  overflow-wrap: anywhere;
}

.stars {
  color: #b7791f;
  font-weight: 600;
}

.repo-link {
  text-decoration: none;
}

.empty-state {
  grid-column: 1 / -1;
  padding: 56px 20px;
  text-align: center;
  color: var(--el-text-color-secondary);
}

.empty-state p {
  margin: 0;
  font-size: 16px;
}

.empty-state .hint {
  margin-top: 8px;
  font-size: 13px;
  color: var(--el-text-color-placeholder);
}

.pagination-row {
  display: flex;
  justify-content: center;
  margin-top: 18px;
}
</style>
