<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { useTeamStore } from '../stores/useTeamStore'
import { fetchTeamSkills, fetchDeptTree, fetchAllPosts, fetchEvolutions, approveEvolution, cacheTeamSkills, getCachedTeamSkills, type TeamSkill, type TeamEvolution, type DeptTreeNode, type PostItem } from '../api/team'
import { getErrorMessage } from '../utils/error'
import LoginDialog from '../components/team/LoginDialog.vue'
import ShareSkillDialog from '../components/team/ShareSkillDialog.vue'
import EvolutionProposalDialog from '../components/team/EvolutionProposalDialog.vue'

const store = useTeamStore()
const loginDialog = ref<InstanceType<typeof LoginDialog> | null>(null)
const shareDialog = ref<InstanceType<typeof ShareSkillDialog> | null>(null)
const evolutionDialog = ref<InstanceType<typeof EvolutionProposalDialog> | null>(null)

const query = ref('')
const category = ref('')
const filterAgent = ref('')
const deptId = ref<number | undefined>(undefined)
const postId = ref<number | undefined>(undefined)
const skills = ref<TeamSkill[]>([])
const total = ref(0)
const page = ref(1)
const perPage = 10
const loading = ref(false)
const deptTree = ref<DeptTreeNode[]>([])
const postList = ref<PostItem[]>([])
const showEvolutions = ref(false)
const evolutions = ref<TeamEvolution[]>([])
const evolutionsLoading = ref(false)
const isOffline = ref(false)
const cachedAt = ref('')
const showAdvanced = ref(false)

const categoryOptions = [
  { label: '编程开发', value: 'coding' },
  { label: '日报数据处理', value: 'daily_report' },
  { label: '日常办公', value: 'office' },
  { label: '数据分析', value: 'data' },
  { label: '测试调试', value: 'testing' },
  { label: '运维部署', value: 'devops' },
  { label: '文档编写', value: 'docs' },
  { label: '设计创意', value: 'design' },
  { label: '其他', value: 'other' },
]

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

async function loadSkills() {
  if (!store.isAuthenticated) return
  loading.value = true
  try {
    const params: Record<string, string> = { pageNum: String(page.value), pageSize: String(perPage) }
    if (query.value) params.name = query.value
    if (category.value) params.category = category.value
    if (deptId.value) params.deptId = String(deptId.value)
    if (postId.value) params.postId = String(postId.value)
    const res = await fetchTeamSkills(params)
    skills.value = res.items
    total.value = res.total
    isOffline.value = false

    if (page.value === 1 && !query.value && !category.value) {
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

function onSearch() {
  page.value = 1
  loadSkills()
}

function resetFilters() {
  query.value = ''
  category.value = ''
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
    const res = await fetchEvolutions({ pageSize: '50' })
    evolutions.value = res.items
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '加载进化列表失败'))
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

async function handleApprove(evolution: TeamEvolution) {
  try {
    await approveEvolution(evolution.id, { status: 'approved' })
    ElMessage.success('已通过进化提案')
    loadEvolutions()
    loadSkills()
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '操作失败'))
  }
}

async function handleReject(evolution: TeamEvolution) {
  try {
    await approveEvolution(evolution.id, { status: 'rejected' })
    ElMessage.success('已拒绝进化提案')
    loadEvolutions()
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '操作失败'))
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
  const found = categoryOptions.find(c => c.value === value)
  return found ? found.label : (value || 'other')
}

function parseJsonArray(val: string): string[] {
  try { return JSON.parse(val) } catch { return [] }
}

function agentMatches(skill: TeamSkill): boolean {
  if (!filterAgent.value) return true
  const agents = parseJsonArray(skill.compatibleAgents)
  return agents.length === 0 || agents.includes('*') || agents.includes(filterAgent.value)
}

async function loadDropdownData() {
  await Promise.all([
    loadDeptTree(),
    loadPosts(),
  ])
}

