<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
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
const { t } = useI18n()
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
      ElMessage.error(t('admin.sessions.listLoadFailed'))
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
      ElMessage.error(t('admin.sessions.detailLoadFailed'))
    }
  } finally {
    sessionDetailLoading.value = false
  }
}

function formatDateTime(raw: string | null | undefined) {
  if (!raw) return '-'
  if (/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/.test(raw)) return raw
  const date = new Date(raw)
  if (Number.isNaN(date.getTime())) return raw
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  const hour = String(date.getHours()).padStart(2, '0')
  const minute = String(date.getMinutes()).padStart(2, '0')
  const second = String(date.getSeconds()).padStart(2, '0')
  return `${year}-${month}-${day} ${hour}:${minute}:${second}`
}

defineExpose({ open })
</script>

<template>
  <el-drawer v-model="sessionsDrawerVisible" :title="t('admin.sessions.detailTitle')" size="72%" append-to-body :lock-scroll="false" :z-index="3000">
    <el-table :data="sessions" v-loading="sessionsLoading" stripe height="calc(100vh - 190px)" :empty-text="t('admin.sessions.empty')">
      <el-table-column prop="agent_source" label="Agent" width="120" />
      <el-table-column prop="project_name" :label="t('admin.sessions.project')" width="180" show-overflow-tooltip />
      <el-table-column prop="first_prompt" :label="t('admin.sessions.firstPrompt')" min-width="260" show-overflow-tooltip />
      <el-table-column prop="message_count" :label="t('admin.sessions.messages')" width="90" />
      <el-table-column prop="jsonl_size" :label="t('admin.sessions.fileSize')" width="100">
        <template #default="{ row }">{{ formatBytes(row.jsonl_size) }}</template>
      </el-table-column>
      <el-table-column :label="t('admin.common.startedAt')" width="170">
        <template #default="{ row }">{{ formatDateTime(row.started_at) }}</template>
      </el-table-column>
      <el-table-column :label="t('admin.common.action')" width="90" fixed="right">
        <template #default="{ row }">
          <el-button link type="primary" @click="showSessionDetail(row.session_id)">{{ t('admin.common.view') }}</el-button>
        </template>
      </el-table-column>
    </el-table>
    <div class="drawer-pagination">
      <el-pagination v-model:current-page="sessionsPage" :total="sessionsTotal" :page-size="20" layout="total, prev, pager, next" @current-change="loadSessions" />
    </div>
  </el-drawer>

  <el-dialog v-model="sessionDetailVisible" :title="t('admin.sessions.sessionRecord')" width="760px" append-to-body destroy-on-close>
    <div v-loading="sessionDetailLoading">
      <el-descriptions v-if="currentSession" :column="1" border size="small">
        <el-descriptions-item :label="t('admin.sessions.sessionId')">{{ currentSession.session_id }}</el-descriptions-item>
        <el-descriptions-item label="Agent">{{ currentSession.agent_source || '-' }}</el-descriptions-item>
        <el-descriptions-item :label="t('admin.sessions.project')">{{ currentSession.project_name || '-' }}</el-descriptions-item>
        <el-descriptions-item :label="t('admin.sessions.workingDir')">{{ currentSession.cwd || '-' }}</el-descriptions-item>
        <el-descriptions-item :label="t('admin.sessions.entrypoint')">{{ currentSession.entrypoint || '-' }}</el-descriptions-item>
        <el-descriptions-item :label="t('admin.sessions.version')">{{ currentSession.version || '-' }}</el-descriptions-item>
        <el-descriptions-item :label="t('admin.common.type')">{{ currentSession.kind || '-' }}</el-descriptions-item>
        <el-descriptions-item :label="t('admin.common.startedAt')">{{ formatDateTime(currentSession.started_at) }}</el-descriptions-item>
        <el-descriptions-item :label="t('admin.sessions.messages')">{{ currentSession.message_count ?? 0 }}</el-descriptions-item>
        <el-descriptions-item :label="t('admin.sessions.jsonlFile')">{{ currentSession.jsonl_path || '-' }}</el-descriptions-item>
        <el-descriptions-item :label="t('admin.sessions.fileSize')">{{ formatBytes(currentSession.jsonl_size) }}</el-descriptions-item>
        <el-descriptions-item :label="t('admin.sessions.firstUserPrompt')">
          <pre class="session-text">{{ currentSession.first_prompt || t('admin.sessions.noFirstPrompt') }}</pre>
        </el-descriptions-item>
        <el-descriptions-item :label="t('admin.sessions.compressedSummary')">
          <pre class="session-text">{{ currentSession.compressed_summary || t('admin.sessions.noSummary') }}</pre>
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
