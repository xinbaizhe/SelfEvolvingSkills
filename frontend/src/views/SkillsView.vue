<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { evolveSkill, fetchSkills, fetchSkillDetail, importSkills, updateSkill, deleteSkill, type SkillItem, type SkillDetail } from '../api/skills'
import { exportSkills } from '../api/export'

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
const editSaving = ref(false)
const editForm = reactive({ name: '', description: '', category: '' })
const editOriginalName = ref('')

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
  } catch (error: any) {
    ElMessage.error(error?.message || '加载 Skills 失败')
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
  } catch (error: any) {
    ElMessage.error(error?.message || '加载 Skill 详情失败')
  } finally {
    detailLoading.value = false
  }
}

// ---- Edit ----
function openEdit(skill: SkillItem, event: MouseEvent) {
  event.stopPropagation()
  editOriginalName.value = skill.name
  editForm.name = skill.name
  editForm.description = skill.description || ''
  editForm.category = skill.category || ''
  editVisible.value = true
}

async function saveEdit() {
  editSaving.value = true
  try {
    const res = await updateSkill(editOriginalName.value, {
      name: editForm.name || undefined,
      description: editForm.description || null,
      category: editForm.category || null,
    })
    if (!res.success) throw new Error(res.error || '更新失败')
    ElMessage.success('Skill 已更新')
    editVisible.value = false
    if (selectedSource.value) await loadSkills(selectedSource.value)
    await loadSourceCounts()
  } catch (error: any) {
    ElMessage.error(error?.message || '更新失败')
  } finally {
    editSaving.value = false
  }
}

// ---- Delete ----
async function confirmDelete(skill: SkillItem, event: MouseEvent) {
  event.stopPropagation()
  try {
    await ElMessageBox.confirm(
      `确定要删除 "${skill.name}" 吗？此操作将从数据库和文件系统中移除该 Skill。`,
      '删除确认',
      { confirmButtonText: '删除', cancelButtonText: '取消', type: 'warning' }
    )
    const res = await deleteSkill(skill.name)
    if (!res.success) throw new Error(res.error || '删除失败')
    ElMessage.success(`已删除 ${res.data?.name || skill.name}`)
    if (selectedSource.value) {
      const currentPage = page.value
      const shouldGoBack = skills.value.length === 1 && currentPage > 1
      if (shouldGoBack) page.value = currentPage - 1
      await loadSkills(selectedSource.value)
    }
    await loadSourceCounts()
  } catch (error: any) {
    if (error !== 'cancel' && error !== 'close') {
      ElMessage.error(error?.message || '删除失败')
    }
  }
}

