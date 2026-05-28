<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { invoke } from '@tauri-apps/api/core'
import { useTeamStore } from '../stores/useTeamStore'
import {
  approveEvolution,
  cacheTeamSkills,
  fetchAllPosts,
  fetchDeptTree,
  fetchEvolutions,
  fetchTeamSkillDetail,
  fetchTeamSkills,
  getCachedTeamSkills,
  type DeptTreeNode,
  type PostItem,
  type TeamEvolution,
  type TeamSkill,
} from '../api/team'
import { getErrorMessage } from '../utils/error'
import { TEAM_CATEGORY_OPTIONS, TEAM_RESOURCE_TYPE_OPTIONS } from '../constants/team'
import { createStoredZip, downloadBlob, sanitizeZipSegment } from '../utils/zip'
import LoginDialog from '../components/team/LoginDialog.vue'
import ShareSkillDialog from '../components/team/ShareSkillDialog.vue'
import EvolutionProposalDialog from '../components/team/EvolutionProposalDialog.vue'

const store = useTeamStore()
const loginDialog = ref<InstanceType<typeof LoginDialog> | null>(null)
const shareDialog = ref<InstanceType<typeof ShareSkillDialog> | null>(null)
const evolutionDialog = ref<InstanceType<typeof EvolutionProposalDialog> | null>(null)

const query = ref('')
const category = ref('')
const resourceType = ref('')
const filterAgent = ref('')
const deptId = ref<number | undefined>(undefined)
const postId = ref<number | undefined>(undefined)
const skills = ref<TeamSkill[]>([])
const total = ref(0)
const page = ref(1)
const perPage = 10
const loading = ref(false)
const downloading = ref<number | null>(null)
const deptTree = ref<DeptTreeNode[]>([])
const postList = ref<PostItem[]>([])
const showEvolutions = ref(false)
const evolutions = ref<TeamEvolution[]>([])
const evolutionsLoading = ref(false)
const isOffline = ref(false)
const cachedAt = ref('')
const showAdvanced = ref(false)

const agentOptions = [
  'Hermes',
  'OpenClaw',
  'Claude Code',
  'Codex',
  'VSCode',
  'Cursor',
  'CodeBuddy',
  'TRAE',
  'ZeeLinClaw',
]

const hasAdvancedFilter = computed(() => filterAgent.value || postId.value)
const displayedSkills = computed(() => skills.value.filter(agentMatches))
const pendingEvolutionCount = computed(() => evolutions.value.filter(item => item.status === 'pending').length)

async function loadSkills() {
  if (!store.isAuthenticated) return
  loading.value = true
  try {
    const params: Record<string, string> = { pageNum: String(page.value), pageSize: String(perPage) }
    if (query.value) params.name = query.value
    if (category.value) params.category = category.value
    if (resourceType.value) params.sourceType = resourceType.value
    if (deptId.value) params.deptId = String(deptId.value)
    if (postId.value) params.postId = String(postId.value)
    const res = await fetchTeamSkills(params)
    skills.value = res.items
    total.value = res.total
    isOffline.value = false

    if (page.value === 1 && !query.value && !category.value && !resourceType.value && !deptId.value && !postId.value) {
      cacheTeamSkills(res.items).catch(() => {})
    }
  } catch (e: unknown) {
    try {
      const cached = await getCachedTeamSkills()
      if (cached.skills.length > 0) {
        skills.value = cached.skills
        total.value = cached.count
        isOffline.value = true
        cachedAt.value = cached.cached_at
      } else {
        ElMessage.error(getErrorMessage(e, '加载团队 Skills 失败'))
      }
    } catch {
      ElMessage.error(getErrorMessage(e, '加载团队 Skills 失败'))
    }
  } finally {
    loading.value = false
  }
}

async function loadDeptTree() {
  try {
    deptTree.value = await fetchDeptTree()
  } catch { /* offline or not available */ }
}

async function loadPosts() {
  try {
    postList.value = await fetchAllPosts()
  } catch { /* offline or not available */ }
}

async function loadDropdownData() {
  await Promise.all([loadDeptTree(), loadPosts()])
}

function onSearch() {
  page.value = 1
  loadSkills()
}

function resetFilters() {
  query.value = ''
  category.value = ''
  resourceType.value = ''
  deptId.value = undefined
  postId.value = undefined
  filterAgent.value = ''
  showAdvanced.value = false
  page.value = 1
  loadSkills()
}

