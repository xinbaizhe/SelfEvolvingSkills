<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useRouter } from 'vue-router'
import { useScanStore } from '../../stores/useScanStore'
import { fetchLlmConfig, fetchSystemInfo, testLlmConnection } from '../../api/admin'
import { searchCommunitySkills } from '../../api/community'
import { detectSources, fetchSources, type SourceConfig } from '../../api/scan'
import { fetchSessionDetail, fetchSessions, fetchSummary, type SummaryStats } from '../../api/stats'
import { getDatabaseInfo, getSystemMonitor, clearLogs, initializeDatabase, type DatabaseInfo, type SystemMonitor } from '../../api/system'

interface SystemInfo {
  runtime: string
  database: string
  total_skills: number
  total_agents: number
  total_sessions: number
  sources: SourceConfig[]
}

interface LlmConfig {
  enabled?: boolean
  provider?: string
  base_url?: string
  model?: string
  has_api_key?: boolean
  api_key_configured?: boolean
}

const router = useRouter()
const scanStore = useScanStore()
const systemInfo = ref<SystemInfo | null>(null)
const summary = ref<SummaryStats | null>(null)
const monitor = ref<SystemMonitor | null>(null)
const dbInfo = ref<DatabaseInfo | null>(null)
const sources = ref<SourceConfig[]>([])
const llmConfig = ref<LlmConfig | null>(null)
const llmTesting = ref(false)
const githubTesting = ref(false)
const detecting = ref(false)
const sessions = ref<any[]>([])
const sessionsTotal = ref(0)
const sessionsPage = ref(1)
const sessionsLoading = ref(false)
const sessionsDrawerVisible = ref(false)
const sessionDetailVisible = ref(false)
const currentSession = ref<any>(null)
const sessionDetailLoading = ref(false)
const clearing = ref(false)
const initializing = ref(false)
let refreshTimer: ReturnType<typeof setInterval> | null = null

const tableNameLabels: Record<string, string> = {
  admin_users: '管理员',
  agents: 'AI 助手',
  community_skills: '社区 Skills',
  evolution_jobs: '进化任务',
  memories: '记忆',
  scan_jobs: '扫描任务',
  sessions: '会话',
  skill_usage: 'Skill 使用记录',
  skills: 'Skills',
  source_configs: '数据源配置',
  workflow_clusters: '工作流聚类',
}

const dbTotalRows = computed(() => dbTables.value.reduce((sum, t) => sum + t.count, 0))

const dbTables = computed(() => {
  if (!dbInfo.value) return []
  const entries = Object.entries(dbInfo.value.table_counts).map(([name, count]) => ({
    name,
    label: tableNameLabels[name] || name,
    count,
  }))
  const maxCount = Math.max(...entries.map((e) => e.count), 1)
  return entries.map((e) => ({
    ...e,
    pct: ((e.count / maxCount) * 100).toFixed(1),
  }))
})

const latestScan = computed(() => scanStore.history[0] ?? null)

const sourceStats = computed(() => {
  const total = sources.value.length
  const available = sources.value.filter((source) => source.is_available).length
  const enabled = sources.value.filter((source) => source.is_enabled).length
  const records = sources.value.reduce((sum, source) => sum + Number(source.record_count || 0), 0)
  return { total, available, enabled, records }
})

const healthItems = computed(() => [
  {
    key: 'sources',
    label: '数据源',
    value: `${sourceStats.value.available}/${sourceStats.value.total}`,
    hint: `${sourceStats.value.enabled} 个已启用`,
    type: sourceStats.value.available > 0 ? 'success' : 'warning',
  },
  {
    key: 'sessions',
    label: '会话数量',
    value: String(summary.value?.total_sessions ?? systemInfo.value?.total_sessions ?? 0),
    hint: '点击卡片可查看会话明细',
    type: (summary.value?.total_sessions ?? 0) > 0 ? 'success' : 'info',
  },
  {
    key: 'latest_scan',
    label: '最近扫描',
    value: latestScan.value ? getStatusLabel(latestScan.value.status) : '暂无',
    hint: latestScan.value?.completed_at || latestScan.value?.started_at || '尚未运行扫描',
    type: latestScan.value ? getStatusType(latestScan.value.status) : 'info',
  },
  {
    key: 'path_records',
    label: '路径记录',
    value: String(sourceStats.value.records),
    hint: '来自数据源检测结果',
    type: sourceStats.value.records > 0 ? 'success' : 'info',
  },
])

