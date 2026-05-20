<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { fetchSessions, fetchSessionDetail } from '../api/stats'

const sessions = ref<any[]>([])
const total = ref(0)
const loading = ref(false)
const currentPage = ref(1)

const detailVisible = ref(false)
const currentSession = ref<any>(null)

onMounted(() => loadSessions())

async function loadSessions() {
  loading.value = true
  try {
    const res = await fetchSessions({ page: currentPage.value, size: 50 })
    if (res.success && res.data) {
      sessions.value = res.data.items
      total.value = res.data.total
    }
  } finally {
    loading.value = false
  }
}

async function showDetail(sessionId: string) {
  detailVisible.value = true
  const res = await fetchSessionDetail(sessionId)
  if (res.success) currentSession.value = res.data
}

function getProjectName(cwd: string | null) {
  if (!cwd) return '-'
  const parts = cwd.replace(/\\/g, '/').split('/')
  return parts[parts.length - 1] || cwd
}
</script>

<template>
  <div>
    <h2 style="margin-bottom: 16px">Conversation History</h2>

    <el-table :data="sessions" v-loading="loading" stripe max-height="calc(100vh - 180px)">
      <el-table-column type="index" label="#" width="50" />
      <el-table-column label="Project" width="200">
        <template #default="{ row }">{{ getProjectName(row.cwd) }}</template>
      </el-table-column>
      <el-table-column prop="entrypoint" label="Entry" width="120" />
      <el-table-column prop="version" label="Version" width="100" />
      <el-table-column prop="message_count" label="Messages" width="80" />
      <el-table-column prop="started_at" label="Time" width="180" />
      <el-table-column label="Actions" width="100">
        <template #default="{ row }">
          <el-button size="small" @click="showDetail(row.session_id)">Details</el-button>
        </template>
      </el-table-column>
    </el-table>

    <div style="margin-top: 16px; text-align: right">
      <el-pagination v-model:current-page="currentPage" :total="total" :page-size="50" layout="prev, pager, next" @current-change="loadSessions" />
    </div>

    <el-dialog v-model="detailVisible" title="Session Details" width="600px">
      <div v-if="currentSession">
        <el-descriptions :column="1" border size="small">
          <el-descriptions-item label="Session ID">{{ currentSession.session_id }}</el-descriptions-item>
          <el-descriptions-item label="Project">{{ currentSession.project_name }}</el-descriptions-item>
          <el-descriptions-item label="Working Directory">{{ currentSession.cwd }}</el-descriptions-item>
          <el-descriptions-item label="Entry">{{ currentSession.entrypoint }}</el-descriptions-item>
          <el-descriptions-item label="Messages">{{ currentSession.message_count }}</el-descriptions-item>
          <el-descriptions-item label="JSONL Size">{{ (currentSession.jsonl_size ?? 0) > 0 ? (currentSession.jsonl_size / 1024).toFixed(1) + ' KB' : '-' }}</el-descriptions-item>
        </el-descriptions>
        <div style="margin-top: 12px" v-if="currentSession.skill_usage?.length">
          <h4>Skills Used ({{ currentSession.skill_usage.length }})</h4>
          <div style="display: flex; gap: 6px; flex-wrap: wrap; margin-top: 8px">
            <el-tag v-for="u in currentSession.skill_usage" :key="u.skill_name" size="small">{{ u.skill_name }} ({{ u.mention_count }})</el-tag>
          </div>
        </div>
      </div>
    </el-dialog>
  </div>
</template>