function handlePageChange(nextPage: number) {
  page.value = nextPage
  loadSkills()
}

function openShare() {
  shareDialog.value?.open()
}

function openEvolution(skillId?: number) {
  evolutionDialog.value?.open(skillId)
}

async function loadEvolutions() {
  evolutionsLoading.value = true
  try {
    const res = await fetchEvolutions({ pageSize: '80' })
    evolutions.value = res.items
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '加载进化记录失败'))
  } finally {
    evolutionsLoading.value = false
  }
}

function toggleEvolutions() {
  if (!store.isAuthenticated) {
    loginDialog.value?.open()
    return
  }
  showEvolutions.value = !showEvolutions.value
  if (showEvolutions.value) loadEvolutions()
}

async function reviewEvolution(evolution: TeamEvolution, status: 'approved' | 'rejected') {
  const label = status === 'approved' ? '通过' : '拒绝'
  try {
    await ElMessageBox.confirm(`确定${label}这条进化提案吗？`, '审核确认', {
      confirmButtonText: label,
      cancelButtonText: '取消',
      type: status === 'approved' ? 'success' : 'warning',
    })
    await approveEvolution(evolution.id, { status })
    ElMessage.success(`已${label}进化提案`)
    loadEvolutions()
    if (status === 'approved') loadSkills()
  } catch (e: unknown) {
    if (String(e) !== 'cancel') ElMessage.error(getErrorMessage(e, '操作失败'))
  }
}

function evolutionStatusTag(status: string) {
  const map: Record<string, string> = { pending: 'warning', approved: 'success', rejected: 'danger' }
  return map[status] || 'info'
}

function evolutionStatusText(status: string) {
  const map: Record<string, string> = { pending: '待审核', approved: '已通过', rejected: '已拒绝' }
  return map[status] || status
}

function onShared() {
  loadSkills()
}

function categoryLabel(value: string): string {
  const found = TEAM_CATEGORY_OPTIONS.find(c => c.value === value)
  return found ? found.label : (value || '其他')
}

function typeLabel(sourceType: string): string {
  const found = TEAM_RESOURCE_TYPE_OPTIONS.find(item => item.value === sourceType)
  return found?.label || sourceType || 'Skill'
}

function isUrlResource(skill: TeamSkill) {
  return skill.sourceType === 'url'
}

function extractUrl(skill: TeamSkill) {
  const body = skill.bodyMd || ''
  const match = body.match(/https?:\/\/[^\s)]+/i)
  return match?.[0] || ''
}

function parseJsonArray(val: string): string[] {
  if (!val) return []
  try {
    const parsed = JSON.parse(val)
    if (Array.isArray(parsed)) return parsed
  } catch { /* fall through */ }
  return val.split(',').map(item => item.trim()).filter(Boolean)
}

function agentMatches(skill: TeamSkill): boolean {
  if (!filterAgent.value) return true
  const agents = parseJsonArray(skill.compatibleAgents)
  return agents.length === 0 || agents.includes('*') || agents.includes(filterAgent.value)
}

async function downloadSkillZip(skill: TeamSkill) {
  if (isUrlResource(skill)) {
    ElMessage.warning('工具网址不支持下载 zip')
    return
  }
  downloading.value = skill.id
  try {
    const detail = skill.bodyMd ? skill : await fetchTeamSkillDetail(skill.id)
    const body = detail.bodyMd || ''
    if (!body.trim()) {
      ElMessage.warning('该 Skill 没有可下载的内容')
      return
    }
    const dir = sanitizeZipSegment(detail.name)
    const blob = createStoredZip([{ path: `${dir}/SKILL.md`, content: body }])
    downloadBlob(blob, `${dir}.zip`)
    ElMessage.success('zip 已生成')
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '下载 zip 失败'))
  } finally {
    downloading.value = null
  }
}

async function openToolUrl(skill: TeamSkill) {
  try {
    const detail = skill.bodyMd ? skill : await fetchTeamSkillDetail(skill.id)
    const url = extractUrl(detail)
    if (!url) {
      ElMessage.warning('该资源没有可打开的网址')
      return
    }
    if (window.__TAURI_INTERNALS__) {
      await invoke('open_external_url', { url })
    } else {
      window.open(url, '_blank', 'noopener,noreferrer')
    }
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '打开网址失败'))
  }
}

onMounted(() => {
  if (store.isAuthenticated) {
    loadSkills()
    loadDropdownData()
  }
})
</script>

