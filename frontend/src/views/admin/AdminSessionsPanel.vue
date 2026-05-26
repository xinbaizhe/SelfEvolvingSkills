<script setup lang="ts">
import { ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { fetchSessionDetail, fetchSessions } from '../../api/stats'
import { formatBytes } from '../../utils/format'

interface SessionItem {
  session_id: string
  agent_source: string
  project_name?: string
  first_prompt?: string
  message_count: number
  jsonl_size?: number
  started_at?: string
  cwd?: string
  entrypoint?: string
  version?: string
  kind?: string
  jsonl_path?: string
  compressed_summary?: string
}

const sessions = ref<SessionItem[]>([])
const props = defineProps<{ openRequest?: number }>()
const sessionsDrawerVisible = defineModel<boolean>({ default: false })
const sessionsTotal = ref(0)
const sessionsPage = ref(1)
const sessionsLoading = ref(false)
const sessionDetailVisible = ref(false)
const currentSession = ref<SessionItem | null>(null)
const sessionDetailLoading = ref(false)

async function open() {
  sessionsDrawerVisible.value = true
  sessionsPage.value = 1
  await loadSessions()
}

watch(
  () => props.openRequest,
  (value, oldValue) => {
    if (!value || value === oldValue) return
    void open()
  },
)

async function loadSessions() {
  sessionsLoading.value = true
  try {
    const res = await fetchSessions({ page: sessionsPage.value, size: 20 })
    if (res.success && res.data) {
      const data = res.data as { items?: SessionItem[]; total?: number }
      sessions.value = data.items ?? []
      sessionsTotal.value = data.total ?? 0
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
      currentSession.value = res.data as SessionItem
    } else {
      ElMessage.error('会话详情加载失败')
    }
  } finally {
    sessionDetailLoading.value = false
  }
}

defineExpose({ open })
</script>

<template>
  <el-drawer v-model="sessionsDrawerVisible" title="会话详情" size="72%" append-to-body :lock-scroll="false" :z-index="3000">
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
</template>

<style scoped>
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
</style>
