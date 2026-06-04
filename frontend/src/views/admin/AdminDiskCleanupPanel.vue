<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  deleteDiskUsagePath,
  listDiskUsage,
  revealDiskUsagePath,
  scanDiskUsage,
  type DiskUsageEntry,
  type DiskUsageScanResult,
} from '../../api/system'
import { getErrorMessage } from '../../utils/error'
import { formatBytes } from '../../utils/format'

const props = defineProps<{ scanRequest?: number; detailsRequest?: number }>()
const visible = defineModel<boolean>({ default: false })
const scanning = ref(false)
const detailLoading = ref(false)
const deleting = ref(false)
const revealing = ref(false)
const panelNotice = ref('')
const usage = ref<DiskUsageScanResult | null>(null)
const selectedEntry = ref<DiskUsageEntry | null>(null)
const activeFilter = ref<'all' | 'logs'>('all')
const resultPage = ref(1)
const resultPageSize = ref(200)
const scanProgress = ref({
  status: 'idle',
  visited: 0,
  size_bytes: 0,
  current_path: '',
})
let unlistenProgress: UnlistenFn | null = null

const emit = defineEmits<{ (e: 'cleaned'): void }>()

const entries = computed(() => usage.value?.entries ?? [])

const treemapItems = computed(() =>
  entries.value
    .filter((item) => item.size_bytes > 0)
    .slice(0, 18)
    .map((item) => ({
      ...item,
      basis: `${Math.max(12, Math.min(100, (item.size_bytes / Math.max(usage.value?.total_size_bytes || 1, 1)) * 100))}%`,
    })),
)

const selectedPercent = computed(() => {
  if (!selectedEntry.value || !usage.value?.total_size_bytes) return '0.0'
  return ((selectedEntry.value.size_bytes / usage.value.total_size_bytes) * 100).toFixed(1)
})

const diskUsedPercent = computed(() => {
  if (!usage.value?.disk_total_bytes || usage.value.disk_total_bytes <= 0) return 0
  return Math.round(((usage.value.disk_used_bytes || 0) / usage.value.disk_total_bytes) * 100)
})

onMounted(async () => {
  unlistenProgress = await listen<typeof scanProgress.value>('disk-usage-progress', (event) => {
    scanProgress.value = event.payload
  })
})

onUnmounted(() => {
  unlistenProgress?.()
})

async function startScan() {
  visible.value = true
  await runScan(null)
}

watch(
  () => props.scanRequest,
  (value, oldValue) => {
    if (!value || value === oldValue) return
    void startScan()
  },
)

watch(
  () => props.detailsRequest,
  (value, oldValue) => {
    if (!value || value === oldValue) return
    if (!usage.value) {
      ElMessage.info('暂无空间明细，请先点击“分析磁盘”完成一次扫描')
    } else {
      visible.value = true
    }
  },
)

async function runScan(path?: string | null) {
  if (scanning.value) {
    panelNotice.value = '磁盘扫描正在进行，请等待本次扫描结束。'
    return
  }
  scanning.value = true
  panelNotice.value = '磁盘扫描正在进行，请等待本次扫描结束。'
  scanProgress.value = { status: 'running', visited: 0, size_bytes: 0, current_path: '' }
  selectedEntry.value = null
  try {
    const res = await scanDiskUsage(path ?? usage.value?.path ?? null)
    if (res.success && res.data) {
      usage.value = res.data
      activeFilter.value = 'all'
      if (res.data.error) {
        panelNotice.value = `扫描完成，但部分内容不可读：${res.data.error}`
      } else if (res.data.scan_limited) {
        panelNotice.value = '扫描完成，但部分目录文件数过多，结果可能不完整。'
      } else {
        panelNotice.value = '空间占用扫描完成。'
      }
    } else {
      const message = res.error || '空间占用扫描失败'
      if (message.includes('磁盘扫描正在进行')) {
        panelNotice.value = message
      } else {
        panelNotice.value = message
      }
    }
  } catch (e) {
    panelNotice.value = getErrorMessage(e, '空间占用扫描失败')
  } finally {
    scanning.value = false
  }
}

async function openCachedOrScan(path: string) {
  await loadFromCache(path, activeFilter.value)
}