// ---- Evolve ----
async function evolveExistingSkill(skill: SkillItem, event: MouseEvent) {
  event.stopPropagation()
  evolving.value.add(skill.id)
  try {
    const res = await evolveSkill(skill.name)
    if (!res.success) throw new Error(res.error || '创建进化草稿失败')
    ElMessage.success('已创建手动进化草稿')
    router.push('/workbench?tab=drafts')
  } catch (error: any) {
    ElMessage.error(error?.message || '创建进化草稿失败')
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
    ElMessage.success(`${selectedSourceName.value} Skills 导出成功`)
  } catch (error: any) {
    ElMessage.error(error?.message || '导出失败，请检查本地数据')
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
    if (!res.success || !res.data) throw new Error(res.error || '导入失败')
    const skipped = res.data.skipped?.length || 0
    ElMessage.success(`已导入 ${res.data.count} 个 Skill 到 ${res.data.agent_name}${skipped ? `，跳过 ${skipped} 个文件` : ''}`)
    await loadSkills(selectedSource.value)
    await loadSourceCounts()
  } catch (error: any) {
    ElMessage.error(error?.message || '导入失败，请确认文件为 SKILL.md、skills.json 或 .zip')
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

function formatSize(bytes: number) {
  if (bytes < 1024) return bytes + ' B'
  return (bytes / 1024).toFixed(1) + ' KB'
}

function onSearch() {
  page.value = 1
  if (selectedSource.value) loadSkills(selectedSource.value)
}

onMounted(loadSourceCounts)
</script>

<template>
  <section class="page-view">
    <div class="page-headline">
      <div>
        <h2>已存在的 Skills</h2>
        <p>先选择 Agent，再查看、导入或导出该 Agent 的 Skills。</p>
      </div>
      <span v-if="selectedSource" class="count-badge">{{ total }} 个 Skills</span>
    </div>

    <div v-if="!selectedSource" class="cards">
      <article v-for="src in sources" :key="src.id" class="card source-card" @click="selectSource(src.id)">
        <span class="source-count">{{ sourceCounts[src.id] ?? 0 }}</span>
        <div class="source-icon" :style="{ background: src.color }">{{ src.icon }}</div>
        <h3>{{ src.name }}</h3>
        <p>查看或导入导出 {{ src.name }} 的 Skills</p>
      </article>
    </div>

    <template v-else>
      <div class="toolbar">
        <el-button size="small" @click="clearSource">返回 Agent 列表</el-button>
        <span class="toolbar-title">{{ selectedSourceName }} 的 Skills</span>

        <input
          ref="fileInput"
          class="hidden-input"
          type="file"
          accept=".md,.json,.zip,application/json,text/markdown,text/plain,application/zip"
          @change="onImportFile"
        />
        <el-button size="small" :loading="importing" @click="openImportPicker">导入</el-button>
        <el-dropdown trigger="click" @command="exportCurrentSkills">
          <el-button size="small" :loading="exporting">导出</el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="json">导出 JSON</el-dropdown-item>
              <el-dropdown-item command="csv">导出 CSV</el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>

        <el-input
          v-model="search"
          placeholder="搜索 Skill 名称或描述"
          clearable
          class="search-input"
          @clear="onSearch"
          @keyup.enter="onSearch"
        />
        <el-button type="primary" size="small" @click="onSearch">搜索</el-button>
      </div>

      <div v-if="loading" class="empty-state">加载中...</div>

      <div v-else-if="skills.length === 0" class="empty-state">
        暂无该 Agent 的 Skills。可以导入 SKILL.md、导出的 skills.json 或完整 Skill zip。
      </div>

      <template v-else>
        <div class="result-grid">
          <div v-for="skill in skills" :key="skill.id" class="skill-card" @click="showDetail(skill.name, skill.source_type)">
            <div class="skill-actions">
              <el-button size="small" type="primary" text :loading="evolving.has(skill.id)" @click="evolveExistingSkill(skill, $event)">
                进化
              </el-button>
              <el-button size="small" text @click="openEdit(skill, $event)">编辑</el-button>
              <el-button size="small" text type="danger" @click="confirmDelete(skill, $event)">删除</el-button>
            </div>
            <div class="item-title">{{ skill.name }}</div>
            <div class="item-desc">{{ skill.description || '暂无描述' }}</div>
            <div class="tag-row">
              <el-tag size="small">{{ skill.category || 'other' }}</el-tag>
              <el-tag size="small" type="info">{{ skill.source_type }}</el-tag>
              <el-tag v-if="skill.usage_count > 0" size="small" type="success">
                使用 {{ skill.usage_count }} 次
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

    <!-- Detail Dialog -->
    <el-dialog v-model="detailVisible" :title="currentDetail?.name" width="800px" top="5vh">
      <div v-if="detailLoading" class="empty-state">加载中...</div>
      <div v-else-if="currentDetail">
        <el-descriptions :column="2" border size="small">
          <el-descriptions-item label="名称">{{ currentDetail.name }}</el-descriptions-item>
          <el-descriptions-item label="分类">{{ currentDetail.category }}</el-descriptions-item>
          <el-descriptions-item label="来源">{{ currentDetail.source_type }}</el-descriptions-item>
          <el-descriptions-item label="原始来源">{{ currentDetail.origin || '-' }}</el-descriptions-item>
          <el-descriptions-item label="插件">{{ currentDetail.plugin_name || '-' }}</el-descriptions-item>
          <el-descriptions-item label="使用次数">{{ currentDetail.usage_count }}</el-descriptions-item>
          <el-descriptions-item label="文件大小">{{ formatSize(currentDetail.file_size) }}</el-descriptions-item>
          <el-descriptions-item label="行数">{{ currentDetail.line_count }}</el-descriptions-item>
          <el-descriptions-item label="文件路径" :span="2">{{ currentDetail.file_path }}</el-descriptions-item>
        </el-descriptions>
        <div class="detail-section">
          <h4>Markdown 内容预览</h4>
          <div class="code-preview">{{ currentDetail.body_text || '无内容' }}</div>
        </div>
      </div>
    </el-dialog>

    <!-- Edit Dialog -->
    <el-dialog v-model="editVisible" title="编辑 Skill" width="520px" top="10vh" @closed="editOriginalName = ''">
      <el-form label-position="top">
        <el-form-item label="名称">
          <el-input v-model="editForm.name" placeholder="Skill 名称" />
        </el-form-item>
        <el-form-item label="描述">
          <el-input v-model="editForm.description" type="textarea" :rows="3" placeholder="简要描述" />
        </el-form-item>
        <el-form-item label="分类">
          <el-input v-model="editForm.category" placeholder="如 frontend、backend、devops" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="editVisible = false">取消</el-button>
        <el-button type="primary" :loading="editSaving" @click="saveEdit">保存</el-button>
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
  gap: 10px;
  margin-bottom: 16px;
  flex-wrap: wrap;
}
.toolbar-title { font-weight: 600; margin-right: 4px; }
.search-input { width: 240px; margin-left: auto; }
.hidden-input { display: none; }
.result-grid { display: flex; flex-wrap: wrap; gap: 16px; }
.empty-state { text-align: center; padding: 40px; color: var(--muted); }
.skill-card {
  position: relative;
  width: 280px;
  background: #fff;
  border-radius: 8px;
  padding: 16px;
  cursor: pointer;
  box-shadow: 0 1px 4px rgba(0,0,0,0.08);
  transition: box-shadow 0.2s;
}
.skill-actions {
  position: absolute;
  top: 6px;
  right: 6px;
  display: flex;
  gap: 2px;
}
.item-title { padding-right: 130px; }
.skill-card:hover { box-shadow: 0 4px 12px rgba(0,0,0,0.15); }
.source-card {
  position: relative;
  cursor: pointer;
  transition: all 0.2s;
  border: 2px solid transparent;
}
.source-card:hover {
  border-color: var(--blue);
  transform: translateY(-2px);
  box-shadow: 0 10px 28px rgba(20,115,230,.12);
}
.source-icon {
  width: 44px;
  height: 44px;
  border-radius: 10px;
  color: #fff;
  display: grid;
  place-items: center;
  font-weight: 700;
  font-size: 15px;
  margin-bottom: 8px;
}
.item-title { font-weight: bold; font-size: 15px; margin-bottom: 6px; }
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
.detail-section { margin-top: 16px; }
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
.pagination-row {
  display: flex;
  justify-content: center;
  margin-top: 20px;
}
</style>