onMounted(async () => {
  await Promise.all([
    scanStore.loadHistory({ size: 10 }),
    refreshStats(),
    refreshMonitor(),
    refreshSources(),
    refreshLlmConfig(),
  ])
  refreshTimer = setInterval(refreshMonitor, 5000)
})

onUnmounted(() => {
  if (refreshTimer) clearInterval(refreshTimer)
})

async function refreshStats() {
  const [sysRes, sumRes] = await Promise.all([fetchSystemInfo(), fetchSummary()])
  if (sysRes.success) {
    systemInfo.value = sysRes.data as SystemInfo
    sources.value = (systemInfo.value.sources || []) as SourceConfig[]
  }
  if (sumRes.success) summary.value = sumRes.data
}

async function refreshMonitor() {
  const [monRes, dbRes] = await Promise.all([
    getSystemMonitor().catch(() => null),
    getDatabaseInfo().catch(() => null),
  ])
  if (monRes?.success) monitor.value = monRes.data
  if (dbRes?.success) dbInfo.value = dbRes.data
}

async function refreshSources() {
  const res = await fetchSources().catch(() => null)
  if (res?.success && res.data) sources.value = res.data
}

async function refreshLlmConfig() {
  const res = await fetchLlmConfig().catch(() => null)
  if (res?.success) llmConfig.value = res.data as LlmConfig
}

async function runScan() {
  const result = await scanStore.runScan()
  if (result) {
    ElMessage.success(`扫描完成：${result.skills_found} Skills，${result.agents_found} Agents，${result.sessions_found} 会话`)
    await Promise.all([scanStore.loadHistory({ size: 10 }), refreshStats(), refreshMonitor(), refreshSources()])
  }
}

async function runDetectSources() {
  detecting.value = true
  try {
    const res = await detectSources()
    if (res.success) {
      sources.value = res.data || []
      ElMessage.success('数据源检测完成')
      await refreshStats()
    } else {
      ElMessage.error(res.error || '数据源检测失败')
    }
  } finally {
    detecting.value = false
  }
}

async function testLlmService() {
  await refreshLlmConfig()
  if (!llmConfig.value?.base_url || !llmConfig.value?.model) {
    ElMessage.warning('请先配置 LLM Base URL 和模型')
    openResourceConfig()
    return
  }
  if (!llmConfig.value.has_api_key && !llmConfig.value.api_key_configured) {
    ElMessage.warning('请先配置 LLM API Key')
    openResourceConfig()
    return
  }
  llmTesting.value = true
  try {
    const res = await testLlmConnection({
      llm_enabled: !!llmConfig.value.enabled,
      llm_provider: llmConfig.value.provider || 'custom',
      llm_base_url: llmConfig.value.base_url,
      llm_model: llmConfig.value.model,
      llm_api_key: '',
    })
    if (res.success) {
      ElMessage.success(res.data?.message || 'LLM 连接正常')
    } else {
      ElMessage.error(res.error || 'LLM 连接失败')
    }
  } finally {
    llmTesting.value = false
  }
}

function openResourceConfig() {
  router.push({ name: 'resourcesConfig', query: { tab: 'model' } }).catch(() => {
    window.location.hash = '#/resources?tab=model'
  })
}

function handleHealthCardClick(key: string) {
  if (key === 'sessions') {
    const total = summary.value?.total_sessions ?? systemInfo.value?.total_sessions ?? 0
    if (total === 0) {
      ElMessage.warning('会话为空，请先执行扫描')
      return
    }
    router.push('/conversations')
  }
}

async function testGithubService() {
  githubTesting.value = true
  try {
    const res = await searchCommunitySkills('skill', 1, 1)
    if (res.success) {
      ElMessage.success('GitHub 社区检索可访问')
    } else {
      ElMessage.error(res.error || 'GitHub 社区检索失败')
    }
  } finally {
    githubTesting.value = false
  }
}

async function openSessionsDrawer() {
  sessionsDrawerVisible.value = true
  sessionsPage.value = 1
  await loadSessions()
}

