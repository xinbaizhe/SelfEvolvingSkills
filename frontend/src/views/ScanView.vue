<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { useScanStore } from '../stores/useScanStore'
import { fetchSystemInfo } from '../api/admin'

interface SystemInfo {
  version?: string
  db_size_bytes?: number
  python?: string
}

const { t } = useI18n()
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
    ElMessage.success(t('vuln.scanView.scanComplete', { skills: result.skills_found, agents: result.agents_found, sessions: result.sessions_found }))
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
    <h2 style="margin-bottom: 16px">{{ t('vuln.scanView.title') }}</h2>

    <el-card style="margin-bottom: 20px">
      <template #header>{{ t('vuln.scanView.systemInfo') }}</template>
      <div v-if="systemInfo">
        <el-row :gutter="20">
          <el-col :span="8"><strong>{{ t('vuln.scanView.version') }}:</strong> {{ systemInfo.version }}</el-col>
          <el-col :span="8"><strong>{{ t('vuln.scanView.dbSize') }}:</strong> {{ ((systemInfo.db_size_bytes ?? 0) / 1024).toFixed(1) }} KB</el-col>
          <el-col :span="8"><strong>Python:</strong> {{ systemInfo.python?.split('\\n')[0] || '-' }}</el-col>
        </el-row>
      </div>
    </el-card>

    <div style="margin-bottom: 20px">
      <el-button type="primary" size="large" :loading="scanStore.scanning" @click="runScan">
        {{ scanStore.scanning ? t('vuln.common.scanning') : t('vuln.common.startScan') }}
      </el-button>
      <span style="margin-left: 12px; color: #909399; font-size: 13px">{{ t('vuln.scanView.hint') }}</span>
    </div>

    <el-card :header="t('vuln.scanView.history')">
      <el-table :data="scanStore.history" stripe max-height="400">
        <el-table-column prop="id" label="ID" width="60" />
        <el-table-column prop="scan_type" :label="t('vuln.scanView.colType')" width="80" />
        <el-table-column prop="status" :label="t('vuln.scanView.colStatus')" width="100">
          <template #default="{ row }">
            <el-tag :type="getStatusType(row.status)">{{ row.status }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="skills_found" :label="t('vuln.scanView.colSkills')" width="80" />
        <el-table-column prop="agents_found" :label="t('vuln.scanView.colAgents')" width="80" />
        <el-table-column prop="sessions_found" :label="t('vuln.scanView.colSessions')" width="80" />
        <el-table-column prop="conversations_analyzed" :label="t('vuln.scanView.colConversations')" width="80" />
        <el-table-column prop="memories_found" :label="t('vuln.scanView.colMemories')" width="80" />
        <el-table-column prop="started_at" :label="t('vuln.scanView.colStartedAt')" width="180" />
        <el-table-column prop="completed_at" :label="t('vuln.scanView.colCompletedAt')" width="180" />
        <el-table-column prop="errors" :label="t('vuln.scanView.colErrors')" min-width="200">
          <template #default="{ row }">
            <span v-if="row.errors" style="color: #f56c6c; font-size: 12px">{{ row.errors }}</span>
            <span v-else style="color: #67c23a">{{ t('vuln.scanView.none') }}</span>
          </template>
        </el-table-column>
      </el-table>
    </el-card>
  </div>
</template>