async function loadFromCache(path?: string | null, filter: 'all' | 'logs' = activeFilter.value, page = resultPage.value) {
  if (detailLoading.value) return
  detailLoading.value = true
  try {
    const res = await listDiskUsage(path ?? usage.value?.path ?? null, filter, page, resultPageSize.value)
    if (res.success && res.data) {
      usage.value = res.data
      selectedEntry.value = null
      visible.value = true
      activeFilter.value = filter
      resultPage.value = res.data.page || page
      panelNotice.value = filter === 'logs' ? '已筛选日志目录。' : '已加载扫描缓存。'
    } else {
      panelNotice.value = res.error || '暂无空间明细，请先分析磁盘'
      if (!usage.value) ElMessage.info(panelNotice.value)
    }
  } catch (e) {
    panelNotice.value = getErrorMessage(e, '读取空间明细失败')
  } finally {
    detailLoading.value = false
  }
}

async function changeFilter(value: string | number | boolean) {
  const next = value === 'logs' ? 'logs' : 'all'
  resultPage.value = 1
  await loadFromCache(next === 'logs' ? null : usage.value?.path ?? null, next, 1)
}

async function changeResultPage(page: number) {
  await loadFromCache(activeFilter.value === 'logs' ? null : usage.value?.path ?? null, activeFilter.value, page)
}

function selectEntry(row: DiskUsageEntry) {
  selectedEntry.value = row
}

async function revealEntry(row?: DiskUsageEntry | null) {
  const target = row ?? selectedEntry.value
  if (!target) {
    ElMessage.warning('请选择要打开的文件或目录')
    return
  }
  selectedEntry.value = target
  revealing.value = true
  try {
    const res = await revealDiskUsagePath(target.path)
    if (res.success && res.data?.opened) {
      ElMessage.success(res.data.message || '已打开所在目录')
    } else {
      ElMessage.error(res.data?.message || res.error || '打开所在目录失败')
    }
  } catch (e) {
    ElMessage.error(getErrorMessage(e, '打开所在目录失败'))
  } finally {
    revealing.value = false
  }
}

async function revealEntryFromContextMenu(event: MouseEvent, row: DiskUsageEntry) {
  event.preventDefault()
  await revealEntry(row)
}

async function enterEntry(row: DiskUsageEntry) {
  selectedEntry.value = row
  if (row.is_dir) {
    await openCachedOrScan(row.path)
  }
}

async function goParent() {
  if (!usage.value?.parent) return
  await openCachedOrScan(usage.value.parent)
}

async function handleDeleteSelected() {
  const target = selectedEntry.value
  if (!target) {
    ElMessage.warning('请选择要清理的文件或目录')
    return
  }

  try {
    await ElMessageBox.confirm(
      `将删除「${target.name}」，预计释放 ${formatBytes(target.size_bytes)}。请确认该路径可以清理。`,
      '确认清理选中路径',
      { confirmButtonText: '确认清理', cancelButtonText: '取消', type: 'warning' },
    )
  } catch {
    return
  }

  deleting.value = true
  try {
    const res = await deleteDiskUsagePath(target.path)
    if (res.success && res.data?.deleted) {
      const message = `${res.data.message}，约释放 ${formatBytes(res.data.size_bytes)}`
      if (res.data.partial) {
        ElMessage.warning(message)
        panelNotice.value = `${message}。跳过 ${Number(res.data.failed_count || 0).toLocaleString()} 项，通常是权限不足或文件正在使用。当前扫描结果可能已过期，建议重新分析磁盘。`
      } else {
        ElMessage.success(message)
        panelNotice.value = '清理完成。当前扫描结果可能已过期，建议重新分析磁盘。'
      }
      emit('cleaned')
    } else {
      ElMessage.error(res.data?.message || res.error || '清理失败')
    }
  } catch (e) {
    ElMessage.error(getErrorMessage(e, '清理失败'))
  } finally {
    deleting.value = false
  }
}

defineExpose({ startScan })
</script>