<template>
  <div class="page-view">
    <div class="page-header">
      <div>
        <h2>团队 Skills</h2>
        <p class="subtitle">浏览团队共享的 Skills，提交进化提案，或下载 zip 发给团队成员离线安装。</p>
      </div>
      <div class="header-actions">
        <el-button @click="toggleEvolutions">
          {{ showEvolutions ? '返回 Skills' : '进化记录' }}
          <el-tag v-if="pendingEvolutionCount" class="button-tag" size="small" type="warning">{{ pendingEvolutionCount }}</el-tag>
        </el-button>
        <el-button type="primary" plain @click="openEvolution()">提案进化</el-button>
        <el-button type="primary" @click="openShare">分享 Skill</el-button>
      </div>
    </div>

    <template v-if="!store.isAuthenticated">
      <div class="login-prompt">
        <div class="login-card">
          <h3>登录团队版</h3>
          <p>登录后可以浏览、搜索、分享和下载团队共享的 Skills。</p>
          <el-button type="primary" size="large" @click="loginDialog?.open()">登录团队版</el-button>
        </div>
      </div>
    </template>

    <template v-else>
      <div v-if="isOffline" class="offline-banner">
        <span class="offline-dot"></span>
        离线模式，当前展示缓存数据。上次缓存：{{ cachedAt || '未知' }}
        <el-button size="small" text @click="loadSkills">重试</el-button>
      </div>

      <div v-if="!showEvolutions" class="toolbar">
        <div class="toolbar-main">
          <el-input
            v-model="query"
            class="search-input"
            placeholder="搜索 Skill 名称..."
            clearable
            @keyup.enter="onSearch"
          />
          <el-select v-model="category" placeholder="分类" clearable @change="onSearch" style="width: 140px">
            <el-option v-for="c in TEAM_CATEGORY_OPTIONS" :key="c.value" :label="c.label" :value="c.value" />
          </el-select>
          <el-select v-model="resourceType" placeholder="资源类型" clearable @change="onSearch" style="width: 140px">
            <el-option v-for="item in TEAM_RESOURCE_TYPE_OPTIONS" :key="item.value" :label="item.label" :value="item.value" />
          </el-select>
          <el-tree-select
            v-model="deptId"
            :data="deptTree"
            :props="{ value: 'id', label: 'label', children: 'children' }"
            placeholder="所属部门"
            clearable
            filterable
            check-strictly
            style="width: 220px"
            popper-class="dept-tree-popper"
            @change="onSearch"
          />
          <el-button type="primary" :loading="loading" @click="onSearch">搜索</el-button>
          <el-button @click="resetFilters">重置</el-button>
          <el-button :type="hasAdvancedFilter ? 'warning' : 'default'" text @click="showAdvanced = !showAdvanced">
            {{ showAdvanced ? '收起筛选' : '更多筛选' }}
            <span v-if="hasAdvancedFilter" class="filter-dot"></span>
          </el-button>
        </div>

        <div v-show="showAdvanced" class="toolbar-advanced">
          <span class="filter-label">岗位</span>
          <el-select v-model="postId" placeholder="选择岗位" clearable @change="onSearch" style="width: 160px">
            <el-option v-for="p in postList" :key="p.postId" :label="p.postName" :value="p.postId" />
          </el-select>
          <span class="filter-label">Agent</span>
          <el-select v-model="filterAgent" placeholder="Agent" clearable style="width: 170px">
            <el-option v-for="a in agentOptions" :key="a" :label="a" :value="a" />
          </el-select>
        </div>
      </div>

      <div v-if="!showEvolutions" v-loading="loading" class="skills-grid">
        <article v-for="skill in displayedSkills" :key="skill.id" class="skill-card">
          <div class="card-header">
            <h4 class="skill-name">{{ skill.name }}</h4>
            <div class="header-tags">
              <el-tag size="small">{{ categoryLabel(skill.category) }}</el-tag>
              <el-tag size="small" :type="isUrlResource(skill) ? 'warning' : 'success'" effect="plain">{{ typeLabel(skill.sourceType) }}</el-tag>
            </div>
          </div>

          <p class="skill-desc">{{ skill.description || '暂无描述' }}</p>

          <div class="card-meta">
            <span>v{{ skill.version }}</span>
            <span>使用 {{ skill.usageCount ?? 0 }} 次</span>
            <span v-if="skill.avgScore">评分 {{ (skill.avgScore * 100).toFixed(0) }}</span>
            <span v-if="skill.originAgent">{{ skill.originAgent }}</span>
          </div>

          <div v-if="!isUrlResource(skill) && parseJsonArray(skill.compatibleAgents).length" class="compat-row">
            <span class="compat-label">Agent</span>
            <el-tag v-for="agent in parseJsonArray(skill.compatibleAgents)" :key="agent" size="small" type="info" effect="plain">
              {{ agent }}
            </el-tag>
          </div>

          <div v-if="!isUrlResource(skill) && parseJsonArray(skill.compatibleModels).length" class="compat-row">
            <span class="compat-label">模型</span>
            <el-tag v-for="model in parseJsonArray(skill.compatibleModels)" :key="model" size="small" effect="plain">
              {{ model }}
            </el-tag>
          </div>

          <div class="card-actions">
            <span class="updated-at">{{ skill.updatedAt?.slice(0, 10) || '' }}</span>
            <div class="action-buttons">
              <el-button v-if="isUrlResource(skill)" size="small" text type="primary" @click="openToolUrl(skill)">
                打开网址
              </el-button>
              <el-button v-else size="small" text type="primary" :loading="downloading === skill.id" @click="downloadSkillZip(skill)">
                下载 zip
              </el-button>
              <el-button v-if="!isUrlResource(skill)" size="small" text type="primary" @click="openEvolution(skill.id)">提案进化</el-button>
            </div>
          </div>
        </article>

        <div v-if="!loading && displayedSkills.length === 0" class="empty-state">
          <p>暂无团队 Skills</p>
          <p class="hint">点击“分享 Skill”将本地 Skill 提交到团队，也可以用进化提案改进已有 Skill。</p>
        </div>
      </div>

      <section v-if="showEvolutions" v-loading="evolutionsLoading" class="evolutions-section">
        <div class="section-title">
          <div>
            <h3>协同进化记录</h3>
            <p>跟踪每一次提案、审核状态和对应 Skill，方便团队沉淀版本演进。</p>
          </div>
          <el-button size="small" @click="loadEvolutions">刷新</el-button>
        </div>

        <div class="evolution-list">
          <article v-for="item in evolutions" :key="item.id" class="evolution-card">
            <div class="evolution-main">
              <div class="evolution-title">
                <span>#{{ item.id }} · Skill {{ item.skillId }}</span>
                <el-tag :type="evolutionStatusTag(item.status)" size="small">{{ evolutionStatusText(item.status) }}</el-tag>
              </div>
              <p class="evolution-reason">{{ item.reason || '未填写改进理由' }}</p>
              <div class="evolution-meta">
                <span>{{ item.previousVersion || '未知版本' }}</span>
                <span>{{ item.createdAt?.slice(0, 16) || '' }}</span>
                <span v-if="item.reviewedAt">审核于 {{ item.reviewedAt.slice(0, 16) }}</span>
              </div>
            </div>
            <div class="evolution-actions">
              <template v-if="item.status === 'pending'">
                <el-button size="small" type="success" plain @click="reviewEvolution(item, 'approved')">通过</el-button>
                <el-button size="small" type="danger" plain @click="reviewEvolution(item, 'rejected')">拒绝</el-button>
              </template>
              <span v-else class="muted-text">已处理</span>
            </div>
          </article>
        </div>

        <div v-if="!evolutionsLoading && evolutions.length === 0" class="empty-hint">暂无进化记录</div>
      </section>

      <div v-if="!showEvolutions && total > perPage" class="pagination-row">
        <el-pagination
          background
          layout="prev, pager, next"
          :current-page="page"
          :page-size="perPage"
          :total="total"
          @current-change="handlePageChange"
        />
      </div>
    </template>

    <LoginDialog ref="loginDialog" @logged-in="() => { loadSkills(); loadDropdownData() }" />
    <ShareSkillDialog ref="shareDialog" @shared="onShared" />
    <EvolutionProposalDialog ref="evolutionDialog" @submitted="() => { loadSkills(); loadEvolutions() }" />
  </div>