const displayedSkills = computed(() =>
  skills.value.filter((s) => agentMatches(s))
)

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
        <p class="subtitle">浏览和搜索团队共享的 Skills，安装到本地 Agent 使用</p>
      </div>
      <div class="header-actions">
        <el-button @click="toggleEvolutions">
          {{ showEvolutions ? '返回 Skills' : '进化记录' }}
        </el-button>
        <el-button type="primary" @click="openEvolution()">提案进化</el-button>
        <el-button type="primary" @click="openShare">分享 Skill</el-button>
      </div>
    </div>

    <template v-if="!store.isAuthenticated">
      <div class="login-prompt">
        <div class="login-card">
          <h3>登录团队版</h3>
          <p>登录后可以浏览、搜索和安装团队共享的 Skills</p>
          <el-button type="primary" size="large" @click="loginDialog?.open()">登录团队版</el-button>
        </div>
      </div>
    </template>

    <template v-else>
      <div v-if="isOffline" class="offline-banner">
        <span class="offline-dot"></span>
        离线模式 — 显示缓存数据（上次更新: {{ cachedAt }}）
        <el-button size="small" text @click="loadSkills">重试</el-button>
      </div>

      <div class="toolbar">
        <div class="toolbar-main">
          <el-input
            v-model="query"
            class="search-input"
            placeholder="搜索 Skill 名称..."
            clearable
            @keyup.enter="onSearch"
          >
            <template #prefix>
              <span class="search-icon">🔍</span>
            </template>
          </el-input>
          <el-select v-model="category" placeholder="分类" clearable @change="onSearch" style="width: 130px">
            <el-option v-for="c in categoryOptions" :key="c.value" :label="c.label" :value="c.value" />
          </el-select>
          <el-tree-select
            v-model="deptId"
            :data="deptTree"
            :props="{ value: 'id', label: 'label', children: 'children' }"
            placeholder="所属部门"
            clearable
            filterable
            check-strictly
            style="width: 240px"
            popper-class="dept-tree-popper"
            :popper-options="{ placement: 'bottom-start', modifiers: [{ name: 'preventOverflow', options: { altAxis: false } }] }"
            @change="onSearch"
          />
          <el-button type="primary" :loading="loading" @click="onSearch">搜索</el-button>
          <el-button @click="resetFilters">重置</el-button>
          <el-button
            :type="hasAdvancedFilter ? 'warning' : 'default'"
            text
            @click="showAdvanced = !showAdvanced"
          >
            {{ showAdvanced ? '收起筛选' : '更多筛选' }}
            <span v-if="hasAdvancedFilter" class="filter-dot"></span>
          </el-button>
        </div>

        <div v-show="showAdvanced" class="toolbar-advanced">
          <span class="filter-label">岗位</span>
          <el-select v-model="postId" placeholder="选择岗位" clearable @change="onSearch" style="width: 150px">
            <el-option
              v-for="p in postList"
              :key="p.postId"
              :label="p.postName"
              :value="p.postId"
            />
          </el-select>
          <span class="filter-label">Agent</span>
          <el-select v-model="filterAgent" placeholder="Agent" clearable style="width: 160px">
            <el-option v-for="a in agentOptions" :key="a" :label="a" :value="a" />
          </el-select>
        </div>
      </div>

      <div v-loading="loading" class="skills-grid">
        <article
          v-for="skill in displayedSkills"
          :key="skill.id"
          class="skill-card"
        >
          <div class="card-header">
            <h4 class="skill-name">{{ skill.name }}</h4>
            <el-tag size="small">{{ categoryLabel(skill.category) }}</el-tag>
          </div>

          <p class="skill-desc">{{ skill.description || '暂无描述' }}</p>

          <div class="card-meta">
            <span>v{{ skill.version }}</span>
            <span>使用 {{ skill.usageCount ?? 0 }} 次</span>
            <span v-if="skill.avgScore">评分 {{ (skill.avgScore * 100).toFixed(0) }}</span>
            <span v-if="skill.originAgent">{{ skill.originAgent }}</span>
          </div>

          <div v-if="skill.compatibleAgents" class="compat-row">
            <span class="compat-label">Agent:</span>
            <el-tag
              v-for="agent in parseJsonArray(skill.compatibleAgents)"
              :key="agent"
              size="small"
              type="info"
              effect="plain"
            >{{ agent }}</el-tag>
          </div>

          <div v-if="skill.compatibleModels" class="compat-row">
            <span class="compat-label">模型:</span>
            <el-tag
              v-for="model in parseJsonArray(skill.compatibleModels)"
              :key="model"
              size="small"
              effect="plain"
            >{{ model }}</el-tag>
          </div>

          <div class="card-actions">
            <span class="updated-at">{{ skill.updatedAt?.slice(0, 10) || '' }}</span>
            <el-button size="small" text type="primary" @click="openEvolution(skill.id)">提案进化</el-button>
          </div>
        </article>

        <div v-if="!loading && displayedSkills.length === 0" class="empty-state">
          <p>暂无团队 Skills</p>
          <p class="hint">点击"分享 Skill"将本地 Skill 分享到团队，或请管理员添加。</p>
        </div>
      </div>

      <div v-if="showEvolutions" v-loading="evolutionsLoading" class="evolutions-section">
        <h3>协同进化记录</h3>
        <el-table :data="evolutions" size="small" style="width: 100%">
          <el-table-column prop="id" label="ID" width="60" />
          <el-table-column prop="skillId" label="Skill ID" width="80" />
          <el-table-column prop="reason" label="改进理由" min-width="180" show-overflow-tooltip />
          <el-table-column label="状态" width="90">
            <template #default="{ row }">
              <el-tag :type="evolutionStatusTag(row.status)" size="small">
                {{ evolutionStatusText(row.status) }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="createdAt" label="提交时间" width="140">
            <template #default="{ row }">{{ row.createdAt?.slice(0, 16) || '' }}</template>
          </el-table-column>
          <el-table-column label="操作" width="160" fixed="right">
            <template #default="{ row }">
              <template v-if="row.status === 'pending'">
                <el-button size="small" text type="success" @click="handleApprove(row)">通过</el-button>
                <el-button size="small" text type="danger" @click="handleReject(row)">拒绝</el-button>
              </template>
              <span v-else class="muted-text">-</span>
            </template>
          </el-table-column>
        </el-table>
        <div v-if="!evolutionsLoading && evolutions.length === 0" class="empty-hint">暂无进化记录</div>
      </div>

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
  border-radius: 10px;
  background: var(--panel);
  box-shadow: var(--shadow);
}

