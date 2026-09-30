<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { evolveSkill, fetchSkills, fetchSkillDetail, importSkills, deleteSkill, type SkillItem, type SkillDetail } from '../api/skills'
import { getErrorMessage } from '../utils/error'
import { exportSkills } from '../api/export'
import { onSkillsChanged } from '../composables/useSkillEvents'
import SkillDetailDialog from '../components/skill/SkillDetailDialog.vue'
import SkillEditDialog from '../components/skill/SkillEditDialog.vue'

const { t } = useI18n()
const router = useRouter()
const skills = ref<SkillItem[]>([])
const total = ref(0)
const page = ref(1)
const size = ref(10)
const loading = ref(false)
const search = ref('')
const selectedSource = ref<string | null>(null)
const sourceCounts = ref<Record<string, number>>({})
const importing = ref(false)
const exporting = ref(false)
const evolving = ref(new Set<number>())
const fileInput = ref<HTMLInputElement | null>(null)

const detailVisible = ref(false)
const detailLoading = ref(false)
const currentDetail = ref<SkillDetail | null>(null)

const editVisible = ref(false)
const editSkill = ref<SkillItem | null>(null)
const editDialogRef = ref<InstanceType<typeof SkillEditDialog> | null>(null)

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
      const res = await fetchSkills({ size: 1, agent_source: source.id })
      return [source.id, Number(res.data?.total || 0)] as const
    })
  )
  sourceCounts.value = Object.fromEntries(entries)
}

async function loadSkills(agentSource?: string) {
  loading.value = true
  try {
    const params: Record<string, any> = { page: page.value, size: size.value }
    if (agentSource) params.agent_source = agentSource
    if (search.value) params.search = search.value
    const res = await fetchSkills(params)
    skills.value = res.data?.items || []
    total.value = res.data?.total || 0
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, t('workbench.skills.loadFailed')))
  } finally {
    loading.value = false
  }
}

function onPageChange(p: number) {
  page.value = p
  if (selectedSource.value) loadSkills(selectedSource.value)
}

function onSizeChange(s: number) {
  size.value = s
  page.value = 1
  if (selectedSource.value) loadSkills(selectedSource.value)
}

function selectSource(sourceId: string) {
  selectedSource.value = sourceId
  search.value = ''
  page.value = 1
  loadSkills(sourceId)
}

function clearSource() {
  selectedSource.value = null
  search.value = ''
  page.value = 1
  skills.value = []
  total.value = 0
  loadSourceCounts()
}

async function showDetail(name: string, sourceType?: string) {
  detailVisible.value = true
  detailLoading.value = true
  currentDetail.value = null
  try {
    const res = await fetchSkillDetail(name, sourceType)
    if (res.success) currentDetail.value = res.data
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, t('workbench.skills.detailFailed')))
  } finally {
    detailLoading.value = false
  }
}

function openEdit(skill: SkillItem, event: MouseEvent) {
  event.stopPropagation()
  editSkill.value = skill
  editVisible.value = true
  editDialogRef.value?.open(skill)
}

async function onEditSaved() {
  if (selectedSource.value) await loadSkills(selectedSource.value)
  await loadSourceCounts()
}

async function confirmDelete(skill: SkillItem, event: MouseEvent) {
  event.stopPropagation()
  try {
    await ElMessageBox.confirm(
      t('workbench.skills.deleteConfirm', { name: skill.name }),
      t('workbench.common.deleteConfirmTitle'),
      { confirmButtonText: t('common.delete'), cancelButtonText: t('common.cancel'), type: 'warning' }
    )
    const res = await deleteSkill(skill.name)
    if (!res.success) throw new Error(res.error || t('workbench.common.deleteFailed'))
    ElMessage.success(t('workbench.common.deletedName', { name: res.data?.name || skill.name }))
    if (selectedSource.value) {
      const currentPage = page.value
      const shouldGoBack = skills.value.length === 1 && currentPage > 1
      if (shouldGoBack) page.value = currentPage - 1
      await loadSkills(selectedSource.value)
    }
    await loadSourceCounts()
  } catch (e: unknown) {
    if (e !== 'cancel' && e !== 'close') {
      ElMessage.error(getErrorMessage(e, t('workbench.common.deleteFailed')))
    }
  }
}