</template>

<style scoped>
.page-view {
  max-width: 1200px;
  margin: 0 auto;
}

.page-header {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  align-items: flex-start;
  margin-bottom: 20px;
}

.header-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.button-tag {
  margin-left: 6px;
}

.page-header h2 {
  margin: 0;
  font-size: 22px;
  font-weight: 700;
}

.subtitle {
  margin: 6px 0 0;
  font-size: 13px;
  color: var(--muted);
}

.login-prompt {
  display: grid;
  place-items: center;
  min-height: 360px;
}

.login-card {
  text-align: center;
  padding: 48px;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  background: var(--panel);
  box-shadow: var(--shadow);
}

.login-card h3 {
  margin: 0 0 8px;
  font-size: 20px;
  color: var(--ink);
}

.login-card p {
  margin: 0 0 20px;
  color: var(--muted);
  font-size: 14px;
}

.toolbar {
  margin-bottom: 18px;
  padding: 16px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--panel);
  box-shadow: var(--shadow);
}

.toolbar-main,
.toolbar-advanced {
  display: flex;
  gap: 10px;
  align-items: center;
  flex-wrap: wrap;
}

.toolbar-advanced {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px dashed var(--line);
}

.filter-label {
  font-size: 13px;
  color: var(--muted);
  white-space: nowrap;
  min-width: 36px;
}