<template>
  <el-drawer
    v-model="visible"
    title="系统盘空间分析"
    size="78%"
    append-to-body
    :lock-scroll="false"
    :z-index="3000"
  >
    <el-alert
      v-if="panelNotice"
      class="panel-notice"
      :title="panelNotice"
      :type="panelNotice.includes('失败') || panelNotice.includes('不可读') ? 'warning' : 'info'"
      show-icon
      :closable="true"
      @close="panelNotice = ''"
    />

    <div class="usage-toolbar">
      <div class="usage-path">
        <div class="metric-strip">
          <div>
            <span>已统计占用</span>
            <b>{{ formatBytes(usage?.total_size_bytes) }}</b>
          </div>
          <div>
            <span>文件数量</span>
            <b>{{ Number(usage?.file_count || 0).toLocaleString() }}</b>
          </div>
          <div v-if="usage?.disk_total_bytes">
            <span>文件系统已用</span>
            <b>{{ formatBytes(usage.disk_used_bytes) }} / {{ formatBytes(usage.disk_total_bytes) }}</b>
          </div>
        </div>
        <div class="usage-current" :title="usage?.path">{{ activeFilter === 'logs' ? '日志目录筛选结果' : usage?.path || '系统盘' }}</div>
        <div v-if="usage?.disk_total_bytes" class="disk-capacity-bar">
          <span :style="{ width: `${diskUsedPercent}%` }"></span>
        </div>
        <div v-if="usage?.scan_limited" class="usage-warning">部分目录已截断，统计值小于真实占用。</div>
        <div class="usage-scan-time">扫描结果保存在当前页面内存中；筛选不会重新扫描磁盘。</div>
      </div>
      <div class="usage-actions">
        <el-segmented
          v-model="activeFilter"
          :disabled="detailLoading || scanning"
          :options="[
            { label: '全部', value: 'all' },
            { label: '日志目录', value: 'logs' },
          ]"
          @change="changeFilter"
        />
        <el-button :disabled="!usage?.parent" @click="goParent">上一级</el-button>
        <el-button :loading="scanning" @click="runScan()">重新扫描</el-button>
        <el-button
          :disabled="!selectedEntry"
          :loading="revealing"
          @click="revealEntry()"
        >
          打开所在目录
        </el-button>
        <el-button
          type="warning"
          :disabled="!selectedEntry"
          :loading="deleting"
          @click="handleDeleteSelected"
        >
          清理选中
        </el-button>
      </div>
    </div>

    <div v-if="scanning || scanProgress.status === 'running'" class="scan-progress">
      <div class="scan-progress__head">
        <b>正在扫描目录</b>
        <span>{{ Number(scanProgress.visited || 0).toLocaleString() }} 个节点 · 已统计 {{ formatBytes(scanProgress.size_bytes) }}</span>
      </div>
      <el-progress :percentage="100" :indeterminate="true" :duration="1.2" />
      <div class="scan-progress__path" :title="scanProgress.current_path">{{ scanProgress.current_path || '准备扫描...' }}</div>
    </div>

    <div class="selection-bar" v-if="selectedEntry">
      <b>{{ selectedEntry.name }}</b>
      <span>{{ formatBytes(selectedEntry.size_bytes) }} · 占当前层级 {{ selectedPercent }}%</span>
      <code>{{ selectedEntry.path }}</code>
    </div>

    <div class="treemap" v-loading="scanning">
      <button
        v-for="item in treemapItems"
        :key="item.path"
        class="tile"
        :class="{ selected: selectedEntry?.path === item.path, folder: item.is_dir }"
        :style="{ flexBasis: item.basis }"
        :title="`${item.path}\n${formatBytes(item.size_bytes)}`"
        @click="selectEntry(item)"
        @contextmenu="revealEntryFromContextMenu($event, item)"
        @dblclick="enterEntry(item)"
      >
        <span class="tile-name">{{ item.name }}</span>
        <span class="tile-size">{{ formatBytes(item.size_bytes) }}</span>
      </button>
      <div v-if="!scanning && treemapItems.length === 0" class="empty-map">
        暂无扫描结果，请点击“重新扫描”或外部“分析磁盘”按钮。
      </div>
    </div>

    <el-table
      :data="entries"
      v-loading="scanning || detailLoading"
      stripe
      height="calc(100vh - 470px)"
      empty-text="暂无扫描结果，请先分析磁盘"
      highlight-current-row
      @row-click="selectEntry"
      @row-contextmenu="revealEntryFromContextMenu"
      @row-dblclick="enterEntry"
    >
      <el-table-column label="名称" min-width="260">
        <template #default="{ row }">
          <button class="entry-link" :class="{ folder: row.is_dir }" @click.stop="enterEntry(row)">
            {{ row.is_dir ? '目录' : '文件' }} · {{ row.name }}
          </button>
        </template>
      </el-table-column>
      <el-table-column label="大小" width="140" sortable>
        <template #default="{ row }">{{ formatBytes(row.size_bytes) }}</template>
      </el-table-column>
      <el-table-column label="文件数" width="110" align="right">
        <template #default="{ row }">{{ Number(row.file_count || 0).toLocaleString() }}</template>
      </el-table-column>
      <el-table-column label="路径" min-width="320" show-overflow-tooltip>
        <template #default="{ row }">{{ row.path }}</template>
      </el-table-column>
      <el-table-column label="状态" width="130">
        <template #default="{ row }">
          <span v-if="row.scan_limited" class="warning-text">结果已截断</span>
          <span v-else-if="row.error" class="warning-text">部分不可读</span>
          <span v-else class="ok-text">已统计</span>
        </template>
      </el-table-column>
    </el-table>

    <div v-if="activeFilter === 'logs' && (usage?.entry_total || 0) > resultPageSize" class="result-pagination">
      <span>日志目录共 {{ Number(usage?.entry_total || 0).toLocaleString() }} 条，每页 {{ resultPageSize }} 条</span>
      <el-pagination
        v-model:current-page="resultPage"
        :page-size="resultPageSize"
        :total="usage?.entry_total || 0"
        layout="prev, pager, next"
        size="small"
        :disabled="detailLoading"
        @current-change="changeResultPage"
      />
    </div>
  </el-drawer>