async function loadSessions() {
  sessionsLoading.value = true
  try {
    const res = await fetchSessions({ page: sessionsPage.value, size: 20 })
    if (res.success && res.data) {
      sessions.value = (res.data as any).items ?? []
      sessionsTotal.value = (res.data as any).total ?? 0
    } else {
      ElMessage.error('会话列表加载失败')
    }
  } finally {
    sessionsLoading.value = false
  }
}

async function showSessionDetail(sessionId: string) {
  sessionDetailVisible.value = true
  sessionDetailLoading.value = true
  currentSession.value = null
  try {
    const res = await fetchSessionDetail(sessionId)
    if (res.success) {
      currentSession.value = res.data
    } else {
      ElMessage.error('会话详情加载失败')
    }
  } finally {
    sessionDetailLoading.value = false
  }
}

function cpuPercent() {
  return Number.parseFloat(monitor.value?.cpu_usage_percent ?? '0')
}

function memoryPercent() {
  return Number.parseFloat(monitor.value?.memory.usage_percent ?? '0')
}

function formatBytes(bytes: number | null | undefined) {
  const value = Number(bytes ?? 0)
  if (!value) return '-'
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`
  return `${(value / 1024 / 1024).toFixed(2)} MB`
}

function formatUptime(seconds: number | null | undefined) {
  const total = Number(seconds ?? 0)
  if (!total) return '-'
  const days = Math.floor(total / 86400)
  const hours = Math.floor((total % 86400) / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  if (days > 0) return `${days} 天 ${hours} 小时`
  if (hours > 0) return `${hours} 小时 ${minutes} 分钟`
  return `${minutes} 分钟`
}

function getStatusType(status: string) {
  if (status === 'completed') return 'success'
  if (status === 'failed') return 'danger'
  if (status === 'running') return 'warning'
  return 'info'
}

function getStatusLabel(status: string) {
  const labels: Record<string, string> = {
    completed: '已完成',
    failed: '失败',
    running: '运行中',
    pending: '待处理',
    cancelled: '已取消',
  }
  return labels[status] || status
}

function getScanTypeLabel(type: string) {
  const labels: Record<string, string> = {
    full: '全量扫描',
    incremental: '增量扫描',
    manual: '手动扫描',
  }
  return labels[type] || type
}

async function handleClearLogs() {
  try {
    await ElMessageBox.confirm(
      '将清理扫描历史、进化记录、工作流聚类、记忆数据和使用记录；Skills、Agents、会话、社区缓存和配置会保留。',
      '确认清理日志与历史',
      { confirmButtonText: '确认清理', cancelButtonText: '取消', type: 'warning' },
    )
  } catch {
    return
  }
  clearing.value = true
  try {
    const res = await clearLogs()
    if (res.success) {
      ElMessage.success(res.data?.message || '清理完成')
      await Promise.all([refreshStats(), refreshMonitor(), scanStore.loadHistory({ size: 10 })])
    } else {
      ElMessage.error(res.error || '清理失败')
    }
  } catch {
    ElMessage.error('清理失败')
  } finally {
    clearing.value = false
  }
}

async function handleInitializeDatabase() {
  try {
    await ElMessageBox.confirm(
      '将初始化数据库：清空 Skills、Agents、会话、记忆、使用记录、扫描历史、进化记录、工作流聚类和社区缓存。仅保留管理员、系统配置和数据源配置。',
      '确认初始化数据库',
      { confirmButtonText: '确认初始化', cancelButtonText: '取消', type: 'error' },
    )
  } catch {
    return
  }
  initializing.value = true
  try {
    const res = await initializeDatabase()
    if (res.success) {
      ElMessage.success(res.data?.message || '数据库已初始化')
      await Promise.all([refreshStats(), refreshMonitor(), refreshSources(), scanStore.loadHistory({ size: 10 })])
    } else {
      ElMessage.error(res.error || '初始化失败')
    }
  } catch {
    ElMessage.error('初始化失败')
  } finally {
    initializing.value = false
  }
}
</script>

<template>
  <div class="admin-page">
    <div class="admin-head">
      <div>
        <h2>系统管理</h2>
        <p>查看本地运行环境、数据健康、外部服务和维护操作。</p>
      </div>
      <button class="admin-config-btn" @click="openResourceConfig">系统配置</button>
    </div>

    <section class="section">
      <div class="section-head">
        <h3>运行概览</h3>
        <span>每 5 秒刷新资源数据</span>
      </div>
      <el-row :gutter="16">
        <el-col :xs="24" :sm="12" :lg="6">
          <div class="metric-card">
            <div class="metric-label">CPU 使用率</div>
            <div class="metric-value" :class="{ warning: cpuPercent() > 80, danger: cpuPercent() > 90 }">{{ monitor?.cpu_usage_percent ?? '0.0' }}%</div>
            <el-progress :percentage="cpuPercent()" :stroke-width="6" :show-text="false" />
            <div class="metric-sub">{{ monitor?.cpu_brand || '未读取到 CPU 信息' }}</div>
          </div>
        </el-col>
        <el-col :xs="24" :sm="12" :lg="6">
          <div class="metric-card">
            <div class="metric-label">内存</div>
            <div class="metric-value">{{ monitor?.memory.usage_percent ?? '0.0' }}%</div>
            <el-progress :percentage="memoryPercent()" :stroke-width="6" :show-text="false" :color="memoryPercent() > 80 ? '#f56c6c' : '#0d9488'" />
            <div class="metric-sub">{{ monitor?.memory.used_mb ?? 0 }} / {{ monitor?.memory.total_mb ?? 0 }} MB</div>
          </div>
        </el-col>
        <el-col :xs="24" :sm="12" :lg="6">
          <div class="metric-card">
            <div class="metric-label">磁盘</div>
            <div class="metric-value">{{ monitor?.disks[0]?.usage_pct ?? '--' }}%</div>
            <el-progress v-if="monitor?.disks[0]" :percentage="monitor.disks[0].usage_pct" :stroke-width="6" :show-text="false" />
            <div class="metric-sub" v-if="monitor?.disks[0]">{{ monitor.disks[0].available_gb }} GB 可用 / {{ monitor.disks[0].total_gb }} GB</div>
            <div class="metric-sub" v-else>未读取到磁盘信息</div>
          </div>
        </el-col>
        <el-col :xs="24" :sm="12" :lg="6">
          <div class="metric-card">
            <div class="metric-label">本机开机时长</div>
            <div class="metric-value metric-value--small">{{ formatUptime(monitor?.uptime_seconds) }}</div>
            <div class="metric-lines">
              <span>数据库：{{ dbInfo?.db_size_mb ?? '0.00' }} MB</span>
              <span>WAL：{{ dbInfo?.wal_size_mb ?? '0.00' }} MB</span>
            </div>
          </div>
        </el-col>
      </el-row>
    </section>

    <section class="section">
      <div class="section-head">
        <h3>数据健康</h3>
        <el-button size="small" :loading="detecting" @click="runDetectSources">重新检测数据源</el-button>
      </div>
      <el-row :gutter="16">
        <el-col v-for="item in healthItems" :key="item.key" :xs="24" :sm="12" :lg="6">
          <div
            class="health-card"
            :class="{ 'health-card--clickable': item.key === 'sessions' }"
            role="button"
            tabindex="0"
            @click="handleHealthCardClick(item.key)"
            @keydown.enter="handleHealthCardClick(item.key)"
          >
            <div class="health-top">
              <span>{{ item.label }}</span>
              <el-tag :type="item.type as any" size="small">{{ item.value }}</el-tag>
            </div>
            <div class="health-hint">{{ item.hint }}</div>
          </div>
        </el-col>
      </el-row>

      <el-table :data="sources" stripe class="source-table" empty-text="暂无数据源，请先重新检测">
        <el-table-column prop="agent_name" label="Agent" width="150" />
        <el-table-column label="状态" width="120">
          <template #default="{ row }">
            <el-tag :type="row.is_available ? 'success' : 'warning'">{{ row.is_available ? '可用' : '未检测到' }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column label="启用" width="90">
          <template #default="{ row }">
            <el-tag :type="row.is_enabled ? 'success' : 'info'">{{ row.is_enabled ? '已启用' : '未启用' }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="record_count" label="记录数" width="100" />
        <el-table-column prop="last_scan_at" label="最近扫描" width="170">
          <template #default="{ row }">{{ row.last_scan_at || '-' }}</template>
        </el-table-column>
        <el-table-column prop="detected_path" label="检测路径" min-width="260" show-overflow-tooltip>
          <template #default="{ row }">{{ row.detected_path || '-' }}</template>
        </el-table-column>
      </el-table>
    </section>

    <section class="section">
      <div class="section-head">
        <h3>服务连通性</h3>
        <span>检测社区检索和大模型优化能力</span>
      </div>
      <el-row :gutter="16">
        <el-col :xs="24" :md="12">
          <div class="service-card">
            <div>
              <h4>GitHub 社区检索</h4>
              <p>用于搜索社区 Skill 参考，网络失败时本地扫描和聚类仍可运行。</p>
            </div>
            <el-button :loading="githubTesting" @click="testGithubService">测试 GitHub</el-button>
          </div>
        </el-col>
        <el-col :xs="24" :md="12">
          <div class="service-card">
            <div>
              <h4>LLM 模型服务</h4>
              <p>{{ llmConfig?.enabled ? '已启用' : '未启用' }} · {{ llmConfig?.provider || '未配置' }} · {{ llmConfig?.model || '未选择模型' }}</p>
            </div>
            <div class="service-actions">
              <el-button :loading="llmTesting" @click="testLlmService">测试 LLM</el-button>
              <el-button link type="primary" @click="openResourceConfig">配置</el-button>
            </div>
          </div>
        </el-col>
      </el-row>
    </section>

    <section class="section">
      <div class="section-head">
        <h3>维护操作</h3>
        <span>清理和初始化会影响本地数据，请谨慎操作</span>
      </div>
      <div class="maintenance-grid">
        <div class="maintenance-item">
          <h4>扫描本机 Agent 数据</h4>
          <p>扫描 Skills、Agents、会话和记忆数据，并更新最近扫描记录。</p>
          <el-button type="primary" :loading="scanStore.scanning" @click="runScan">{{ scanStore.scanning ? '扫描中...' : '开始扫描' }}</el-button>
        </div>
        <div class="maintenance-item">
          <h4>清理日志与历史</h4>
          <p>保留业务数据，只清理扫描历史、进化记录、聚类和使用记录。</p>
          <el-button type="warning" :loading="clearing" @click="handleClearLogs">清理日志与历史</el-button>
        </div>
        <div class="maintenance-item maintenance-item--danger">
          <h4>初始化数据库</h4>
          <p>清空 Skills、Agents、会话、社区缓存和历史记录，仅保留基础配置。</p>
          <el-button type="danger" :loading="initializing" @click="handleInitializeDatabase">初始化数据库</el-button>
        </div>
      </div>

      <el-card class="history-card" shadow="never">
        <template #header>最近扫描历史</template>
        <el-table :data="scanStore.history" stripe max-height="360" empty-text="暂无扫描记录">
          <el-table-column prop="id" label="ID" width="60" />
          <el-table-column prop="scan_type" label="类型" width="100">
            <template #default="{ row }">{{ getScanTypeLabel(row.scan_type) }}</template>
          </el-table-column>
          <el-table-column prop="status" label="状态" width="110">
            <template #default="{ row }">
              <el-tag :type="getStatusType(row.status) as any">{{ getStatusLabel(row.status) }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="skills_found" label="Skills" width="90" />
          <el-table-column prop="agents_found" label="Agents" width="90" />
          <el-table-column prop="sessions_found" label="会话" width="90" />
          <el-table-column prop="conversations_analyzed" label="分析数" width="90" />
          <el-table-column prop="memories_found" label="记忆" width="80" />
          <el-table-column prop="started_at" label="开始时间" width="170" />
          <el-table-column prop="completed_at" label="完成时间" width="170" />
          <el-table-column prop="errors" label="错误" min-width="200">
            <template #default="{ row }">
              <span v-if="row.errors" class="error-text">{{ row.errors }}</span>
              <span v-else class="ok-text">无</span>
            </template>
          </el-table-column>
        </el-table>
      </el-card>

      <el-card class="history-card" shadow="never" v-if="dbInfo">
        <template #header>
          <div class="db-card-header">
            <span>数据库表统计</span>
            <span class="db-card-sub">共 {{ dbTables.length }} 张表</span>
          </div>
        </template>
        <div class="db-path">{{ dbInfo.db_path }}</div>
        <el-table :data="dbTables" stripe size="small" class="db-table">
          <el-table-column prop="label" label="表名" width="180" />
          <el-table-column prop="name" label="英文名" width="200">
            <template #default="{ row }">
              <code class="db-table-code">{{ row.name }}</code>
            </template>
          </el-table-column>
          <el-table-column prop="count" label="记录数" width="120" sortable>
            <template #default="{ row }">
              <span class="db-table-count">{{ row.count.toLocaleString() }}</span>
            </template>
          </el-table-column>
          <el-table-column label="数据占比" min-width="240">
            <template #default="{ row }">
              <div class="db-bar-row">
                <el-progress :percentage="Number(row.pct)" :stroke-width="8" :show-text="false" />
                <span class="db-bar-label">{{ row.pct }}%</span>
              </div>
            </template>
          </el-table-column>
        </el-table>
        <div class="db-footer">
          总记录数：<strong>{{ dbTotalRows.toLocaleString() }}</strong>
        </div>
      </el-card>
    </section>

    <el-drawer v-model="sessionsDrawerVisible" title="会话详情" size="72%">
      <el-table :data="sessions" v-loading="sessionsLoading" stripe height="calc(100vh - 190px)" empty-text="暂无会话数据，请先执行扫描">
        <el-table-column prop="agent_source" label="Agent" width="120" />
        <el-table-column prop="project_name" label="项目" width="180" show-overflow-tooltip />
        <el-table-column prop="first_prompt" label="首条请求" min-width="260" show-overflow-tooltip />
        <el-table-column prop="message_count" label="消息数" width="90" />
        <el-table-column prop="jsonl_size" label="文件大小" width="100">
          <template #default="{ row }">{{ formatBytes(row.jsonl_size) }}</template>
        </el-table-column>
        <el-table-column prop="started_at" label="开始时间" width="170" />
        <el-table-column label="操作" width="90" fixed="right">
          <template #default="{ row }">
            <el-button link type="primary" @click="showSessionDetail(row.session_id)">查看</el-button>
          </template>
        </el-table-column>
      </el-table>
      <div class="drawer-pagination">
        <el-pagination v-model:current-page="sessionsPage" :total="sessionsTotal" :page-size="20" layout="total, prev, pager, next" @current-change="loadSessions" />
      </div>
    </el-drawer>

    <el-dialog v-model="sessionDetailVisible" title="会话记录" width="760px" append-to-body destroy-on-close>
      <div v-loading="sessionDetailLoading">
        <el-descriptions v-if="currentSession" :column="1" border size="small">
          <el-descriptions-item label="会话 ID">{{ currentSession.session_id }}</el-descriptions-item>
          <el-descriptions-item label="Agent">{{ currentSession.agent_source || '-' }}</el-descriptions-item>
          <el-descriptions-item label="项目">{{ currentSession.project_name || '-' }}</el-descriptions-item>
          <el-descriptions-item label="工作目录">{{ currentSession.cwd || '-' }}</el-descriptions-item>
          <el-descriptions-item label="入口">{{ currentSession.entrypoint || '-' }}</el-descriptions-item>
          <el-descriptions-item label="版本">{{ currentSession.version || '-' }}</el-descriptions-item>
          <el-descriptions-item label="类型">{{ currentSession.kind || '-' }}</el-descriptions-item>
          <el-descriptions-item label="开始时间">{{ currentSession.started_at || '-' }}</el-descriptions-item>
          <el-descriptions-item label="消息数">{{ currentSession.message_count ?? 0 }}</el-descriptions-item>
          <el-descriptions-item label="JSONL 文件">{{ currentSession.jsonl_path || '-' }}</el-descriptions-item>
          <el-descriptions-item label="文件大小">{{ formatBytes(currentSession.jsonl_size) }}</el-descriptions-item>
          <el-descriptions-item label="首条用户请求">
            <pre class="session-text">{{ currentSession.first_prompt || '未提取到首条用户请求，重新扫描后可补充。' }}</pre>
          </el-descriptions-item>
          <el-descriptions-item label="压缩摘要">
            <pre class="session-text">{{ currentSession.compressed_summary || '暂无压缩摘要，重新扫描后可补充。' }}</pre>
          </el-descriptions-item>
        </el-descriptions>
      </div>
    </el-dialog>
  </div>
</template>

<style scoped>
.admin-page {
  position: relative;
  padding: 26px 28px 38px;
  background:
    linear-gradient(180deg, rgba(240, 247, 250, 0.92), rgba(246, 248, 251, 0.45) 320px),
    radial-gradient(circle at 92% 24px, rgba(20, 184, 166, 0.12), transparent 280px);
}

.admin-page::before {
  content: "";
  position: absolute;
  inset: 0 0 auto;
  height: 220px;
  pointer-events: none;
  background: linear-gradient(135deg, rgba(13, 148, 136, 0.08), rgba(56, 189, 248, 0.04));
  border-bottom: 1px solid rgba(226, 232, 240, 0.72);
}

.admin-page > * {
  position: relative;
}

.admin-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  margin-bottom: 22px;
  padding: 24px;
  color: #f8fafc;
  background: linear-gradient(135deg, rgba(8, 35, 46, 0.96), rgba(12, 74, 82, 0.94)), #0f172a;
  border: 1px solid rgba(20, 184, 166, 0.22);
  border-radius: 12px;
  box-shadow: 0 18px 48px rgba(15, 23, 42, 0.18);
  overflow: hidden;
}

.admin-head::after {
  content: "";
  width: 180px;
  height: 180px;
  position: absolute;
  right: -54px;
  top: -70px;
  border: 1px solid rgba(45, 212, 191, 0.22);
  border-radius: 50%;
  box-shadow: inset 0 0 70px rgba(45, 212, 191, 0.08);
  pointer-events: none;
}

.admin-head h2 {
  margin: 0;
  font-size: 24px;
}

.admin-head p {
  max-width: 680px;
  margin: 8px 0 0;
  color: #b7c7d8;
  font-size: 13px;
}

.admin-head :deep(.el-button),
.admin-config-btn {
  border: 0;
  background: linear-gradient(135deg, #14b8a6, #0891b2);
  box-shadow: 0 10px 26px rgba(20, 184, 166, 0.26);
  color: #fff;
  padding: 10px 16px;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
}

.section {
  margin-bottom: 18px;
  padding: 18px;
  background: rgba(255, 255, 255, 0.82);
  border: 1px solid rgba(226, 232, 240, 0.86);
  border-radius: 12px;
  box-shadow: 0 8px 28px rgba(15, 23, 42, 0.06);
}

.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid #eef2f7;
}

.section-head h3 {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 0;
  color: #0f172a;
  font-size: 17px;
  font-weight: 700;
}

.section-head h3::before {
  content: "";
  width: 8px;
  height: 18px;
  border-radius: 999px;
  background: linear-gradient(180deg, #14b8a6, #0891b2);
}

.section-head span {
  color: #64748b;
  font-size: 12px;
}

.metric-card,
.health-card,
.service-card,
.maintenance-item {
  position: relative;
  min-height: 100%;
  background: linear-gradient(180deg, #ffffff, #fbfdff);
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  padding: 18px;
  box-shadow: 0 2px 12px rgba(15, 23, 42, 0.04);
  overflow: hidden;
  transition: transform 0.18s ease, box-shadow 0.18s ease, border-color 0.18s ease;
}

.metric-card::before,
.health-card::before,
.service-card::before,
.maintenance-item::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 3px;
  background: linear-gradient(180deg, #14b8a6, #0891b2);
}

.metric-card:hover,
.health-card:hover,
.service-card:hover,
.maintenance-item:hover {
  transform: translateY(-2px);
  border-color: rgba(13, 148, 136, 0.24);
  box-shadow: 0 14px 34px rgba(15, 23, 42, 0.08);
}

.metric-label {
  color: #64748b;
  font-size: 12px;
  font-weight: 700;
  margin-bottom: 8px;
}

.metric-value {
  color: #0f766e;
  font-size: 32px;
  font-weight: 800;
  line-height: 1;
  margin-bottom: 12px;
  font-variant-numeric: tabular-nums;
}

.metric-value--small {
  font-size: 22px;
  line-height: 1.25;
}

.metric-value.warning {
  color: #b7791f;
}

.metric-value.danger {
  color: #c24141;
}

.metric-sub,
.metric-lines {
  color: #64748b;
  font-size: 12px;
  line-height: 1.5;
  margin-top: 10px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.metric-lines {
  display: grid;
  gap: 6px;
  white-space: normal;
}

.metric-card :deep(.el-progress-bar__outer) {
  background: #e9eef5;
}

.metric-card :deep(.el-progress-bar__inner) {
  background: linear-gradient(90deg, #14b8a6, #0891b2);
}

.health-card {
  min-height: 94px;
}

.health-card--clickable {
  cursor: pointer;
}

.health-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  color: #1e293b;
  font-weight: 700;
}

.health-hint {
  color: #64748b;
  font-size: 12px;
  line-height: 1.5;
  margin-top: 12px;
}

.health-action {
  margin-top: 8px;
  padding: 0;
}

.source-table {
  margin-top: 16px;
  border: 1px solid #e6ebf2;
  border-radius: 10px;
  overflow: hidden;
}

.source-table :deep(.el-table__header-wrapper th) {
  background: #f8fafc;
  color: #475569;
  font-weight: 700;
}

.source-table :deep(.el-table__row:hover > td) {
  background: #f5fbfb;
}

.service-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  min-height: 122px;
}

.service-card h4,
.maintenance-item h4 {
  margin: 0;
  color: #0f172a;
  font-size: 15px;
  font-weight: 800;
}

.service-card p,
.maintenance-item p {
  margin: 8px 0 0;
  color: #64748b;
  font-size: 13px;
  line-height: 1.55;
}

.service-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.maintenance-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
}

.maintenance-item {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 14px;
  min-height: 158px;
}

.maintenance-item--danger {
  border-color: #fecaca;
  background: linear-gradient(180deg, #fff, #fffafa);
}

.maintenance-item--danger::before {
  background: linear-gradient(180deg, #ef4444, #f97316);
}

.history-card {
  margin-top: 16px;
  border: 1px solid #e6ebf2;
  border-radius: 10px;
  overflow: hidden;
}

.history-card :deep(.el-card__header) {
  padding: 14px 16px;
  color: #0f172a;
  font-weight: 800;
  background: #f8fafc;
}

.history-card :deep(.el-card__body) {
  padding: 0;
}

.db-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.db-card-sub {
  color: #64748b;
  font-size: 12px;
  font-weight: 400;
}

.db-path {
  margin: 0;
  padding: 10px 16px;
  color: #64748b;
  font-size: 11px;
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  background: #fbfdff;
  border-bottom: 1px solid #eef2f7;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.db-table :deep(.el-table__header-wrapper th) {
  background: #f8fafc;
  color: #475569;
  font-weight: 700;
}

.db-table-code {
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 12px;
  color: #475569;
  background: #f1f5f9;
  padding: 2px 6px;
  border-radius: 3px;
}

.db-table-count {
  color: #0f766e;
  font-weight: 800;
  font-size: 14px;
  font-variant-numeric: tabular-nums;
}

.db-bar-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.db-bar-row :deep(.el-progress-bar__outer) {
  background: #e9eef5;
  flex: 1;
}

.db-bar-row :deep(.el-progress-bar__inner) {
  background: linear-gradient(90deg, #14b8a6, #0891b2);
}

.db-bar-label {
  color: #64748b;
  font-size: 12px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  min-width: 42px;
  text-align: right;
}

.db-footer {
  padding: 10px 16px;
  color: #64748b;
  font-size: 12px;
  text-align: right;
  border-top: 1px solid #eef2f7;
  background: #f8fafc;
}

.db-footer strong {
  color: #0f766e;
  font-weight: 800;
}

.error-text {
  color: #c24141;
  font-size: 12px;
}

.ok-text {
  color: #0f766e;
  font-weight: 600;
}

.drawer-pagination {
  display: flex;
  justify-content: flex-end;
  margin-top: 14px;
}

.session-text {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 12px;
  line-height: 1.6;
}

@media (max-width: 900px) {
  .admin-page {
    padding: 18px;
  }

  .admin-head,
  .section-head,
  .service-card {
    align-items: flex-start;
    flex-direction: column;
  }

  .maintenance-grid,
  .db-tables {
    grid-template-columns: 1fr;
  }
}
</style>
