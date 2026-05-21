<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { deleteWorkflowDraft, fetchWorkflows, updateWorkflowDraft, type SkillReviewFeedback, type Workflow } from '../api/workflows'

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
    { key: 'safety', title: '安全', items: feedback.safety || [] },
    { key: 'performance', title: '性能', items: feedback.performance || [] },
    { key: 'functionality', title: '功能', items: feedback.functionality || [] },
    { key: 'writing', title: '写法', items: feedback.writing || [] },
    { key: 'improvements', title: '怎么改进', items: feedback.improvements || [] },
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
    const items = Array.isArray(res.data) ? res.data : ((res.data as any)?.items || [])
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
  if (source === 'manual-existing-skill') return '手动进化'
  if (source === 'llm') return '进化管道推荐 · 大模型复核'
  if (source.includes('workflow') || source.includes('local')) return '进化管道推荐'
  if (source.includes('existing')) return '已存在自动进化'
  return '来源未标注'
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
    pending: '待审核',
    edited: '已手动编辑',
    installed: '已安装',
    'manual-draft': '手动草稿',
  }
  return map[status || ''] || status || '待审核'
}

function reviewVerdictLabel(verdict?: string) {
  const map: Record<string, string> = {
    install: '建议安装',
    revise: '建议修改',
    merge: '建议合并',
    discard: '建议丢弃',
  }
  return map[verdict || ''] || verdict || '等待评审'
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
    if (!res.success || !res.data) throw new Error(res.error || '保存失败')
    selectedDraft.value = res.data
    const index = drafts.value.findIndex((draft) => draft.id === res.data!.id)
    if (index >= 0) drafts.value[index] = res.data
    ElMessage.success('草稿已保存')
  } catch (error: any) {
    ElMessage.error(error?.message || '保存失败')
  } finally {
    saving.value = false
  }
}

async function removeDraft() {
  if (!selectedDraft.value) return
  try {
    await ElMessageBox.confirm(`确定删除草稿「${selectedDraft.value.name}」吗？`, '删除草稿', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
    })
    deleting.value = true
    const id = selectedDraft.value.id
    const res = await deleteWorkflowDraft(id)
    if (!res.success) throw new Error(res.error || '删除失败')
    drafts.value = drafts.value.filter((draft) => draft.id !== id)
    selectedDraft.value = drafts.value[0] || null
    ElMessage.success('草稿已删除')
  } catch (error: any) {
    if (error !== 'cancel') ElMessage.error(error?.message || '删除失败')
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
      >手动进化 {{ manualCount }}</el-tag>
      <el-tag
        :class="'filter-tag' + (activeFilter === 'pipeline' ? ' active' : '')"
        effect="plain"
        @click="setFilter('pipeline')"
      >进化管道推荐 {{ pipelineCount }}</el-tag>
      <el-tag
        type="success"
        :class="'filter-tag' + (activeFilter === 'llm' ? ' active' : '')"
        effect="plain"
        @click="setFilter('llm')"
      >大模型复核 {{ llmCount }}</el-tag>
      <el-tag v-if="activeFilter" class="filter-tag" effect="plain" @click="setFilter(null)">显示全部 ({{ drafts.length }})</el-tag>
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
          <p>{{ draft.description || '暂无描述' }}</p>
          <div class="draft-meta">
            <span>{{ statusLabel(draft.status) }}</span>
            <span>推荐 {{ draft.skill_score }}</span>
            <span v-if="draft.review_score != null">评审 {{ draft.review_score }}</span>
          </div>
        </article>
      </div>

      <section v-if="selectedDraft" class="panel draft-editor">
        <div class="head">
          <div>
            <h2>Skill 草稿：{{ selectedDraft.name }}</h2>
            <p>{{ selectedDraft.description }}</p>
          </div>
          <div class="actions">
            <el-tag :type="sourceType(selectedDraft)">{{ sourceLabel(selectedDraft) }}</el-tag>
            <el-button size="small" :loading="saving" @click="saveDraft">保存</el-button>
            <el-button size="small" type="danger" :loading="deleting" @click="removeDraft">删除</el-button>
          </div>
        </div>

        <div class="body draft-review-grid">
          <div class="draft-pane">
            <div class="pane-title">
              <h3>自己的草稿</h3>
              <span>可直接编辑 SKILL.md</span>
            </div>
            <el-input
              v-model="editBody"
              type="textarea"
              :rows="24"
              resize="vertical"
              placeholder="编辑 SKILL.md 草稿内容"
            />
            <div v-if="selectedSampleTasks.length" class="source-tasks">
              <p>来源任务</p>
              <ul>
                <li v-for="task in selectedSampleTasks" :key="task">{{ task }}</li>
              </ul>
            </div>
          </div>

          <aside class="review-pane">
            <div class="pane-title">
              <h3>改进意见</h3>
              <span>Skill Review Agent</span>
            </div>
            <div v-if="selectedDraft.review_score != null || reviewFeedback" class="review-summary">
              <el-tag :type="reviewScoreType(selectedDraft.review_score)">
                {{ selectedDraft.review_score ?? '-' }} 分
              </el-tag>
              <el-tag type="info">{{ reviewVerdictLabel(reviewFeedback?.verdict) }}</el-tag>
              <p>{{ selectedDraft.review_summary || '暂无总结' }}</p>
            </div>
            <div v-if="reviewFeedback" class="review-sections">
              <section v-for="section in reviewSections" :key="section.key" class="review-section">
                <h4>{{ section.title }}</h4>
                <ul v-if="section.items.length">
                  <li v-for="item in section.items" :key="item">{{ item }}</li>
                </ul>
                <p v-else>暂无明显问题。</p>
              </section>
            </div>
            <el-empty v-else description="暂无评审意见。运行进化管道并启用大模型后会自动生成。" />
          </aside>
        </div>
      </section>
    </div>

    <div v-else-if="activeFilter" class="panel">
      <div class="body empty-state">
        <p>该筛选条件下暂无草稿。</p>
        <el-button link type="primary" @click="setFilter(null)">显示全部</el-button>
      </div>
    </div>

    <div v-else class="panel">
      <div class="body empty-state">
        暂无 Skill 草稿。可以在"进化管道"启动自动流程，也可以到"已存在 Skills"中点击单个 Skill 右上角的"进化"手动创建草稿。
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