</template>

<style scoped>
.usage-toolbar {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 12px;
  padding: 16px;
  background: linear-gradient(180deg, #ffffff, #f8fafc);
  border: 1px solid #e2e8f0;
  border-radius: 12px;
  box-shadow: 0 8px 26px rgba(15, 23, 42, 0.06);
}

.panel-notice {
  margin-bottom: 12px;
}

.metric-strip {
  display: grid;
  grid-template-columns: repeat(3, minmax(130px, max-content));
  gap: 12px;
}

.metric-strip div {
  padding: 10px 12px;
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
}

.metric-strip span {
  display: block;
  margin-bottom: 4px;
  color: #64748b;
  font-size: 11px;
  font-weight: 700;
}

.metric-strip b {
  color: #0f172a;
  font-size: 15px;
}

.usage-current {
  max-width: 720px;
  margin-top: 6px;
  color: #64748b;
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.usage-scan-time {
  margin-top: 4px;
  color: #64748b;
  font-size: 12px;
}

.usage-warning {
  margin-top: 4px;
  color: #b7791f;
  font-size: 12px;
  font-weight: 700;
}

.usage-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  flex-wrap: wrap;
  justify-content: flex-end;
  max-width: 420px;
}

.disk-capacity-bar {
  width: min(520px, 100%);
  height: 8px;
  margin-top: 8px;
  overflow: hidden;
  border-radius: 999px;
  background: #e2e8f0;
}

.disk-capacity-bar span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, #14b8a6, #0891b2);
}

.scan-progress {
  margin-bottom: 12px;
  padding: 12px 14px;
  background: #f0fdfa;
  border: 1px solid #99f6e4;
  border-radius: 10px;
}

.scan-progress__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 8px;
  color: #0f766e;
  font-size: 13px;
}

.scan-progress__path {
  margin-top: 8px;
  color: #475569;
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.result-pagination {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 12px;
  color: #64748b;
  font-size: 12px;
}

.selection-bar {
  display: grid;
  grid-template-columns: minmax(140px, auto) auto minmax(0, 1fr);
  gap: 12px;
  align-items: center;
  margin-bottom: 12px;
  padding: 10px 12px;
  color: #334155;
  background: #ecfdf5;
  border: 1px solid #bbf7d0;
  border-radius: 8px;
  font-size: 12px;
}

.selection-bar code {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.treemap {
  display: flex;
  flex-wrap: wrap;
  align-content: flex-start;
  gap: 6px;
  min-height: 190px;
  max-height: 240px;
  margin-bottom: 14px;
  padding: 8px;
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: 12px;
  overflow: auto;
}

.tile {
  min-width: 120px;
  min-height: 72px;
  flex-grow: 1;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 8px;
  padding: 10px;
  text-align: left;
  color: #0f172a;
  background: #dbeafe;
  border: 1px solid #bfdbfe;
  border-radius: 8px;
  cursor: pointer;
}

.tile.folder {
  background: #ccfbf1;
  border-color: #99f6e4;
}

.tile.selected {
  outline: 2px solid #0d9488;
  outline-offset: 1px;
}

.tile-name {
  font-size: 13px;
  font-weight: 800;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tile-size {
  color: #475569;
  font-size: 12px;
  font-weight: 700;
}

.empty-map {
  margin: auto;
  color: #94a3b8;
}

.entry-link {
  border: 0;
  padding: 0;
  color: #334155;
  background: transparent;
  font: inherit;
  font-weight: 700;
  cursor: pointer;
}

.entry-link.folder {
  color: #0f766e;
}

.warning-text { color: #b7791f; font-weight: 600; }
.ok-text { color: #0f766e; font-weight: 600; }

@media (max-width: 900px) {
  .usage-toolbar,
  .selection-bar {
    align-items: flex-start;
    grid-template-columns: 1fr;
    flex-direction: column;
  }

  .usage-actions {
    width: 100%;
    flex-wrap: wrap;
  }
}
</style>
