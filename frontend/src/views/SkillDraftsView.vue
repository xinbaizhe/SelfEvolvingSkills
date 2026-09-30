<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { deleteWorkflowDraft, fetchWorkflows, updateWorkflowDraft, type SkillReviewFeedback, type Workflow } from '../api/workflows'
import { getErrorMessage } from '../utils/error'
import type { PaginatedResult } from '../api/skills'

const { t } = useI18n()

const drafts = ref<Workflow[]>([])
const loading = ref(false)
const saving = ref(false)
const deleting = ref(false)
const selectedDraft = ref<Workflow | null>(null)
const editBody = ref('')
const activeFilter = ref<string | null>(null)

const manualCount = computed(() => drafts.value.filter((w) => w.recommendation_source === 'manual-existing-skill' || w.status === 'manual-draft').length)
const pipelineCount = computed(() => drafts.value.filter((w) => w.recommendation_source !== 'manual-existing-skill' && w.recommendation_source !== 'llm' && w.status !== 'manual-draft').length)
const llmCount = computed(() => drafts.value.filter((w) => w.recommendation_source === 'llm').length)

const filteredDrafts = computed(() => {
  if (activeFilter.value === 'manual') return drafts.value.filter((w) => w.recommendation_source === 'manual-existing-skill' || w.status === 'manual-draft')
  if (activeFilter.value === 'pipeline') return drafts.value.filter((w) => w.recommendation_source !== 'manual-existing-skill' && w.recommendation_source !== 'llm' && w.status !== 'manual-draft')
  if (activeFilter.value === 'llm') return drafts.value.filter((w) => w.recommendation_source === 'llm')
  return drafts.value
})

function setFilter(key: string | null) {
  activeFilter.value = activeFilter.value === key ? null : key
}

const selectedSampleTasks = computed(() => parseArray(selectedDraft.value?.sample_tasks))
const reviewFeedback = computed<SkillReviewFeedback | null>(() => selectedDraft.value?.review_feedback || null)

const reviewSections = computed(() => {
  const feedback = reviewFeedback.value || {}
  return [
    { key: 'safety', titleKey: 'workbench.common.review.safety', items: feedback.safety || [] },
    { key: 'performance', titleKey: 'workbench.common.review.performance', items: feedback.performance || [] },
    { key: 'functionality', titleKey: 'workbench.common.review.functionality', items: feedback.functionality || [] },
    { key: 'writing', titleKey: 'workbench.common.review.writing', items: feedback.writing || [] },
    { key: 'improvements', titleKey: 'workbench.common.review.improvements', items: feedback.improvements || [] },
  ]
})

function parseArray(raw: unknown): string[] {
  if (!raw) return []
  if (Array.isArray(raw)) return raw.map(String)
  try {
    const parsed = JSON.parse(String(raw))
    return Array.isArray(parsed) ? parsed.map(String) : []
  } catch {
    return []
  }
}

async function load() {
  loading.value = true
  try {
    const res = await fetchWorkflows()
    const items = Array.isArray(res.data) ? res.data : ((res.data as unknown as PaginatedResult<Workflow>)?.items || [])
    drafts.value = items.filter((workflow: Workflow) => workflow.draft_body)
    const currentId = selectedDraft.value?.id
    selectedDraft.value = drafts.value.find((draft) => draft.id === currentId) || drafts.value[0] || null
  } finally {
    loading.value = false
  }
}

function viewDraft(draft: Workflow) {
  selectedDraft.value = draft
}

function sourceLabel(draft: Workflow) {
  const source = draft.recommendation_source || ''
  if (source === 'manual-existing-skill') return t('workbench.drafts.sourceManual')
  if (source === 'llm') return t('workbench.drafts.sourceLlm')
  if (source.includes('workflow') || source.includes('local')) return t('workbench.drafts.sourcePipeline')
  if (source.includes('existing')) return t('workbench.drafts.sourceExisting')
  return t('workbench.drafts.sourceUnknown')
}

function sourceType(draft: Workflow) {
  const source = draft.recommendation_source || ''
  if (source === 'manual-existing-skill') return 'warning'
  if (source === 'llm') return 'success'
  if (source.includes('workflow') || source.includes('local')) return 'primary'
  return 'info'
}

function statusLabel(status?: string | null) {
  const map: Record<string, string> = {
    pending: t('workbench.drafts.statusPending'),
    edited: t('workbench.drafts.statusEdited'),
    installed: t('workbench.drafts.statusInstalled'),
    'manual-draft': t('workbench.drafts.statusManualDraft'),
  }
  return map[status || ''] || status || t('workbench.drafts.statusPending')
}

function reviewVerdictLabel(verdict?: string) {
  const map: Record<string, string> = {
    install: t('workbench.common.review.verdictInstall'),
    revise: t('workbench.common.review.verdictRevise'),
    merge: t('workbench.common.review.verdictMerge'),
    discard: t('workbench.common.review.verdictDiscard'),
  }
  return map[verdict || ''] || verdict || t('workbench.common.review.verdictPending')
}

