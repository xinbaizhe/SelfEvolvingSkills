<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { fetchAgents, fetchAgentDetail, updateAgent, deleteAgent, evolveAgent, type Agent, type AgentDetail } from '../api/agents'
import { getErrorMessage } from '../utils/error'

const { t } = useI18n()
const router = useRouter()
const agents = ref<Agent[]>([])
const total = ref(0)
const loading = ref(false)
const search = ref('')
const selectedSource = ref<string | null>(null)
const sourceCounts = ref<Record<string, number>>({})
const page = ref(1)
const pageSize = ref(10)

const detailVisible = ref(false)
const currentDetail = ref<AgentDetail | null>(null)
const detailLoading = ref(false)

const editVisible = ref(false)
const editSaving = ref(false)
const editForm = reactive({ name: '', description: '', model: '' })
const editOriginalName = ref('')

const evolving = ref(new Set<number>())

const sources = [
  { id: 'hermes', name: 'Hermes', color: '#6d5bd0', icon: 'H' },
  { id: 'openclaw', name: 'OpenClaw', color: '#7c3aed', icon: 'OC' },
  { id: 'claude-code', name: 'Claude Code', color: '#1473e6', icon: 'CC' },
  { id: 'codex', name: 'Codex', color: '#0f9f7a', icon: 'CX' },
  { id: 'vscode', name: 'VSCode', color: '#d98612', icon: 'CL' },
  { id: 'cursor', name: 'Cursor', color: '#d64f4f', icon: 'CU' },
  { id: 'codebuddy', name: 'CodeBuddy', color: '#6d5bd0', icon: 'CB' },
  { id: 'trae', name: 'TRAE', color: '#1473e6', icon: 'TR' },
  { id: 'zeelinclaw', name: 'ZeeLinClaw', color: '#0f9f7a', icon: 'ZC' },
]

const selectedSourceName = computed(() => {
  return sources.find((source) => source.id === selectedSource.value)?.name || ''
})

async function loadSourceCounts() {
  const entries = await Promise.all(
    sources.map(async (source) => {
      const res = await fetchAgents({ size: 1, agent_source: source.id })
      return [source.id, Number(res.data?.total || 0)] as const
    })
  )
  sourceCounts.value = Object.fromEntries(entries)
}

async function loadAgents(agentSource?: string) {
  loading.value = true
  try {
    const params: Record<string, any> = { page: page.value, size: pageSize.value }
    if (agentSource) params.agent_source = agentSource
    if (search.value) params.search = search.value
    const res = await fetchAgents(params)
    if (res.success && res.data) {
      agents.value = res.data.items
      total.value = res.data.total
    }
  } finally {
    loading.value = false
  }
}

function selectSource(sourceId: string) {
  selectedSource.value = sourceId
  search.value = ''
  page.value = 1
  loadAgents(sourceId)
}

function clearSource() {
  selectedSource.value = null
  search.value = ''
  agents.value = []
  total.value = 0
  loadSourceCounts()
}

async function showDetail(name: string) {
  detailVisible.value = true
  detailLoading.value = true
  currentDetail.value = null
  try {
    const res = await fetchAgentDetail(name)
    if (res.success) currentDetail.value = res.data
  } finally {
    detailLoading.value = false
  }
}

function onSearch() {
  page.value = 1
  if (selectedSource.value) loadAgents(selectedSource.value)
}

function onPageChange(p: number) {
  page.value = p
  if (selectedSource.value) loadAgents(selectedSource.value)
}

function onSizeChange(s: number) {
  pageSize.value = s
  page.value = 1
  if (selectedSource.value) loadAgents(selectedSource.value)
}

// ---- Edit ----
function openEdit(agent: Agent, event: MouseEvent) {
  event.stopPropagation()
  editOriginalName.value = agent.name
  editForm.name = agent.name
  editForm.description = agent.description || ''
  editForm.model = agent.model || ''
  editVisible.value = true
}

async function saveEdit() {
  editSaving.value = true
  try {
    const res = await updateAgent(editOriginalName.value, {
      name: editForm.name || undefined,
      description: editForm.description || null,
      model: editForm.model || null,
    })
    if (!res.success) throw new Error(res.error || t('workbench.common.updateFailed'))
    ElMessage.success(t('workbench.agents.updated'))
    editVisible.value = false
    if (selectedSource.value) await loadAgents(selectedSource.value)
    await loadSourceCounts()
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, t('workbench.common.updateFailed')))
  } finally {
    editSaving.value = false
  }
}

