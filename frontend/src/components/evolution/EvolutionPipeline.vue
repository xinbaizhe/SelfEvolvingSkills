<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { ElMessage } from 'element-plus'
import { getEvolutionStatus, resetEvolution, startEvolution, type EvolutionJob, type EvolutionPhase, type EvolutionStep } from '../../api/evolution'
import { fetchSources, type SourceConfig } from '../../api/scan'

const emit = defineEmits<{
  (e: 'completed'): void
  (e: 'started'): void
}>()

const job = ref<EvolutionJob | null>(null)
const sources = ref<SourceConfig[]>([])
const selectedAgentIds = ref<string[]>([])
const loading = ref(false)
const resetting = ref(false)
const statusLoading = ref(false)
const staleDetected = ref(false)
const anyFailed = ref(false)
const unlisten = ref<(() => void) | null>(null)

const steps: EvolutionStep[] = [
  { phase: 'discover', label: '扫描发现', start: 0, end: 20 },
  { phase: 'reference_retrieval', label: '参考检索', start: 20, end: 32 },
  { phase: 'cluster', label: '聚类分析', start: 32, end: 50 },
  { phase: 'draft_generate', label: '生成草稿', start: 50, end: 62 },
  { phase: 'optimize', label: '智能优化', start: 62, end: 78 },
  { phase: 'qa_review', label: '质量评审', start: 78, end: 90 },
  { phase: 'diff_recommend', label: '差异推荐', start: 90, end: 100 },
]

const availableSources = computed(() => sources.value.filter((source) => source.is_enabled || source.is_available))

const currentPhase = computed(() => {
  if (!job.value?.current_phase) return null
  return job.value.current_phase
})

const currentStepIndex = computed(() => {
  if (!currentPhase.value) return -1
  return steps.findIndex((step) => step.phase === currentPhase.value)
})

const phaseStatuses = computed(() => {
  if (!job.value?.phases) return new Map<string, string>()
  const map = new Map<string, string>()
  for (const p of job.value.phases) {
    map.set(p.phase, p.status)
  }
  return map
})

const progressPercent = computed(() => {
  if (!job.value?.phases || job.value.phases.length === 0) {
    return job.value?.last_completed ? 100 : 0
  }
  const running = job.value.phases.find((p) => p.status === 'running')
  if (running) return running.progress
  const completed = job.value.phases.filter((p) => p.status === 'completed').length
  if (completed === steps.length) return 100
  const lastDone = job.value.phases
    .filter((p) => p.status === 'completed' || p.status === 'failed')
    .reduce((max, p) => (p.progress > max ? p.progress : max), 0)
  return lastDone || 0
})

const currentMessage = computed(() => {
  if (!job.value?.phases) return ''
  const running = job.value.phases.find((p) => p.status === 'running')
  return running?.message || ''
})

const isRunning = computed(() => job.value?.running ?? false)

const statusText = computed(() => {
  if (isRunning.value) return '进行中'
  if (!job.value?.phases || job.value.phases.length === 0) return '待启动'
  const allDone = job.value.phases.every((p) => p.status === 'completed')
  if (allDone) return '已完成'
  if (anyFailed.value) return '部分失败'
  return '待启动'
})

const circumference = 2 * Math.PI * 54
const dashOffset = computed(() => circumference - (progressPercent.value / 100) * circumference)

const phaseOrder = ['discover', 'reference_retrieval', 'cluster', 'draft_generate', 'optimize', 'qa_review', 'diff_recommend']

function phaseCompleted(phase: string): boolean {
  if (!job.value?.phases) return false
  const status = phaseStatuses.value.get(phase)
  return status === 'completed'
}

function phaseActive(phase: string): boolean {
  return currentPhase.value === phase && isRunning.value
}

function phaseFailed(phase: string): boolean {
  if (!job.value?.phases) return false
  const status = phaseStatuses.value.get(phase)
  return status === 'failed'
}

async function loadSources() {
  const res = await fetchSources()
  if (res.success && res.data) {
    sources.value = res.data
    if (selectedAgentIds.value.length === 0) {
      selectedAgentIds.value = res.data.filter((source) => source.is_enabled).map((source) => source.agent_id)
    }
  }
}

