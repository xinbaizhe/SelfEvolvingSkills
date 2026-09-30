<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { check } from '@tauri-apps/plugin-updater'
import { useRouter } from 'vue-router'
import { useScanStore } from '../../stores/useScanStore'
import { fetchLlmConfig, fetchSystemInfo } from '../../api/admin'
import { detectSources, fetchSources, type SourceConfig } from '../../api/scan'
import { fetchSummary, type SummaryStats } from '../../api/stats'
import {
  getDatabaseInfo,
  getSystemMonitor,
  clearLogs,
  initializeDatabase,
  type DatabaseInfo,
  type SystemMonitor,
} from '../../api/system'
import { formatBytes, formatUptime } from '../../utils/format'
import { describeError } from '../../utils/error'
import AdminDiskCleanupPanel from './AdminDiskCleanupPanel.vue'
import AdminSessionsPanel from './AdminSessionsPanel.vue'
import AdminMaintenanceSection from './AdminMaintenanceSection.vue'
import AdminServicePanel from './AdminServicePanel.vue'

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

const { t, te } = useI18n()
const router = useRouter()
const scanStore = useScanStore()
const systemInfo = ref<SystemInfo | null>(null)
const summary = ref<SummaryStats | null>(null)
const monitor = ref<SystemMonitor | null>(null)
const dbInfo = ref<DatabaseInfo | null>(null)
const sources = ref<SourceConfig[]>([])
const llmConfig = ref<LlmConfig | null>(null)
const updateChecking = ref(false)
const detecting = ref(false)
const clearing = ref(false)
const initializing = ref(false)
const diskUsagePanelVisible = ref(false)
const diskUsageScanRequest = ref(0)
const diskUsageDetailsRequest = ref(0)
const sessionsPanelVisible = ref(false)
const sessionsOpenRequest = ref(0)
let refreshTimer: ReturnType<typeof setInterval> | null = null

// DB table name -> admin.dashboard.table.<camelCase>
function tableLabel(name: string): string {
  const key = `admin.dashboard.table.${name.replace(/_([a-z])/g, (_, c: string) => c.toUpperCase())}`
  return te(key) ? t(key) : name
}

const dbTotalRows = computed(() => dbTables.value.reduce((sum, t) => sum + t.count, 0))

const dbTables = computed(() => {
  if (!dbInfo.value) return []
  return Object.entries(dbInfo.value.table_counts)
    .map(([name, count]) => ({
      name,
      label: tableLabel(name),
      count,
    }))
    .sort((a, b) => b.count - a.count)
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
    label: t('admin.dashboard.healthSources'),
    value: `${sourceStats.value.available}/${sourceStats.value.total}`,
    hint: t('admin.dashboard.enabledHint', { n: sourceStats.value.enabled }),
    type: sourceStats.value.available > 0 ? 'success' : 'warning',
  },
  {
    key: 'sessions',
    label: t('admin.dashboard.healthSessions'),
    value: String(summary.value?.total_sessions ?? systemInfo.value?.total_sessions ?? 0),
    hint: t('admin.dashboard.sessionsHint'),
    type: (summary.value?.total_sessions ?? 0) > 0 ? 'success' : 'info',
  },
  {
    key: 'latest_scan',
    label: t('admin.dashboard.latestScan'),
    value: latestScan.value ? getStatusLabel(latestScan.value.status) : t('admin.dashboard.noneYet'),
    hint: latestScan.value?.completed_at || latestScan.value?.started_at || t('admin.dashboard.noScanYet'),
    type: latestScan.value ? getStatusType(latestScan.value.status) : 'info',
  },
  {
    key: 'path_records',
    label: t('admin.dashboard.pathRecords'),
    value: String(sourceStats.value.records),
    hint: t('admin.dashboard.fromSourceDetection'),
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
    ElMessage.success(t('admin.dashboard.scanComplete', { skills: result.skills_found, agents: result.agents_found, sessions: result.sessions_found }))
    await Promise.all([scanStore.loadHistory({ size: 10 }), refreshStats(), refreshMonitor(), refreshSources()])
  }
}

