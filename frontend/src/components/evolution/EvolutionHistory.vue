<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { getEvolutionHistory, type EvolutionPhase, type EvolutionRun } from '../../api/evolution'

const runs = ref<EvolutionRun[]>([])
const total = ref(0)
const page = ref(1)
const size = 10
const loading = ref(false)

const phaseLabel: Record<string, string> = {
  discover: '扫描发现',
  reference_retrieval: '参考检索',
  cluster: '聚类分析',
  draft_generate: '生成草稿',
  optimize: '智能优化',
  qa_review: '质量评审',
  diff_recommend: '差异推荐',
}

const phaseOrder = ['discover', 'reference_retrieval', 'cluster', 'draft_generate', 'optimize', 'qa_review', 'diff_recommend']

const statusTag: Record<string, 'success' | 'warning' | 'info' | 'danger'> = {
  completed: 'success',
  running: 'warning',
  failed: 'danger',
  pending: 'info',
}

const statusText: Record<string, string> = {
  completed: '完成',
  running: '运行中',
  failed: '失败',
  pending: '待执行',
}

function formatTime(t: string | null): string {
  if (!t) return '-'
  try {
    const d = new Date(t)
    const pad = (n: number) => String(n).padStart(2, '0')
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
  } catch {
    return t
  }
}

function sortedPhases(phases: EvolutionPhase[]): EvolutionPhase[] {
  return phases.slice().sort((a, b) => phaseOrder.indexOf(a.phase) - phaseOrder.indexOf(b.phase))
}

async function load() {
  loading.value = true
  try {
    const res = await getEvolutionHistory(page.value, size)
    if (res.success && res.data) {
      runs.value = res.data.items
      total.value = res.data.total
    }
  } finally {
    loading.value = false
  }
}

onMounted(load)

function onPageChange(p: number) {
  page.value = p
  load()
}
</script>

<template>
  <div v-loading="loading" class="evolution-history">
    <el-empty v-if="runs.length === 0" description="暂无进化管道操作记录">
      <el-button type="primary" @click="load">刷新</el-button>
    </el-empty>

    <template v-else>
      <div class="history-header">
        <span class="history-title">共 {{ total }} 次进化记录</span>
        <el-button size="small" text @click="load">刷新</el-button>
      </div>

      <div class="run-list">
        <div v-for="run in runs" :key="run.run_id" class="run-card">
          <div class="run-card__head">
            <strong>#{{ run.run_id }}</strong>
            <el-tag :type="statusTag[run.status] || 'info'" size="small">
              {{ statusText[run.status] || run.status }}
            </el-tag>
            <span class="run-time">{{ formatTime(run.started_at) }} ~ {{ formatTime(run.completed_at) }}</span>
          </div>

          <div class="phase-bar">
            <div
              v-for="phase in sortedPhases(run.phases)"
              :key="phase.id"
              class="phase-item"
              :class="'phase-' + phase.status"
            >
              <div class="phase-dot">
                <span v-if="phase.status === 'completed'" class="check">&#10003;</span>
                <span v-else-if="phase.status === 'running'" class="pulse" />
                <span v-else class="empty-dot" />
              </div>
              <div class="phase-name">{{ phaseLabel[phase.phase] || phase.phase }}</div>
              <div class="phase-msg" :title="phase.message || ''">{{ phase.message || '-' }}</div>
              <div class="phase-time">
                <template v-if="phase.started_at">
                  {{ formatTime(phase.started_at) }}
                  <template v-if="phase.completed_at">
                    ~ {{ formatTime(phase.completed_at) }}
                  </template>
                </template>
                <template v-else>-</template>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div v-if="total > size" class="history-pager">
        <el-pagination
          background
          layout="prev, pager, next"
          :total="total"
          :page-size="size"
          v-model:current-page="page"
          @current-change="onPageChange"
        />
      </div>
    </template>
  </div>
</template>

<style scoped>
.evolution-history {
  background: var(--el-bg-color);
  border-radius: 12px;
  padding: 20px;
  border: 1px solid var(--el-border-color-lighter);
}

.history-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.history-title {
  font-size: 14px;
  color: var(--el-text-color-secondary);
}

.run-list {
  display: grid;
  gap: 16px;
}

.run-card {
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 10px;
  overflow: hidden;
}

.run-card__head {
  padding: 10px 14px;
  display: flex;
  align-items: center;
  gap: 10px;
  background: #fafbfe;
  border-bottom: 1px solid var(--el-border-color-lighter);
  font-size: 14px;
}

.run-time {
  margin-left: auto;
  font-size: 12px;
  color: var(--el-text-color-placeholder);
}

.phase-bar {
  padding: 6px 14px;
}

.phase-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 0;
  border-bottom: 1px solid #f3f4f6;
}

.phase-item:last-child {
  border-bottom: none;
}

.phase-dot {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  border: 2px solid var(--el-border-color);
}

.phase-completed .phase-dot {
  border-color: #22c55e;
  background: #22c55e;
}

.phase-running .phase-dot {
  border-color: var(--el-color-primary);
  box-shadow: 0 0 0 3px var(--el-color-primary-light-8);
}

.phase-failed .phase-dot {
  border-color: var(--el-color-danger);
  background: var(--el-color-danger);
}

.check {
  font-size: 12px;
  color: #fff;
  font-weight: 700;
}

.pulse {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--el-color-primary);
  animation: pulse 1.5s infinite;
}

.empty-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--el-border-color);
}

@keyframes pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.4; transform: scale(0.7); }
}

.phase-name {
  width: 72px;
  font-size: 13px;
  font-weight: 500;
  flex-shrink: 0;
  color: var(--el-text-color-regular);
}

.phase-completed .phase-name {
  color: #22c55e;
}

.phase-running .phase-name {
  color: var(--el-color-primary);
}

.phase-failed .phase-name {
  color: var(--el-color-danger);
}

.phase-msg {
  flex: 1;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.phase-time {
  font-size: 11px;
  color: var(--el-text-color-placeholder);
  white-space: nowrap;
}

.history-pager {
  margin-top: 16px;
  display: flex;
  justify-content: center;
}
</style>
