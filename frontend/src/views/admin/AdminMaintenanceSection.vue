<script setup lang="ts">
import type { DatabaseInfo } from '../../api/system'
import type { useScanStore } from '../../stores/useScanStore'

interface DbTable {
  name: string
  label: string
  count: number
}

defineProps<{
  scanStore: ReturnType<typeof useScanStore>
  clearing: boolean
  initializing: boolean
  updateChecking: boolean
  dbInfo: DatabaseInfo | null
  dbTables: DbTable[]
  dbTotalRows: number
}>()

const emit = defineEmits<{
  (e: 'scan'): void
  (e: 'clearLogs'): void
  (e: 'initDb'): void
  (e: 'checkUpdate'): void
}>()

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
</script>

<template>
  <div class="maintenance-grid">
    <div class="maintenance-item">
      <h4>扫描本机 Agent 数据</h4>
      <p>扫描 Skills、Agents、会话和记忆数据，并更新最近扫描记录。</p>
      <el-button type="primary" :loading="scanStore.scanning" @click="emit('scan')">{{ scanStore.scanning ? '扫描中...' : '开始扫描' }}</el-button>
    </div>
    <div class="maintenance-item">
      <h4>清理日志与历史</h4>
      <p>保留业务数据，只清理扫描历史、进化记录、聚类和使用记录。</p>
      <el-button type="warning" :loading="clearing" @click="emit('clearLogs')">清理日志与历史</el-button>
    </div>
    <div class="maintenance-item maintenance-item--danger">
      <h4>初始化数据库</h4>
      <p>清空 Skills、Agents、会话、社区缓存和历史记录，仅保留基础配置。</p>
      <el-button type="danger" :loading="initializing" @click="emit('initDb')">初始化数据库</el-button>
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
        <span class="db-card-sub">共 {{ dbTables.length }} 张表 · {{ dbTotalRows.toLocaleString() }} 条记录</span>
      </div>
    </template>
    <div class="db-path">{{ dbInfo.db_path }}</div>
    <el-table :data="dbTables" stripe size="small" class="db-table">
      <el-table-column prop="label" label="表名" min-width="160" />
      <el-table-column prop="name" label="英文表名" min-width="220">
        <template #default="{ row }">
          <code class="db-table-code">{{ row.name }}</code>
        </template>
      </el-table-column>
      <el-table-column prop="count" label="记录数" width="140" sortable align="right">
        <template #default="{ row }">
          <span class="db-table-count">{{ row.count.toLocaleString() }}</span>
        </template>
      </el-table-column>
    </el-table>
  </el-card>
</template>

<style scoped>
.maintenance-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
}

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
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 14px;
  min-height: 158px;
}

.maintenance-item::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 3px;
  background: linear-gradient(180deg, #14b8a6, #0891b2);
}

.maintenance-item:hover {
  transform: translateY(-2px);
  border-color: rgba(13, 148, 136, 0.24);
  box-shadow: 0 14px 34px rgba(15, 23, 42, 0.08);
}

.maintenance-item--danger {
  border-color: #fecaca;
  background: linear-gradient(180deg, #fff, #fffafa);
}

.maintenance-item--danger::before {
  background: linear-gradient(180deg, #ef4444, #f97316);
}

.maintenance-item h4 {
  margin: 0;
  color: #0f172a;
  font-size: 15px;
  font-weight: 800;
}

.maintenance-item p {
  margin: 0;
  color: #64748b;
  font-size: 13px;
  line-height: 1.55;
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

.history-card :deep(.el-card__body) { padding: 0; }

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

.error-text { color: #c24141; font-size: 12px; }
.ok-text { color: #0f766e; font-weight: 600; }

@media (max-width: 900px) {
  .maintenance-grid,
  .db-tables { grid-template-columns: 1fr; }
}
</style>