async function evolveExistingSkill(skill: SkillItem, event: MouseEvent) {
  event.stopPropagation()
  evolving.value.add(skill.id)
  try {
    const res = await evolveSkill(skill.name)
    if (!res.success) throw new Error(res.error || t('workbench.common.createDraftFailed'))
    ElMessage.success(t('workbench.skills.evolveCreated'))
    router.push('/workbench?tab=drafts')
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, t('workbench.common.createDraftFailed')))
  } finally {
    evolving.value.delete(skill.id)
  }
}

function downloadBlob(content: string, filename: string, type: string) {
  const blob = new Blob([content], { type })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

async function exportCurrentSkills(format: 'json' | 'csv') {
  if (!selectedSource.value) return
  exporting.value = true
  try {
    const data = await exportSkills(format, selectedSource.value)
    downloadBlob(data.content, data.filename, data.content_type)
    ElMessage.success(t('workbench.skills.exportSuccess', { source: selectedSourceName.value }))
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, t('workbench.skills.exportFailed')))
  } finally {
    exporting.value = false
  }
}

function openImportPicker() {
  if (!selectedSource.value) return
  fileInput.value?.click()
}

async function onImportFile(event: Event) {
  if (!selectedSource.value) return
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return

  importing.value = true
  try {
    const isZip = file.name.toLowerCase().endsWith('.zip')
    const content = isZip ? '' : await file.text()
    const contentBase64 = isZip ? await fileToBase64(file) : undefined
    const res = await importSkills(selectedSource.value, file.name, content, contentBase64)
    if (!res.success || !res.data) throw new Error(res.error || t('workbench.skills.importFailed'))
    const skipped = res.data.skipped?.length || 0
    ElMessage.success(t('workbench.skills.importSuccess', { count: res.data.count, agent: res.data.agent_name }) + (skipped ? t('workbench.skills.importSkipped', { n: skipped }) : ''))
    await loadSkills(selectedSource.value)
    await loadSourceCounts()
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, t('workbench.skills.importFailedHint')))
  } finally {
    importing.value = false
  }
}

function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      const value = String(reader.result || '')
      resolve(value.includes(',') ? value.split(',')[1] : value)
    }
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(file)
  })
}

function onSearch() {
  page.value = 1
  if (selectedSource.value) loadSkills(selectedSource.value)
}

let cleanupSkillsListener: (() => void) | null = null

onMounted(() => {
  loadSourceCounts()
  cleanupSkillsListener = onSkillsChanged(() => {
    loadSourceCounts()
    if (selectedSource.value) loadSkills(selectedSource.value)
  })
})

onUnmounted(() => {
  cleanupSkillsListener?.()
})
</script>

