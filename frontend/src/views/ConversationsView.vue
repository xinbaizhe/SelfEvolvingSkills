<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { fetchSessions, fetchSessionDetail } from '../api/stats'
import { getErrorMessage } from '../utils/error'
import type { PaginatedResult } from '../api/skills'

interface SessionItem {
  session_id: string
  agent_source: string
  project_name?: string
  first_prompt?: string
  message_count: number
  jsonl_size?: number
  started_at?: string
}

interface SessionDetail extends SessionItem {
  cwd?: string
  entrypoint?: string
  version?: string
  kind?: string
  jsonl_path?: string
  compressed_summary?: string
}

const router = useRouter()

const sessions = ref<SessionItem[]>([])
const total = ref(0)
const loading = ref(false)
const currentPage = ref(1)
const pageSize = ref(20)

const detailVisible = ref(false)
const detailLoading = ref(false)
const currentSession = ref<SessionDetail | null>(null)

onMounted(() => loadSessions())

async function loadSessions() {
  loading.value = true
  try {
    const res = await fetchSessions({ page: currentPage.value, size: pageSize.value })
    if (res.success && res.data) {
      sessions.value = (res.data as PaginatedResult<SessionItem>).items ?? []
      total.value = (res.data as PaginatedResult<SessionItem>).total ?? 0
    } else {
      ElMessage.error('会话列表加载失败')
    }
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '加载失败'))
  } finally {
    loading.value = false
  }
}

async function showDetail(sessionId: string) {
  detailVisible.value = true
  detailLoading.value = true
  currentSession.value = null
  try {
    const res = await fetchSessionDetail(sessionId)
    if (res.success) {
      currentSession.value = res.data as SessionDetail
    } else {
      ElMessage.error('会话详情加载失败')
    }
  } finally {
    detailLoading.value = false
  }
}

function formatBytes(bytes: number | null | undefined) {
  const value = Number(bytes ?? 0)
  if (!value) return '-'
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`
  return `${(value / 1024 / 1024).toFixed(2)} MB`
}

function formatDateTime(raw: string | null | undefined) {
  if (!raw) return '-'
  // Already in target format: yyyy-MM-dd HH:mm:ss
  if (/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/.test(raw)) return raw
  // ISO 8601: 2026-05-21T01:35:03.184Z
  const d = new Date(raw)
  if (isNaN(d.getTime())) return raw
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  const h = String(d.getHours()).padStart(2, '0')
  const min = String(d.getMinutes()).padStart(2, '0')
  const s = String(d.getSeconds()).padStart(2, '0')
  return `${y}-${m}-${day} ${h}:${min}:${s}`
}

function onPageChange() {
  loadSessions()
}
</script>

<template>
  <section class="page-view">
    <div class="page-headline">
      <div>
        <h2>会话历史</h2>
        <p>浏览本地 Agent 的会话记录，查看工作流指纹和摘要信息。</p>
      </div>
      <div class="headline-right">
        <span class="count-badge">{{ total }} 条会话</span>
        <el-button size="small" @click="router.push('/admin')">返回</el-button>
      </div>
    </div>

    <el-table :data="sessions" v-loading="loading" stripe height="calc(100vh - 240px)" empty-text="暂无会话数据，请先执行扫描">
      <el-table-column prop="agent_source" label="Agent" width="120" />
      <el-table-column prop="project_name" label="项目" width="180" show-overflow-tooltip />
      <el-table-column prop="first_prompt" label="首条请求" min-width="260" show-overflow-tooltip />
      <el-table-column prop="message_count" label="消息数" width="90" />
      <el-table-column label="文件大小" width="100">
        <template #default="{ row }">{{ formatBytes(row.jsonl_size) }}</template>
      </el-table-column>
      <el-table-column label="开始时间" width="170">
        <template #default="{ row }">{{ formatDateTime(row.started_at) }}</template>
      </el-table-column>
      <el-table-column label="操作" width="90" fixed="right">
        <template #default="{ row }">
          <el-button link type="primary" @click="showDetail(row.session_id)">详情</el-button>
        </template>
      </el-table-column>
    </el-table>

    <div class="pagination-row">
      <el-pagination
        v-model:current-page="currentPage"
        v-model:page-size="pageSize"
        :total="total"
        :page-sizes="[10, 20, 50]"
        layout="total, sizes, prev, pager, next"
        @current-change="onPageChange"
        @size-change="onPageChange"
      />
    </div>

    <el-dialog v-model="detailVisible" title="会话详情" width="760px" append-to-body destroy-on-close>
      <div v-loading="detailLoading">
        <el-descriptions v-if="currentSession" :column="1" border size="small">
          <el-descriptions-item label="会话 ID">{{ currentSession.session_id }}</el-descriptions-item>
          <el-descriptions-item label="Agent">{{ currentSession.agent_source || '-' }}</el-descriptions-item>
          <el-descriptions-item label="项目">{{ currentSession.project_name || '-' }}</el-descriptions-item>
          <el-descriptions-item label="工作目录">{{ currentSession.cwd || '-' }}</el-descriptions-item>
          <el-descriptions-item label="入口">{{ currentSession.entrypoint || '-' }}</el-descriptions-item>
          <el-descriptions-item label="版本">{{ currentSession.version || '-' }}</el-descriptions-item>
          <el-descriptions-item label="类型">{{ currentSession.kind || '-' }}</el-descriptions-item>
          <el-descriptions-item label="开始时间">{{ formatDateTime(currentSession.started_at) }}</el-descriptions-item>
          <el-descriptions-item label="消息数">{{ currentSession.message_count ?? 0 }}</el-descriptions-item>
          <el-descriptions-item label="JSONL 文件">{{ currentSession.jsonl_path || '-' }}</el-descriptions-item>
          <el-descriptions-item label="文件大小">{{ formatBytes(currentSession.jsonl_size) }}</el-descriptions-item>
          <el-descriptions-item label="首条用户请求">
            <pre class="session-text">{{ currentSession.first_prompt || '未提取到首条用户请求' }}</pre>
          </el-descriptions-item>
          <el-descriptions-item label="压缩摘要">
            <pre class="session-text">{{ currentSession.compressed_summary || '暂无压缩摘要' }}</pre>
          </el-descriptions-item>
        </el-descriptions>
        <div v-else-if="!detailLoading" class="empty-state">暂无数据</div>
      </div>
    </el-dialog>
  </section>
</template>

<style scoped>
.page-headline {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 20px;
}
.page-headline h2 { margin: 0 0 8px; }
.page-headline p { margin: 0; color: var(--muted); font-size: 13px; }
.headline-right {
  display: flex;
  align-items: center;
  gap: 12px;
}
.count-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 999px;
  background: #e9f8f3;
  color: #0c8265;
  font-size: 12px;
  font-weight: 700;
  min-width: 92px;
  padding: 6px 10px;
}
.pagination-row {
  display: flex;
  justify-content: flex-end;
  margin-top: 16px;
}
.session-text {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 12px;
  line-height: 1.6;
  max-height: 200px;
  overflow-y: auto;
}
.empty-state { text-align: center; padding: 40px; color: var(--muted); }
</style>