async function runDetectSources() {
  detecting.value = true
  try {
    const res = await detectSources()
    if (res.success) {
      sources.value = res.data || []
      ElMessage.success(t('admin.dashboard.detectDone'))
      await refreshStats()
    } else {
      ElMessage.error(res.error || t('admin.dashboard.detectFailed'))
    }
  } finally {
    detecting.value = false
  }
}

function openResourceConfig() {
  router.push({ name: 'resourcesConfig', query: { tab: 'model' } }).catch(() => {
    window.location.hash = '#/resources?tab=model'
  })
}

function openDiskUsagePanel() {
  diskUsageDetailsRequest.value += 1
}

function startDiskUsageScan() {
  diskUsagePanelVisible.value = true
  diskUsageScanRequest.value += 1
}

function handleHealthCardClick(key: string) {
  if (key === 'sessions') {
    const total = summary.value?.total_sessions ?? systemInfo.value?.total_sessions ?? 0
    if (total === 0) {
      ElMessage.warning(t('admin.dashboard.sessionsEmpty'))
      return
    }
    sessionsPanelVisible.value = true
    sessionsOpenRequest.value += 1
  }
}

async function checkForAppUpdate() {
  updateChecking.value = true
  try {
    const update = await check({ timeout: 8000 })
    if (!update) {
      ElMessage.success(t('admin.dashboard.upToDate'))
      return
    }

    const action = await ElMessageBox.confirm(
      t('app.update.message', { version: update.version }),
      t('app.update.title'),
      { confirmButtonText: t('app.update.confirm'), cancelButtonText: t('app.update.cancel'), type: 'info' }
    ).catch(() => 'cancel')

    if (action === 'confirm') {
      await update.downloadAndInstall()
      ElMessage.success(t('app.update.installed'))
    }
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e)
    if (msg.includes('signature') || msg.includes('verify')) {
      ElMessage.error(t('admin.dashboard.signatureFailed'))
    } else if (msg.includes('network') || msg.includes('timeout') || msg.includes('fetch')) {
      ElMessage.error(t('admin.dashboard.networkFailed'))
    } else if (msg.includes('404')) {
      ElMessage.error(t('admin.dashboard.latestJsonMissing'))
    } else {
      ElMessage.error(t('admin.dashboard.checkUpdateFailed', { error: msg }))
    }
  } finally {
    updateChecking.value = false
  }
}

function cpuPercent() {
  return Number.parseFloat(monitor.value?.cpu_usage_percent ?? '0')
}

function memoryPercent() {
  return Number.parseFloat(monitor.value?.memory.usage_percent ?? '0')
}

function getStatusType(status: string) {
  if (status === 'completed') return 'success'
  if (status === 'failed') return 'danger'
  if (status === 'running') return 'warning'
  return 'info'
}

function getStatusLabel(status: string) {
  const key = `admin.common.status.${status}`
  return te(key) ? t(key) : status
}

async function handleClearLogs() {
  try {
    await ElMessageBox.confirm(
      t('admin.dashboard.cleanConfirm'),
      t('admin.dashboard.cleanConfirmTitle'),
      { confirmButtonText: t('admin.dashboard.cleanConfirmBtn'), cancelButtonText: t('common.cancel'), type: 'warning' },
    )
  } catch {
    return
  }
  clearing.value = true
  try {
    const res = await clearLogs()
    if (res.success) {
      ElMessage.success(describeError(res.data?.message, t('admin.dashboard.cleanDone')))
      await Promise.all([refreshStats(), refreshMonitor(), scanStore.loadHistory({ size: 10 })])
    } else {
      ElMessage.error(res.error || t('admin.dashboard.cleanFailed'))
    }
  } catch {
    ElMessage.error(t('admin.dashboard.cleanFailed'))
  } finally {
    clearing.value = false
  }
}