async function loadStatus() {
  statusLoading.value = true
  try {
    const res = await getEvolutionStatus()
    if (res.success && res.data) {
      const data = res.data as EvolutionJob & { auto_failed?: number; any_failed?: boolean }
      job.value = data
      anyFailed.value = data.any_failed ?? false
      if (data.auto_failed && data.auto_failed > 0) {
        staleDetected.value = true
        ElMessage.warning(`检测到 ${data.auto_failed} 个阶段超时（>10分钟），已自动标记为失败。`)
      }
      // Check for stuck state: running but current_phase is null or no progress
      if (data.running && !data.current_phase) {
        staleDetected.value = true
      }
      if (data.running) startListening()
    }
  } catch {
    // 状态查询失败不阻断页面。
  } finally {
    statusLoading.value = false
  }
}

async function handleStart() {
  if (selectedAgentIds.value.length === 0) {
    ElMessage.warning('请至少选择一个 Agent 作为扫描范围')
    return
  }

  loading.value = true
  try {
    const res = await startEvolution(selectedAgentIds.value)
    if (res.success && res.data) {
      job.value = {
        run_id: res.data.run_id,
        phases: steps.map((s) => ({
          id: 0,
          phase: s.phase,
          status: 'pending',
          progress: 0,
          message: null,
          started_at: null,
          completed_at: null,
        })),
        steps,
        running: true,
        current_phase: 'discover',
        last_completed: null,
      }
      startListening()
      emit('started')
      ElMessage.success('进化管道已启动，执行 7 步流程：扫描发现 → 参考检索 → 聚类分析 → 生成草稿 → 智能优化 → 质量评审 → 差异推荐')
    } else {
      ElMessage.error(res.error || '启动进化管道失败')
    }
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : '启动进化管道失败')
  } finally {
    loading.value = false
  }
}

function selectClaudeCodeOnly() {
  selectedAgentIds.value = sources.value.some((source) => source.agent_id === 'claude-code') ? ['claude-code'] : []
}

function selectAllEnabled() {
  selectedAgentIds.value = sources.value.filter((source) => source.is_enabled).map((source) => source.agent_id)
}

function startListening() {
  if (unlisten.value) return
  listen<{ run_id: number; phase: string; progress: number; message: string }>('evolution-progress', (event) => {
    if (!job.value || job.value.run_id !== event.payload.run_id) return

    // When the "completed" terminal event arrives, mark everything done
    if (event.payload.phase === 'completed') {
      const finalPhases = job.value.phases.map((p) => ({
        ...p,
        status: 'completed' as const,
        completed_at: p.completed_at || new Date().toISOString(),
      }))
      job.value = { ...job.value, phases: finalPhases, running: false, current_phase: null }
      stopListening()
      emit('completed')
      ElMessage.success('进化流程已完成，推荐、草稿和社区对比已刷新。')
      return
    }

    const phases = job.value.phases.map((p) => {
      if (p.phase === event.payload.phase) {
        const step = steps.find((s) => s.phase === p.phase)
        const reachedEnd = step ? event.payload.progress >= step.end : false
        return {
          ...p,
          status: reachedEnd ? 'completed' as const : p.status === 'pending' ? 'running' as const : p.status,
          progress: event.payload.progress,
          message: event.payload.message,
          started_at: p.started_at || new Date().toISOString(),
          completed_at: reachedEnd ? (p.completed_at || new Date().toISOString()) : p.completed_at,
        }
      }
      const phaseIdx = phaseOrder.indexOf(p.phase)
      const eventIdx = phaseOrder.indexOf(event.payload.phase)
      if (phaseIdx < eventIdx && p.status !== 'completed') {
        const step = steps.find((s) => s.phase === p.phase)
        return { ...p, status: 'completed' as const, progress: step?.end ?? p.progress, completed_at: p.completed_at || new Date().toISOString() }
      }
      return p
    })

    const allDone = phases.every((p) => p.status === 'completed')

    job.value = {
      ...job.value,
      phases,
      current_phase: allDone ? null : event.payload.phase,
      running: !allDone,
    }

    if (allDone) {
      stopListening()
      emit('completed')
      ElMessage.success('进化流程已完成，推荐、草稿和社区对比已刷新。')
    }
  }).then((fn) => {
    unlisten.value = fn
  })
}

