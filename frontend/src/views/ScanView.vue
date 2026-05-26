<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { useScanStore } from '../stores/useScanStore'
import { fetchSystemInfo } from '../api/admin'

interface SystemInfo {
  version?: string
  db_size_bytes?: number
  python?: string
}

const scanStore = useScanStore()
const systemInfo = ref<SystemInfo | null>(null)
const currentPage = ref(1)

onMounted(() => {
  scanStore.loadHistory({ size: 20 })
  fetchSystemInfo().then((res) => { if (res.success) systemInfo.value = res.data as SystemInfo })
})

async function runScan() {
  const result = await scanStore.runScan()
  if (result) {
    ElMessage.success(`扫描完成: ${result.skills_found} 技能, ${result.agents_found} Agent, ${result.sessions_found} 会话`)
    await scanStore.loadHistory({ size: 20 })
  }
}

function getStatusType(status: string) {
  if (status === 'completed') return 'success'
  if (status === 'failed') return 'danger'
  return 'warning'
}
</script>

<template>
  <div>
    <h2 style="margin-bottom: 16px">扫描管理</h2>

    <el-card style="margin-bottom: 20px">
      <template #header>系统信息</template>
      <div v-if="systemInfo">
        <el-row :gutter="20">
          <el-col :span="8"><strong>版本:</strong> {{ systemInfo.version }}</el-col>
          <el-col :span="8"><strong>数据库大小:</strong> {{ ((systemInfo.db_size_bytes ?? 0) / 1024).toFixed(1) }} KB</el-col>
          <el-col :span="8"><strong>Python:</strong> {{ systemInfo.python?.split('\\n')[0] || '-' }}</el-col>
        </el-row>
      </div>
    </el-card>

    <div style="margin-bottom: 20px">
      <el-button type="primary" size="large" :loading="scanStore.scanning" @click="runScan">
        {{ scanStore.scanning ? '扫描中...' : '开始扫描' }}
      </el-button>
      <span style="margin-left: 12px; color: #909399; font-size: 13px">扫描本机的 Claude Code 技能、Agent、会话数据</span>
    </div>

    <el-card header="扫描历史">
      <el-table :data="scanStore.history" stripe max-height="400">
        <el-table-column prop="id" label="ID" width="60" />
        <el-table-column prop="scan_type" label="类型" width="80" />
        <el-table-column prop="status" label="状态" width="100">
          <template #default="{ row }">
            <el-tag :type="getStatusType(row.status)">{{ row.status }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="skills_found" label="技能数" width="80" />
        <el-table-column prop="agents_found" label="Agent数" width="80" />
        <el-table-column prop="sessions_found" label="会话数" width="80" />
        <el-table-column prop="conversations_analyzed" label="分析数" width="80" />
        <el-table-column prop="memories_found" label="记忆数" width="80" />
        <el-table-column prop="started_at" label="开始时间" width="180" />
        <el-table-column prop="completed_at" label="完成时间" width="180" />
        <el-table-column prop="errors" label="错误" min-width="200">
          <template #default="{ row }">
            <span v-if="row.errors" style="color: #f56c6c; font-size: 12px">{{ row.errors }}</span>
            <span v-else style="color: #67c23a">无</span>
          </template>
        </el-table-column>
      </el-table>
    </el-card>
  </div>
</template>