function reviewScoreType(score?: number | null) {
  if (score == null) return 'info'
  if (score >= 85) return 'success'
  if (score >= 70) return 'warning'
  return 'danger'
}

async function saveDraft() {
  if (!selectedDraft.value) return
  saving.value = true
  try {
    const res = await updateWorkflowDraft(selectedDraft.value.id, {
      draft_body: editBody.value,
      description: selectedDraft.value.description,
    })
    if (!res.success || !res.data) throw new Error(res.error || t('workbench.common.saveFailed'))
    selectedDraft.value = res.data
    const index = drafts.value.findIndex((draft) => draft.id === res.data!.id)
    if (index >= 0) drafts.value[index] = res.data
    ElMessage.success(t('workbench.drafts.saved'))
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, t('workbench.common.saveFailed')))
  } finally {
    saving.value = false
  }
}

async function removeDraft() {
  if (!selectedDraft.value) return
  try {
    await ElMessageBox.confirm(t('workbench.drafts.deleteConfirm', { name: selectedDraft.value.name }), t('workbench.drafts.deleteTitle'), {
      confirmButtonText: t('common.delete'),
      cancelButtonText: t('common.cancel'),
      type: 'warning',
    })
    deleting.value = true
    const id = selectedDraft.value.id
    const res = await deleteWorkflowDraft(id)
    if (!res.success) throw new Error(res.error || t('workbench.common.deleteFailed'))
    drafts.value = drafts.value.filter((draft) => draft.id !== id)
    selectedDraft.value = drafts.value[0] || null
    ElMessage.success(t('workbench.drafts.deleted'))
  } catch (e: unknown) {
    if (e !== 'cancel') ElMessage.error(getErrorMessage(e, t('workbench.common.deleteFailed')))
  } finally {
    deleting.value = false
  }
}

watch(selectedDraft, (draft) => {
  editBody.value = draft?.draft_body || ''
}, { immediate: true })

onMounted(load)
</script>

<template>
  <section class="page-view" v-loading="loading">
    <div class="legend">
      <el-tag
        :type="activeFilter === 'manual' ? 'warning' : 'warning'"
        :class="'filter-tag' + (activeFilter === 'manual' ? ' active' : '')"
        effect="plain"
        @click="setFilter('manual')"
      >{{ t('workbench.common.filterManual', { n: manualCount }) }}</el-tag>
      <el-tag
        :class="'filter-tag' + (activeFilter === 'pipeline' ? ' active' : '')"
        effect="plain"
        @click="setFilter('pipeline')"
      >{{ t('workbench.common.filterPipeline', { n: pipelineCount }) }}</el-tag>
      <el-tag
        type="success"
        :class="'filter-tag' + (activeFilter === 'llm' ? ' active' : '')"
        effect="plain"
        @click="setFilter('llm')"
      >{{ t('workbench.common.filterLlm', { n: llmCount }) }}</el-tag>
      <el-tag v-if="activeFilter" class="filter-tag" effect="plain" @click="setFilter(null)">{{ t('workbench.common.showAllCount', { n: drafts.length }) }}</el-tag>
    </div>

    <div v-if="filteredDrafts.length > 0" class="draft-layout">
      <div class="draft-list">
        <article
          v-for="draft in filteredDrafts"
          :key="draft.id"
          :class="['draft-card', { active: selectedDraft?.id === draft.id }]"
          @click="viewDraft(draft)"
        >
          <div class="draft-card__head">
            <h3>{{ draft.name }}</h3>
            <el-tag size="small" :type="sourceType(draft)">{{ sourceLabel(draft) }}</el-tag>
          </div>
          <p>{{ draft.description || t('workbench.common.noDescription') }}</p>
          <div class="draft-meta">
            <span>{{ statusLabel(draft.status) }}</span>
            <span>{{ t('workbench.drafts.recommendScore', { score: draft.skill_score }) }}</span>
            <span v-if="draft.review_score != null">{{ t('workbench.drafts.reviewScore', { score: draft.review_score }) }}</span>
          </div>
        </article>
      </div>

      <section v-if="selectedDraft" class="panel draft-editor">
        <div class="head">
          <div>
            <h2>{{ t('workbench.drafts.editorTitle', { name: selectedDraft.name }) }}</h2>
            <p>{{ selectedDraft.description }}</p>
          </div>
          <div class="actions">
            <el-tag :type="sourceType(selectedDraft)">{{ sourceLabel(selectedDraft) }}</el-tag>
            <el-button size="small" :loading="saving" @click="saveDraft">{{ t('common.save') }}</el-button>
            <el-button size="small" type="danger" :loading="deleting" @click="removeDraft">{{ t('common.delete') }}</el-button>
          </div>
        </div>

        <div class="body draft-review-grid">
          <div class="draft-pane">
            <div class="pane-title">
              <h3>{{ t('workbench.drafts.ownDraft') }}</h3>
              <span>{{ t('workbench.drafts.editHint') }}</span>
            </div>
            <el-input
              v-model="editBody"
              type="textarea"
              :rows="24"
              resize="vertical"
              :placeholder="t('workbench.drafts.editPlaceholder')"
            />
            <div v-if="selectedSampleTasks.length" class="source-tasks">
              <p>{{ t('workbench.drafts.sourceTasks') }}</p>
              <ul>
                <li v-for="task in selectedSampleTasks" :key="task">{{ task }}</li>
              </ul>
            </div>
          </div>

          <aside class="review-pane">
            <div class="pane-title">
              <h3>{{ t('workbench.common.review.title') }}</h3>
              <span>Skill Review Agent</span>
            </div>
            <div v-if="selectedDraft.review_score != null || reviewFeedback" class="review-summary">
              <el-tag :type="reviewScoreType(selectedDraft.review_score)">
                {{ t('workbench.common.review.score', { score: selectedDraft.review_score ?? '-' }) }}
              </el-tag>
              <el-tag type="info">{{ reviewVerdictLabel(reviewFeedback?.verdict) }}</el-tag>
              <p>{{ selectedDraft.review_summary || t('workbench.common.review.summaryEmpty') }}</p>
            </div>
            <div v-if="reviewFeedback" class="review-sections">
              <section v-for="section in reviewSections" :key="section.key" class="review-section">
                <h4>{{ t(section.titleKey) }}</h4>
                <ul v-if="section.items.length">
                  <li v-for="item in section.items" :key="item">{{ item }}</li>
                </ul>
                <p v-else>{{ t('workbench.common.review.noIssues') }}</p>
              </section>
            </div>
            <el-empty v-else :description="t('workbench.common.review.emptyDescription')" />
          </aside>
        </div>
      </section>
    </div>

    <div v-else-if="activeFilter" class="panel">
      <div class="body empty-state">
        <p>{{ t('workbench.drafts.emptyFiltered') }}</p>
        <el-button link type="primary" @click="setFilter(null)">{{ t('workbench.common.showAll') }}</el-button>
      </div>
    </div>

    <div v-else class="panel">
      <div class="body empty-state">
        {{ t('workbench.drafts.empty') }}
      </div>
    </div>
  </section>