function stopListening() {
  unlisten.value?.()
  unlisten.value = null
}

async function handleReset() {
  resetting.value = true
  try {
    const res = await resetEvolution()
    if (res.success && res.data) {
      ElMessage.success(res.data.message || '已重置')
      staleDetected.value = false
      job.value = null
      await loadStatus()
    } else {
      ElMessage.error(res.error || '重置失败')
    }
  } catch (e: unknown) {
    ElMessage.error(e instanceof Error ? e.message : '重置失败')
  } finally {
    resetting.value = false
  }
}

onMounted(() => {
  loadSources()
})

onUnmounted(stopListening)
</script>

<template>
  <div class="evolution-pipeline">
    <div class="pipeline-header">
      <div>
        <h3>Skill 进化管道</h3>
        <p>选择 Agent 后启动，系统会真实执行：扫描发现 → 参考检索 → 聚类分析 → 生成草稿 → 智能优化 → 质量评审 → 差异推荐。</p>
      </div>
      <el-button type="primary" :loading="loading" :disabled="isRunning" @click="handleStart">
        {{ isRunning ? '进化中...' : '启动进化' }}
      </el-button>
      <el-button text :loading="statusLoading" @click="loadStatus">加载状态</el-button>
      <el-button v-if="staleDetected" type="warning" text :loading="resetting" @click="handleReset">
        重置卡住的管道
      </el-button>
    </div>

    <div class="scope-panel">
      <div class="scope-head">
        <b>扫描范围</b>
        <div>
          <el-button size="small" text @click="selectClaudeCodeOnly">只选 Claude Code</el-button>
          <el-button size="small" text @click="selectAllEnabled">选择全部已启用</el-button>
        </div>
      </div>
      <el-select
        v-model="selectedAgentIds"
        multiple
        filterable
        collapse-tags
        collapse-tags-tooltip
        placeholder="选择要扫描的 Agent"
        style="width: 100%"
        :disabled="isRunning"
      >
        <el-option
          v-for="source in availableSources"
          :key="source.agent_id"
          :label="`${source.agent_name} · ${source.is_available ? '已检测到' : '未检测到路径'}`"
          :value="source.agent_id"
        />
      </el-select>
      <p class="scope-note">
        长对话会先做本地结构化压缩，只保留目标、工具、错误、结果和关键上下文，用于聚类与草稿生成。
      </p>
    </div>

    <div class="progress-ring-container">
      <svg class="progress-ring" viewBox="0 0 120 120">
        <circle class="ring-bg" cx="60" cy="60" r="54" fill="none" stroke="var(--el-border-color-lighter)" stroke-width="8" />
        <circle
          class="ring-fill"
          cx="60"
          cy="60"
          r="54"
          fill="none"
          stroke="var(--el-color-primary)"
          stroke-width="8"
          stroke-linecap="round"
          :stroke-dasharray="circumference"
          :stroke-dashoffset="dashOffset"
          transform="rotate(-90 60 60)"
        />
        <text x="60" y="56" text-anchor="middle" class="ring-text-large">{{ progressPercent }}%</text>
        <text x="60" y="74" text-anchor="middle" class="ring-text-small">{{ statusText }}</text>
      </svg>
    </div>

    <div v-if="currentMessage" class="current-message">
      <span v-if="isRunning" class="spinner" />
      <span>{{ currentMessage }}</span>
    </div>

    <div class="steps-bar">
      <div
        v-for="(step, index) in steps"
        :key="step.phase"
        class="step-item"
        :class="{
          active: phaseActive(step.phase),
          completed: phaseCompleted(step.phase),
          failed: phaseFailed(step.phase),
        }"
      >
        <div class="step-dot">
          <span v-if="phaseCompleted(step.phase)" class="check">&#10003;</span>
          <span v-else-if="phaseFailed(step.phase)" class="cross">&#10007;</span>
          <span v-else-if="phaseActive(step.phase)" class="pulse" />
          <span v-else class="num">{{ index + 1 }}</span>
        </div>
        <div class="step-label">{{ step.label }}</div>
        <div class="step-range">{{ step.start }}% - {{ step.end }}%</div>
      </div>
    </div>

    <div v-if="job?.last_completed && !isRunning" class="last-completed">
      上次完成时间：{{ job.last_completed.completed_at || '未知' }}
    </div>
  </div>