async function handleInitializeDatabase() {
  try {
    await ElMessageBox.confirm(
      t('admin.dashboard.initConfirm'),
      t('admin.dashboard.initConfirmTitle'),
      { confirmButtonText: t('admin.dashboard.initConfirmBtn'), cancelButtonText: t('common.cancel'), type: 'error' },
    )
  } catch {
    return
  }
  initializing.value = true
  try {
    const res = await initializeDatabase()
    if (res.success) {
      ElMessage.success(describeError(res.data?.message, t('admin.dashboard.dbInitialized')))
      await Promise.all([refreshStats(), refreshMonitor(), refreshSources(), scanStore.loadHistory({ size: 10 })])
    } else {
      ElMessage.error(res.error || t('admin.dashboard.initFailed'))
    }
  } catch {
    ElMessage.error(t('admin.dashboard.initFailed'))
  } finally {
    initializing.value = false
  }
}
</script>

<template>
  <div class="admin-page">
    <div class="admin-head">
      <div>
        <h2>{{ t('admin.dashboard.title') }}</h2>
        <p>{{ t('admin.dashboard.subtitle') }}</p>
      </div>
      <button class="admin-config-btn" @click="openResourceConfig">{{ t('admin.dashboard.systemConfig') }}</button>
    </div>

    <section class="section">
      <div class="section-head">
        <h3>{{ t('admin.dashboard.runtimeOverview') }}</h3>
        <span>{{ t('admin.dashboard.refreshHint') }}</span>
      </div>
      <el-row :gutter="16">
        <el-col :xs="24" :sm="12" :lg="6">
          <div class="metric-card">
            <div class="metric-label">{{ t('admin.dashboard.cpuUsage') }}</div>
            <div class="metric-value" :class="{ warning: cpuPercent() > 80, danger: cpuPercent() > 90 }">{{ monitor?.cpu_usage_percent ?? '0.0' }}%</div>
            <el-progress :percentage="cpuPercent()" :stroke-width="6" :show-text="false" />
            <div class="metric-sub">{{ monitor?.cpu_brand || t('admin.dashboard.cpuUnavailable') }}</div>
          </div>
        </el-col>
        <el-col :xs="24" :sm="12" :lg="6">
          <div class="metric-card">
            <div class="metric-label">{{ t('admin.dashboard.memory') }}</div>
            <div class="metric-value">{{ monitor?.memory.usage_percent ?? '0.0' }}%</div>
            <el-progress :percentage="memoryPercent()" :stroke-width="6" :show-text="false" :color="memoryPercent() > 80 ? '#f56c6c' : '#0d9488'" />
            <div class="metric-sub">{{ monitor?.memory.used_mb ?? 0 }} / {{ monitor?.memory.total_mb ?? 0 }} MB</div>
          </div>
        </el-col>
        <el-col :xs="24" :sm="12" :lg="6">
          <div
            class="metric-card metric-card--clickable"
            role="button"
            tabindex="0"
            @click="openDiskUsagePanel"
            @keydown.enter="openDiskUsagePanel"
          >
            <div class="metric-label">{{ t('admin.dashboard.disk') }}</div>
            <div class="metric-value">{{ monitor?.disks[0]?.usage_pct ?? '--' }}%</div>
            <el-progress v-if="monitor?.disks[0]" :percentage="monitor.disks[0].usage_pct" :stroke-width="6" :show-text="false" />
            <div class="metric-sub" v-if="monitor?.disks[0]">{{ t('admin.dashboard.diskAvailable', { available: monitor.disks[0].available_gb, total: monitor.disks[0].total_gb }) }}</div>
            <div class="metric-sub" v-else>{{ t('admin.dashboard.diskUnavailable') }}</div>
            <div class="metric-action">{{ t('admin.dashboard.diskAction') }}</div>
            <el-button class="metric-inline-button" size="small" type="primary" text @click.stop="startDiskUsageScan">
              {{ t('admin.dashboard.analyzeDisk') }}
            </el-button>
          </div>
        </el-col>
        <el-col :xs="24" :sm="12" :lg="6">
          <div class="metric-card">
            <div class="metric-label">{{ t('admin.dashboard.uptime') }}</div>
            <div class="metric-value metric-value--small">{{ formatUptime(monitor?.uptime_seconds) }}</div>
            <div class="metric-lines">
              <span>{{ t('admin.dashboard.databaseSize', { size: dbInfo?.db_size_mb ?? '0.00' }) }}</span>
              <span>{{ t('admin.dashboard.walSize', { size: dbInfo?.wal_size_mb ?? '0.00' }) }}</span>
            </div>
          </div>
        </el-col>
      </el-row>
    </section>

    <section class="section">
      <div class="section-head">
        <h3>{{ t('admin.dashboard.dataHealth') }}</h3>
        <el-button size="small" :loading="detecting" @click="runDetectSources">{{ t('admin.dashboard.detectSources') }}</el-button>
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

      <el-table :data="sources" stripe class="source-table" :empty-text="t('admin.dashboard.sourcesEmpty')">
        <el-table-column prop="agent_name" label="Agent" width="150" />
        <el-table-column :label="t('admin.common.statusLabel')" width="120">
          <template #default="{ row }">
            <el-tag :type="row.is_available ? 'success' : 'warning'">{{ row.is_available ? t('admin.common.available') : t('admin.common.unavailable') }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column :label="t('admin.dashboard.enabledColumn')" width="90">
          <template #default="{ row }">
            <el-tag :type="row.is_enabled ? 'success' : 'info'">{{ row.is_enabled ? t('admin.common.enabled') : t('admin.common.disabled') }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="record_count" :label="t('admin.common.records')" width="100" />
        <el-table-column prop="last_scan_at" :label="t('admin.dashboard.latestScan')" width="170">
          <template #default="{ row }">{{ row.last_scan_at || '-' }}</template>
        </el-table-column>
        <el-table-column prop="detected_path" :label="t('admin.dashboard.detectedPath')" min-width="260" show-overflow-tooltip>
          <template #default="{ row }">{{ row.detected_path || '-' }}</template>
        </el-table-column>
      </el-table>
    </section>

    <AdminServicePanel :llm-config="llmConfig" @config="openResourceConfig" />

    <section class="section">
      <div class="section-head">
        <div>
          <h3>{{ t('admin.dashboard.maintenanceTitle') }}</h3>
          <span>{{ t('admin.dashboard.maintenanceHint') }}</span>
        </div>
        <el-button :loading="updateChecking" @click="checkForAppUpdate" class="update-btn">
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none"><path d="M13 6.5L8 11.5L3 6.5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>
          {{ t('admin.dashboard.checkUpdate') }}
        </el-button>
      </div>
      <AdminMaintenanceSection
        :scan-store="scanStore"
        :clearing="clearing"
        :initializing="initializing"
        :update-checking="updateChecking"
        :db-info="dbInfo"
        :db-tables="dbTables"
        :db-total-rows="dbTotalRows"
        @scan="runScan"
        @clear-logs="handleClearLogs"
        @init-db="handleInitializeDatabase"
        @check-update="checkForAppUpdate"
      />
    </section>

    <AdminSessionsPanel v-model="sessionsPanelVisible" :open-request="sessionsOpenRequest" />
    <AdminDiskCleanupPanel
      v-model="diskUsagePanelVisible"
      :scan-request="diskUsageScanRequest"
      :details-request="diskUsageDetailsRequest"
      @cleaned="refreshMonitor"
    />
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
.health-card {
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
.health-card::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 3px;
  background: linear-gradient(180deg, #14b8a6, #0891b2);
}

.metric-card:hover,
.health-card:hover {
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

.metric-card--clickable {
  cursor: pointer;
}

.metric-card--clickable:focus-visible,
.health-card--clickable:focus-visible {
  outline: 2px solid #0d9488;
  outline-offset: 3px;
}

.metric-action {
  margin-top: 8px;
  color: #0d9488;
  font-size: 12px;
  font-weight: 700;
}

.metric-inline-button {
  margin-top: 8px;
  padding: 0;
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

@media (max-width: 900px) {
  .admin-page {
    padding: 18px;
  }

  .admin-head,
  .section-head {
    align-items: flex-start;
    flex-direction: column;
  }
}
</style>