// ---- Delete ----
async function confirmDelete(agent: Agent, event: MouseEvent) {
  event.stopPropagation()
  try {
    await ElMessageBox.confirm(
      t('workbench.agents.deleteConfirm', { name: agent.name }),
      t('workbench.common.deleteConfirmTitle'),
      { confirmButtonText: t('common.delete'), cancelButtonText: t('common.cancel'), type: 'warning' }
    )
    const res = await deleteAgent(agent.name)
    if (!res.success) throw new Error(res.error || t('workbench.common.deleteFailed'))
    ElMessage.success(t('workbench.common.deletedName', { name: res.data?.name || agent.name }))
    if (selectedSource.value) await loadAgents(selectedSource.value)
    await loadSourceCounts()
  } catch (e: unknown) {
    if (e !== 'cancel' && e !== 'close') {
      ElMessage.error(getErrorMessage(e, t('workbench.common.deleteFailed')))
    }
  }
}

// ---- Evolve ----
async function evolveExistingAgent(agent: Agent, event: MouseEvent) {
  event.stopPropagation()
  evolving.value.add(agent.id)
  try {
    const res = await evolveAgent(agent.name)
    if (!res.success) throw new Error(res.error || t('workbench.common.createDraftFailed'))
    ElMessage.success(t('workbench.agents.evolveCreated'))
    router.push('/workbench?tab=drafts')
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, t('workbench.common.createDraftFailed')))
  } finally {
    evolving.value.delete(agent.id)
  }
}

onMounted(loadSourceCounts)
</script>

<template>
  <section class="page-view">
    <div class="page-headline">
      <div>
        <h2>{{ t('workbench.agents.title') }}</h2>
        <p>{{ t('workbench.agents.subtitle') }}</p>
      </div>
      <span v-if="selectedSource" class="count-badge">{{ t('workbench.agents.count', { n: total }) }}</span>
    </div>

    <div v-if="!selectedSource" class="cards">
      <article
        v-for="src in sources"
        :key="src.id"
        class="card source-card"
        @click="selectSource(src.id)"
      >
        <span class="source-count">{{ sourceCounts[src.id] ?? 0 }}</span>
        <div class="source-icon" :style="{ background: src.color }">{{ src.icon }}</div>
        <h3>{{ src.name }}</h3>
        <p>{{ t('workbench.agents.viewAll', { source: src.name }) }}</p>
      </article>
    </div>

    <template v-else>
      <div class="toolbar">
        <el-button size="small" @click="clearSource">{{ t('workbench.agents.backToSources') }}</el-button>
        <span class="toolbar-title">{{ t('workbench.agents.sourceAgents', { source: selectedSourceName }) }}</span>

        <el-input
          v-model="search"
          :placeholder="t('workbench.agents.searchPlaceholder')"
          clearable
          class="search-input"
          @clear="onSearch"
          @keyup.enter="onSearch"
        />
        <el-button type="primary" size="small" @click="onSearch">{{ t('common.search') }}</el-button>
      </div>

      <div v-if="loading" class="empty-state">{{ t('common.loading') }}</div>

      <div v-else-if="agents.length === 0" class="empty-state">{{ t('workbench.agents.empty') }}</div>

      <div v-else class="result-grid">
        <div v-for="agent in agents" :key="agent.id" class="agent-card" @click="showDetail(agent.name)">
          <div class="agent-actions">
            <el-button size="small" type="primary" text :loading="evolving.has(agent.id)" @click="evolveExistingAgent(agent, $event)">
              {{ t('workbench.common.evolve') }}
            </el-button>
            <el-button size="small" text @click="openEdit(agent, $event)">{{ t('common.edit') }}</el-button>
            <el-button size="small" text type="danger" @click="confirmDelete(agent, $event)">{{ t('common.delete') }}</el-button>
          </div>
          <div class="agent-title">
            <span>{{ agent.name }}</span>
            <el-tag v-if="agent.model" size="small">{{ agent.model }}</el-tag>
          </div>
          <div class="item-desc">{{ agent.description || t('workbench.common.noDescription') }}</div>
          <div v-if="agent.tools" class="tag-row">
            <el-tag
              v-for="tool in (Array.isArray(agent.tools) ? agent.tools : [])"
              :key="tool"
              size="small"
              type="info"
            >
              {{ tool }}
            </el-tag>
          </div>
        </div>
      </div>

      <div v-if="total > pageSize" class="pagination-wrap">
        <el-pagination
          v-model:current-page="page"
          v-model:page-size="pageSize"
          :page-sizes="[10, 20, 50]"
          :total="total"
          layout="total, sizes, prev, pager, next"
          size="small"
          @current-change="onPageChange"
          @size-change="onSizeChange"
        />
      </div>
    </template>

    <!-- Detail Dialog -->
    <el-dialog v-model="detailVisible" :title="currentDetail?.name" width="700px" top="5vh">
      <div v-if="detailLoading" class="empty-state">{{ t('common.loading') }}</div>
      <div v-else-if="currentDetail">
        <el-descriptions :column="2" border size="small">
          <el-descriptions-item :label="t('workbench.common.field.name')">{{ currentDetail.name }}</el-descriptions-item>
          <el-descriptions-item :label="t('workbench.agents.model')">{{ currentDetail.model || '-' }}</el-descriptions-item>
          <el-descriptions-item :label="t('workbench.common.field.filePath')" :span="2">{{ currentDetail.file_path }}</el-descriptions-item>
          <el-descriptions-item :label="t('workbench.common.field.description')" :span="2">{{ currentDetail.description || '-' }}</el-descriptions-item>
        </el-descriptions>
        <div class="detail-section">
          <h4>{{ t('workbench.agents.definition') }}</h4>
          <div class="code-preview">{{ currentDetail.body_text || t('workbench.common.noContent') }}</div>
        </div>
      </div>
    </el-dialog>

    <!-- Edit Dialog -->
    <el-dialog v-model="editVisible" :title="t('workbench.agents.editTitle')" width="520px" top="10vh" @closed="editOriginalName = ''">
      <el-form label-position="top">
        <el-form-item :label="t('workbench.common.field.name')">
          <el-input v-model="editForm.name" :placeholder="t('workbench.agents.namePlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('workbench.common.field.description')">
          <el-input v-model="editForm.description" type="textarea" :rows="3" :placeholder="t('workbench.common.descriptionPlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('workbench.agents.model')">
          <el-input v-model="editForm.model" :placeholder="t('workbench.agents.modelPlaceholder')" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="editVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="editSaving" @click="saveEdit">{{ t('common.save') }}</el-button>
      </template>
    </el-dialog>
  </section>