.filter-dot {
  display: inline-block;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #e6a23c;
  margin-left: 2px;
  vertical-align: middle;
}

.offline-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  margin-bottom: 14px;
  background: rgba(245, 158, 11, .12);
  border: 1px solid rgba(245, 158, 11, .28);
  border-radius: 8px;
  font-size: 13px;
  color: #b7791f;
}

.offline-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #f59e0b;
  flex-shrink: 0;
}

.search-input {
  flex: 1;
  min-width: 240px;
  max-width: 400px;
}

.skills-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 16px;
  min-height: 200px;
}

.skill-card,
.evolution-card {
  padding: 18px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--panel);
  box-shadow: var(--shadow);
  transition: all .2s;
}

.skill-card:hover {
  box-shadow: var(--shadow-hover);
  transform: translateY(-1px);
  border-color: rgba(13, 148, 136, .18);
}

.card-header,
.card-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.header-tags {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.skill-name {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  overflow-wrap: anywhere;
}

.skill-desc {
  min-height: 40px;
  margin: 12px 0;
  font-size: 13px;
  line-height: 1.5;
  color: var(--muted);
}

.card-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin-bottom: 10px;
  font-size: 12px;
  color: var(--muted);
}

.compat-row {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  align-items: center;
  margin-bottom: 8px;
}

.compat-label {
  font-size: 11px;
  color: var(--muted);
  margin-right: 2px;
}

.updated-at {
  font-size: 12px;
  color: var(--muted);
}

.action-buttons {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.empty-state {
  grid-column: 1 / -1;
  padding: 56px 20px;
  text-align: center;
  color: var(--muted);
}

.empty-state p {
  margin: 0;
  font-size: 16px;
}

.empty-state .hint {
  margin-top: 8px;
  font-size: 13px;
}

.pagination-row {
  display: flex;
  justify-content: center;
  margin-top: 18px;
}

.evolutions-section {
  margin-top: 4px;
}

.section-title {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 14px;
}

.section-title h3 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
}

.section-title p {
  margin: 6px 0 0;
  font-size: 13px;
  color: var(--muted);
}

.evolution-list {
  display: grid;
  gap: 12px;
}

.evolution-card {
  display: flex;
  justify-content: space-between;
  gap: 16px;
}

.evolution-main {
  min-width: 0;
}

.evolution-title {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 14px;
  font-weight: 600;
}

.evolution-reason {
  margin: 8px 0;
  color: var(--ink);
  font-size: 13px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}

.evolution-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  font-size: 12px;
  color: var(--muted);
}

.evolution-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.empty-hint {
  padding: 24px 0;
  text-align: center;
  color: var(--muted);
  font-size: 13px;
}

.muted-text {
  color: var(--muted);
  font-size: 13px;
}

@media (max-width: 760px) {
  .page-header,
  .evolution-card,
  .card-actions {
    flex-direction: column;
    align-items: stretch;
  }

  .header-actions,
  .action-buttons,
  .evolution-actions {
    justify-content: flex-start;
  }
}
</style>

<style>
.dept-tree-popper {
  width: auto !important;
  min-width: 280px !important;
}
.dept-tree-popper .el-select-dropdown__wrap,
.dept-tree-popper .el-tree,
.dept-tree-popper .el-select-dropdown__item {
  min-width: 280px !important;
}
.dept-tree-popper .el-tree-node__label {
  overflow: visible !important;
  white-space: nowrap;
}
</style>