</template>

<style scoped>
.legend {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 14px;
  color: var(--muted);
  font-size: 12px;
}

.filter-tag {
  cursor: pointer;
  user-select: none;
  transition: transform 0.15s, box-shadow 0.15s;
}
.filter-tag:hover {
  transform: translateY(-1px);
  box-shadow: 0 2px 8px rgba(0,0,0,0.12);
}
.filter-tag.active {
  transform: translateY(-1px);
  box-shadow: 0 0 0 2px var(--el-color-primary), 0 2px 8px rgba(0,0,0,0.15);
}

.draft-layout {
  display: grid;
  grid-template-columns: minmax(260px, 360px) 1fr;
  gap: 16px;
}

.draft-list {
  display: grid;
  gap: 10px;
  align-content: start;
}

.draft-card {
  border: 1px solid var(--line);
  border-radius: 8px;
  background: #fff;
  padding: 12px;
  cursor: pointer;
}

.draft-card.active {
  border-color: var(--blue);
  box-shadow: 0 8px 22px rgba(20, 115, 230, 0.12);
}

.draft-card__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
}

.draft-card h3 {
  margin: 0;
  font-size: 14px;
}

.draft-card p {
  margin: 8px 0;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.5;
}

.draft-meta {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  color: var(--muted);
  font-size: 12px;
}

.draft-editor .head {
  align-items: flex-start;
}

.actions {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
}

.draft-review-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(280px, 380px);
  gap: 16px;
  align-items: start;
}

.draft-pane,
.review-pane {
  min-width: 0;
}

.pane-title {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
}

.pane-title h3 {
  margin: 0;
  font-size: 15px;
}

.pane-title span {
  color: var(--muted);
  font-size: 12px;
}

.review-pane {
  border-left: 1px solid var(--line);
  padding-left: 16px;
}

.review-summary {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 14px;
}

.review-summary p {
  flex-basis: 100%;
  margin: 2px 0 0;
  color: #334155;
  font-size: 13px;
  line-height: 1.6;
}

.review-sections {
  display: grid;
  gap: 12px;
}

.review-section {
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 10px;
  background: #fff;
}

.review-section h4 {
  margin: 0 0 8px;
  font-size: 13px;
}

.review-section ul {
  margin: 0;
  padding-left: 18px;
  color: #475569;
  font-size: 12px;
  line-height: 1.6;
}

.review-section p {
  margin: 0;
  color: var(--muted);
  font-size: 12px;
}

.source-tasks {
  margin-top: 12px;
  color: var(--muted);
  font-size: 12px;
}

.source-tasks p {
  margin: 0 0 8px;
  font-weight: 600;
  color: #334155;
}

.source-tasks ul {
  margin: 0;
  padding-left: 18px;
}

.empty-state {
  text-align: center;
  padding: 40px;
  color: var(--muted);
  line-height: 1.7;
}

@media (max-width: 1180px) {
  .draft-layout,
  .draft-review-grid {
    grid-template-columns: 1fr;
  }

  .review-pane {
    border-left: 0;
    border-top: 1px solid var(--line);
    padding-left: 0;
    padding-top: 16px;
  }
}
</style>