</template>

<style scoped>
.page-headline {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 20px;
}
.page-headline h2 { margin: 0 0 8px; }
.page-headline p { margin: 0; color: var(--muted); font-size: 13px; }
.count-badge, .source-count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 999px;
  background: #e9f8f3;
  color: #0c8265;
  font-size: 12px;
  font-weight: 700;
}
.count-badge { min-width: 92px; padding: 6px 10px; }
.source-count {
  position: absolute;
  top: 12px;
  right: 12px;
  min-width: 30px;
  height: 24px;
  padding: 0 8px;
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}
.toolbar-title { font-weight: 600; }
.search-input { width: 220px; margin-left: auto; }
.result-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 16px;
}
.empty-state { text-align: center; padding: 40px; color: var(--muted); }
.agent-card {
  position: relative;
  background: #fff;
  border-radius: 8px;
  padding: 16px;
  cursor: pointer;
  box-shadow: 0 1px 4px rgba(0,0,0,0.08);
  transition: box-shadow 0.2s;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.agent-card:hover { box-shadow: 0 4px 12px rgba(0,0,0,0.15); }
.agent-actions {
  position: absolute;
  top: 6px;
  right: 6px;
  display: flex;
  gap: 2px;
}
.source-card {
  position: relative;
  cursor: pointer;
  min-height: 152px;
  transition: transform 0.18s ease, box-shadow 0.18s ease, border-color 0.18s ease;
  border: 1px solid #e2e8f0;
  background: linear-gradient(180deg, #ffffff, #fbfdff);
  border-radius: 10px;
  padding: 20px 20px 18px;
  box-shadow: 0 6px 22px rgba(15, 23, 42, 0.06);
  overflow: hidden;
}
.source-card::before {
  content: "";
  position: absolute;
  inset: 0 auto 0 0;
  width: 4px;
  background: linear-gradient(180deg, #14b8a6, #0891b2);
}
.cards {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 18px;
}
.source-card:hover {
  border-color: rgba(13, 148, 136, 0.32);
  transform: translateY(-2px);
  box-shadow: 0 16px 34px rgba(15, 23, 42, 0.1);
}
.source-icon {
  width: 46px;
  height: 46px;
  border-radius: 12px;
  color: #fff;
  display: grid;
  place-items: center;
  font-weight: 700;
  font-size: 15px;
  margin-bottom: 12px;
  box-shadow: 0 10px 18px rgba(15, 23, 42, 0.12);
}
.source-card h3 {
  margin: 0;
  color: #0f172a;
  font-size: 16px;
  font-weight: 800;
}
.source-card p {
  margin: 8px 0 0;
  color: #64748b;
  font-size: 13px;
  line-height: 1.5;
}
.agent-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: bold;
  font-size: 15px;
  padding-right: 120px;
}
.item-desc {
  color: #909399;
  font-size: 13px;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.tag-row { display: flex; gap: 4px; flex-wrap: wrap; margin-top: 8px; }
.pagination-wrap { display: flex; justify-content: center; margin-top: 20px; }
.detail-section { margin-top: 16px; }

@media (max-width: 960px) {
  .result-grid {
    grid-template-columns: repeat(2, 1fr);
  }
  .cards {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (max-width: 640px) {
  .result-grid {
    grid-template-columns: 1fr;
  }
  .cards {
    grid-template-columns: 1fr;
  }
}
.code-preview {
  max-height: 400px;
  overflow-y: auto;
  background: #f8f8f8;
  padding: 12px;
  border-radius: 4px;
  font-size: 13px;
  white-space: pre-wrap;
  font-family: Consolas, monospace;
}
</style>