</template>

<style scoped>
.evolution-pipeline {
  background: var(--el-bg-color);
  border-radius: 12px;
  padding: 24px;
  border: 1px solid var(--el-border-color-lighter);
}

.pipeline-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 16px;
  margin-bottom: 16px;
}

.pipeline-header h3 {
  margin: 0 0 4px;
  font-size: 18px;
  font-weight: 600;
}

.pipeline-header p,
.scope-note {
  margin: 0;
  color: var(--el-text-color-secondary);
  font-size: 13px;
  line-height: 1.5;
}

.scope-panel {
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  padding: 14px;
  margin-bottom: 18px;
}

.scope-head {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: center;
  margin-bottom: 10px;
}

.scope-note {
  margin-top: 8px;
}

.progress-ring-container {
  display: flex;
  justify-content: center;
  margin-bottom: 16px;
}

.progress-ring { width: 140px; height: 140px; }
.ring-bg { opacity: 0.15; }
.ring-fill { transition: stroke-dashoffset 0.6s ease; }
.ring-text-large { font-size: 22px; font-weight: 700; fill: var(--el-text-color-primary); }
.ring-text-small { font-size: 11px; fill: var(--el-text-color-secondary); }

.current-message {
  text-align: center;
  padding: 10px 16px;
  margin-bottom: 20px;
  background: var(--el-color-primary-light-9);
  border-radius: 8px;
  font-size: 14px;
  color: var(--el-color-primary);
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.steps-bar {
  display: flex;
  justify-content: space-between;
  gap: 4px;
}

.step-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  flex: 1;
  position: relative;
}

.step-item::after {
  content: '';
  position: absolute;
  top: 14px;
  left: 60%;
  right: -40%;
  height: 2px;
  background: var(--el-border-color-lighter);
  z-index: 0;
}

.step-item:last-child::after { display: none; }
.step-item.completed::after { background: var(--el-color-primary); }

.step-dot {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 600;
  z-index: 1;
  border: 2px solid var(--el-border-color);
  background: var(--el-bg-color);
  color: var(--el-text-color-secondary);
  transition: all 0.3s;
}

.step-item.active .step-dot {
  border-color: var(--el-color-primary);
  color: var(--el-color-primary);
  box-shadow: 0 0 0 4px var(--el-color-primary-light-8);
}

.step-item.completed .step-dot {
  border-color: var(--el-color-primary);
  background: var(--el-color-primary);
  color: #fff;
}

.step-item.failed .step-dot {
  border-color: var(--el-color-danger);
  background: var(--el-color-danger);
  color: #fff;
}

.step-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  white-space: nowrap;
}

.step-item.active .step-label,
.step-item.completed .step-label {
  color: var(--el-color-primary);
  font-weight: 600;
}

.step-item.failed .step-label {
  color: var(--el-color-danger);
}

.step-range {
  font-size: 10px;
  color: var(--el-text-color-placeholder);
}

.check { font-size: 14px; color: #fff; }
.cross { font-size: 14px; color: #fff; font-weight: 700; }
.pulse {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--el-color-primary);
  animation: pulse 1.5s infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.4; transform: scale(0.7); }
}

.spinner {
  width: 14px;
  height: 14px;
  border: 2px solid var(--el-color-primary-light-5);
  border-top-color: var(--el-color-primary);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  flex-shrink: 0;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.last-completed {
  text-align: center;
  margin-top: 16px;
  font-size: 12px;
  color: var(--el-text-color-placeholder);
}
</style>