.toolbar-main {
  display: flex;
  gap: 10px;
  align-items: center;
  flex-wrap: wrap;
}

.toolbar-advanced {
  display: flex;
  gap: 10px;
  align-items: center;
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px dashed var(--line);
  flex-wrap: wrap;
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
  color: #fbbf24;
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

.search-icon {
  font-size: 14px;
  opacity: 0.5;
}

.skills-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 16px;
  min-height: 200px;
}

.skill-card {
  padding: 18px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--panel);
  box-shadow: var(--shadow);
  transition: all .2s;
}

.skill-card:hover {
  box-shadow: var(--shadow-hover);
  transform: translateY(-1px);
  border-color: rgba(13,148,136,.18);
}

.card-header,
.card-meta,
.card-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.card-meta {
  justify-content: flex-start;
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
  color: var(--muted);
}

.pagination-row {
  display: flex;
  justify-content: center;
  margin-top: 18px;
}

.evolutions-section {
  margin-top: 24px;
}

.evolutions-section h3 {
  margin: 0 0 14px;
  font-size: 18px;
  font-weight: 600;
}

.empty-hint {
  padding: 24px 0;
  text-align: center;
  color: var(--muted);
  font-size: 13px;
}

.muted-text {
  color: var(--muted);
}
</style>

<style>
.dept-tree-popper {
  width: auto !important;
  min-width: 280px !important;
}
.dept-tree-popper .el-select-dropdown__wrap {
  min-width: 280px !important;
}
.dept-tree-popper .el-select-dropdown__item {
  width: 100% !important;
  min-width: 280px !important;
}
.dept-tree-popper .el-tree {
  min-width: 280px !important;
}
.dept-tree-popper .el-tree-node__content {
  overflow: visible !important;
}
.dept-tree-popper .el-tree-node__label {
  overflow: visible !important;
  white-space: nowrap;
}
</style>