<template>
  <section class="page-view">
    <div class="page-headline">
      <div>
        <h2>{{ t('workbench.skills.title') }}</h2>
        <p>{{ t('workbench.skills.subtitle') }}</p>
      </div>
      <span v-if="selectedSource" class="count-badge">{{ t('workbench.skills.count', { n: total }) }}</span>
    </div>

    <div v-if="!selectedSource" class="cards">
      <article v-for="src in sources" :key="src.id" class="card source-card" @click="selectSource(src.id)">
        <span class="source-count">{{ sourceCounts[src.id] ?? 0 }}</span>
        <div class="source-icon" :style="{ background: src.color }">{{ src.icon }}</div>
        <h3>{{ src.name }}</h3>
        <p>{{ t('workbench.skills.viewSource', { source: src.name }) }}</p>
      </article>
    </div>

    <template v-else>
      <div class="toolbar">
        <el-button size="small" @click="clearSource">{{ t('workbench.skills.backToAgents') }}</el-button>
        <span class="toolbar-title">{{ t('workbench.skills.sourceSkills', { source: selectedSourceName }) }}</span>

        <input
          ref="fileInput"
          class="hidden-input"
          type="file"
          accept=".md,.json,.zip,application/json,text/markdown,text/plain,application/zip"
          @change="onImportFile"
        />
        <el-button size="small" :loading="importing" @click="openImportPicker">{{ t('workbench.skills.import') }}</el-button>
        <el-dropdown trigger="click" @command="exportCurrentSkills">
          <el-button size="small" :loading="exporting">{{ t('workbench.skills.export') }}</el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="json">{{ t('workbench.skills.exportJson') }}</el-dropdown-item>
              <el-dropdown-item command="csv">{{ t('workbench.skills.exportCsv') }}</el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>

        <el-input
          v-model="search"
          :placeholder="t('workbench.skills.searchPlaceholder')"
          clearable
          class="search-input"
          @clear="onSearch"
          @keyup.enter="onSearch"
        />
        <el-button type="primary" size="small" @click="onSearch">{{ t('common.search') }}</el-button>
      </div>

      <div v-if="loading" class="empty-state">{{ t('common.loading') }}</div>

      <div v-else-if="skills.length === 0" class="empty-state">
        {{ t('workbench.skills.empty') }}
      </div>

      <template v-else>
        <div class="result-grid">
          <div v-for="skill in skills" :key="skill.id" class="skill-card" @click="showDetail(skill.name, skill.source_type)">
            <div class="skill-actions">
              <el-button size="small" type="primary" text :loading="evolving.has(skill.id)" @click="evolveExistingSkill(skill, $event)">
                {{ t('workbench.common.evolve') }}
              </el-button>
              <el-button size="small" text @click="openEdit(skill, $event)">{{ t('common.edit') }}</el-button>
              <el-button size="small" text type="danger" @click="confirmDelete(skill, $event)">{{ t('common.delete') }}</el-button>
            </div>
            <div class="item-title">{{ skill.name }}</div>
            <div class="item-desc">{{ skill.description || t('workbench.common.noDescription') }}</div>
            <div class="tag-row">
              <el-tag size="small">{{ skill.category || 'other' }}</el-tag>
              <el-tag size="small" type="info">{{ skill.source_type }}</el-tag>
              <el-tag v-if="skill.usage_count > 0" size="small" type="success">
                {{ t('workbench.skills.usageCount', { n: skill.usage_count }) }}
              </el-tag>
            </div>
          </div>
        </div>

        <div class="pagination-row">
          <el-pagination
            v-model:current-page="page"
            v-model:page-size="size"
            :total="total"
            :page-sizes="[10, 20, 50]"
            :pager-count="5"
            layout="total, sizes, prev, pager, next"
            @current-change="onPageChange"
            @size-change="onSizeChange"
          />
        </div>
      </template>
    </template>

    <SkillDetailDialog v-model:visible="detailVisible" :loading="detailLoading" :detail="currentDetail" />
    <SkillEditDialog ref="editDialogRef" v-model:visible="editVisible" :skill="editSkill" @saved="onEditSaved" />
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
  gap: 10px;
  margin-bottom: 16px;
  flex-wrap: wrap;
}
.toolbar-title { font-weight: 600; margin-right: 4px; }
.search-input { width: 240px; margin-left: auto; }
.hidden-input { display: none; }
.result-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 16px;
}
.empty-state { text-align: center; padding: 40px; color: var(--muted); }
.skill-card {
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
.skill-actions {
  position: absolute;
  top: 6px;
  right: 6px;
  display: flex;
  gap: 2px;
}
.item-title { font-weight: bold; font-size: 15px; padding-right: 120px; margin-bottom: 6px; }
.skill-card:hover { box-shadow: 0 4px 12px rgba(0,0,0,0.15); }
.cards {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 18px;
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
.item-desc {
  color: #909399;
  font-size: 13px;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  margin-bottom: 8px;
}
.tag-row { display: flex; gap: 6px; flex-wrap: wrap; }
.pagination-row {
  display: flex;
  justify-content: center;
  margin-top: 20px;
}

@media (max-width: 960px) {
  .result-grid { grid-template-columns: repeat(2, 1fr); }
  .cards { grid-template-columns: repeat(2, 1fr); }
}

@media (max-width: 640px) {
  .result-grid { grid-template-columns: 1fr; }
  .cards { grid-template-columns: 1fr; }
}
</style>
