<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import {
  fetchDailyReport,
  fetchDailyReportHistory,
  fetchGenerationStatus,
  generateDailyReport,
  type DailyReport,
  type GenStatus,
} from '../../api/admin'

const report = ref<DailyReport | null>(null)
const loading = ref(false)
const generating = ref(false)
const genProgress = ref(0)
const genPhase = ref('')
const selectedDate = ref(todayStr())
const history = ref<{ report_date: string; source_count: number; generated_at: string }[]>([])

let autoTimer: ReturnType<typeof setInterval> | null = null
let pollTimer: ReturnType<typeof setInterval> | null = null
let failedAutoAttempts = 0
const MAX_AUTO_ATTEMPTS = 3

function todayStr(): string {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

function isToday(date: string): boolean {
  return date === todayStr()
}

function formatDisplayTime(dateStr: string): string {
  if (!dateStr) return ''
  return dateStr.length >= 16 ? dateStr.slice(0, 16) : dateStr
}

function friendlyError(err: unknown): string {
  const msg = String(err?.toString?.() ?? err ?? '')
  if (msg.includes('大模型未启用') || msg.includes('未启用')) {
    return 'AI 模型还没启用，请先去「资源与配置」页面配置并启用大模型'
  }
  if (msg.includes('API Key') || msg.includes('api_key')) {
    return 'AI 模型缺少 API 密钥，请先去「资源与配置」页面设置 API Key'
  }
  if (msg.includes('没有找到') || msg.includes('对话记录')) {
    return '没有找到选中日期的编程对话记录，是不是当天没有用 AI 编程工具？试试切换到其他日期'
  }
  if (msg.includes('HTTP') || msg.includes('timeout') || msg.includes('超时') || msg.includes('连接')) {
    return 'AI 模型连接不上，请检查网络或模型配置（API 地址、密钥、模型名称）是否正确'
  }
  if (msg.includes('缺少文本') || msg.includes('响应') || msg.includes('内容')) {
    return 'AI 模型返回了无法识别的内容，可能是模型不支持该请求格式，请检查模型配置'
  }
  if (msg.includes('已有日报')) {
    return msg
  }
  if (msg.includes('生成中')) {
    return msg
  }
  return msg || '操作失败，请检查模型配置和网络连接后重试'
}

function phaseLabel(phase: string): string {
  const map: Record<string, string> = {
    scanning: '正在扫描本地 AI 对话文件...',
    summarizing: '正在调用 AI 模型生成日报...',
    done: '日报生成完成',
    error: '生成出错',
  }
  return map[phase] || phase
}

async function loadReport(date?: string) {
  loading.value = true
  try {
    const res = await fetchDailyReport(date)
    report.value = res.data ?? null
  } catch (err) {
    ElMessage.error(friendlyError(err))
  } finally {
    loading.value = false
  }
}

async function loadHistory() {
  try {
    const res = await fetchDailyReportHistory(30)
    history.value = res.data ?? []
  } catch {
    // 历史加载失败不影响主功能
  }
}

/** Start generation and begin polling for progress */
async function handleGenerate() {
  try {
    // 1. Call API first — don't show generating UI until we know it's not cached
    const res = await generateDailyReport(selectedDate.value)
    const status: GenStatus = res.data!

    if (status.from_cache) {
      // Already cached — load directly, no progress UI needed
      report.value = {
        report_date: status.report_date || selectedDate.value,
        content: status.content || '',
        source_count: status.source_count || 0,
        generated_at: status.generated_at || '',
        from_cache: true,
      }
      ElMessage.success('该日期已有日报，直接加载')
      await loadHistory()
      return
    }

    // 2. Not cached — start progress UI and poll
    generating.value = true
    genProgress.value = status.progress || 5
    genPhase.value = status.phase || 'scanning'

    if (status.message?.includes('生成中')) {
      ElMessage.info('日报生成已启动，后台处理中...')
    }

    await pollUntilDone()
  } catch (err) {
    ElMessage.error(friendlyError(err))
    generating.value = false
  }
}

/** Poll generation status until done or error */
async function pollUntilDone() {
  clearPollTimer()
  return new Promise<void>((resolve) => {
    pollTimer = setInterval(async () => {
      try {
        const res = await fetchGenerationStatus()
        const status: GenStatus = res.data!
        genPhase.value = status.phase || ''
        genProgress.value = status.progress || 0

        if (!status.generating) {
          // Done or error
          clearPollTimer()
          generating.value = false
          if (status.phase === 'error' || status.error) {
            ElMessage.error(friendlyError(status.error))
            failedAutoAttempts++
          } else {
            ElMessage.success('日报生成完成')
            await loadReport()
            await loadHistory()
            failedAutoAttempts = 0
          }
          resolve()
        }
      } catch {
        // Poll errors are non-critical, keep trying
      }
    }, 2000)
  })
}

function clearPollTimer() {
  if (pollTimer) { clearInterval(pollTimer); pollTimer = null }
}

async function onDateChange(date: string) {
  selectedDate.value = date
  await loadReport(date)
}

// ── Auto 9AM generation ──

onMounted(async () => {
  await loadReport()
  await loadHistory()

  // Check every 60s if it's past 9AM and no report yet
  autoTimer = setInterval(() => {
    const now = new Date()
    const nineAM = new Date()
    nineAM.setHours(9, 0, 0, 0)
    if (
      now >= nineAM &&
      !report.value &&
      !generating.value &&
      failedAutoAttempts < MAX_AUTO_ATTEMPTS
    ) {
      handleGenerate()
    }
  }, 60000)
})

onUnmounted(() => {
  if (autoTimer) clearInterval(autoTimer)
  clearPollTimer()
})
</script>

<template>
  <div class="daily-report-page">
    <div class="page-header">
      <div class="header-left">
        <h2>工作日报</h2>
        <span class="header-subtitle">基于 AI 编程对话记录自动生成每日工作总结</span>
      </div>
      <div class="header-right">
        <input
          type="date"
          :value="selectedDate"
          :max="todayStr()"
          class="date-picker"
          @change="onDateChange(($event.target as HTMLInputElement).value)"
        />
        <button
          class="btn-generate"
          :disabled="generating"
          @click="handleGenerate"
        >
          <span v-if="generating" class="spinner"></span>
          {{ generating ? '后台生成中…' : '生成日报' }}
        </button>
      </div>
    </div>

    <!-- Generation progress bar -->
    <div v-if="generating" class="gen-progress-bar">
      <div class="gen-progress-info">
        <span class="spinner small"></span>
        <span>{{ phaseLabel(genPhase) }}</span>
        <span class="gen-progress-pct">{{ genProgress }}%</span>
      </div>
      <div class="progress-track">
        <div
          class="progress-fill"
          :style="{ width: genProgress + '%' }"
        ></div>
      </div>
      <p class="gen-progress-hint">数据来源：读取本地 AI 编程工具对话文件（Claude Code、Codex、Cursor 等），无需联网</p>
    </div>

    <!-- History date chips -->
    <div class="history-bar" v-if="history.length > 0">
      <span class="history-label">历史日报：</span>
      <button
        v-for="item in history.slice(0, 14)"
        :key="item.report_date"
        :class="['date-chip', { active: selectedDate === item.report_date }]"
        @click="onDateChange(item.report_date)"
      >
        {{ item.report_date.slice(5) }}
        <span class="chip-badge">{{ item.source_count }}</span>
      </button>
    </div>

    <!-- Loading state -->
    <div v-if="loading" class="state-box">
      <span class="spinner large"></span>
      <p>加载日报中，请稍候…</p>
    </div>

    <!-- Empty state -->
    <div v-else-if="!report && !generating" class="state-box empty">
      <div class="empty-icon">📋</div>
      <p>{{ selectedDate }} 的日报还没有生成</p>
      <p class="hint">点击右上角「生成日报」，AI 会读取 {{ selectedDate }} 当天所有 AI 编程工具的对话文件，总结这天你和 AI 聊了什么、做了什么工作</p>
      <p class="hint-sub">💾 数据来源：本地磁盘 ~/.claude/ ~/.codex/ ~/.hermes/ 等 9 个 AI 工具的对话文件，按文件修改日期匹配</p>
    </div>

    <!-- Report content -->
    <div v-else-if="report" class="report-content">
      <div class="report-meta">
        <span>📊 分析了 {{ report.source_count }} 条编程对话记录</span>
        <span>🕐 生成时间：{{ formatDisplayTime(report.generated_at) }}</span>
        <span v-if="report.from_cache" class="cache-tag">（缓存）</span>
        <span class="storage-hint" title="数据存储在本地的 data/skills_analyzer.db 数据库中 → daily_reports 表">💾 本地存储</span>
      </div>
      <div
        class="markdown-body"
        v-html="report.content
          .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
          .replace(/^### (.+)$/gm, '<h4>$1</h4>')
          .replace(/^## (.+)$/gm, '<h3>$1</h3>')
          .replace(/^# (.+)$/gm, '<h2>$1</h2>')
          .replace(/^- (.+)$/gm, '<li>$1</li>')
          .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
          .replace(/\*(.+?)\*/g, '<em>$1</em>')
          .replace(/`([^`]+)`/g, '<code>$1</code>')
          .replace(/\n\n/g, '</p><p>')
          .replace(/\n/g, '<br>')"
      ></div>
    </div>
  </div>
</template>

<style scoped>
.daily-report-page {
  padding: 24px 32px;
  max-width: 960px;
  margin: 0 auto;
  color: #1e293b;
}

.page-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  margin-bottom: 24px;
  gap: 16px;
  flex-wrap: wrap;
}

.header-left h2 {
  margin: 0 0 4px;
  font-size: 24px;
  font-weight: 700;
  color: #0f172a;
}

.header-subtitle {
  font-size: 14px;
  color: #64748b;
}

.header-right {
  display: flex;
  gap: 10px;
  align-items: center;
}

.date-picker {
  padding: 9px 14px;
  border-radius: 10px;
  border: 1.5px solid #d1d5db;
  background: #fff;
  color: #1e293b;
  font-size: 14px;
  outline: none;
  box-shadow: 0 1px 2px rgba(0,0,0,.04);
}

.date-picker:focus { border-color: #0ea5e9; box-shadow: 0 0 0 3px rgba(14,165,233,.12); }

.btn-generate {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 10px 24px;
  border-radius: 10px;
  border: none;
  background: linear-gradient(135deg, #0ea5e9, #0284c7);
  color: #fff;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: 0.2s;
  white-space: nowrap;
  box-shadow: 0 2px 8px rgba(14,165,233,.25);
}

.btn-generate:hover:not(:disabled) {
  background: linear-gradient(135deg, #0284c7, #0369a1);
  box-shadow: 0 4px 14px rgba(14,165,233,.35);
  transform: translateY(-1px);
}
.btn-generate:disabled { opacity: 0.5; cursor: not-allowed; }

.spinner {
  display: inline-block;
  width: 14px; height: 14px;
  border: 2px solid rgba(255,255,255,0.4);
  border-top-color: #fff;
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}
.spinner.large { width: 32px; height: 32px; border-width: 3px; margin-bottom: 14px; border-top-color: #0ea5e9; border-color: rgba(14,165,233,.15); }
.spinner.small { width: 14px; height: 14px; border-width: 2px; flex-shrink: 0; border-top-color: #0ea5e9; border-color: rgba(14,165,233,.15); }

@keyframes spin { to { transform: rotate(360deg); } }

/* Generation progress */
.gen-progress-bar {
  margin-bottom: 24px;
  padding: 18px 20px;
  border-radius: 14px;
  background: linear-gradient(135deg, #f0f9ff, #e0f2fe);
  border: 1.5px solid #bae6fd;
}

.gen-progress-info {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 14px;
  font-weight: 600;
  color: #0c4a6e;
  margin-bottom: 12px;
}

.gen-progress-pct {
  margin-left: auto;
  font-variant-numeric: tabular-nums;
  color: #0284c7;
  font-size: 16px;
}

.progress-track {
  height: 8px;
  border-radius: 4px;
  background: #e0f2fe;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  border-radius: 4px;
  background: linear-gradient(90deg, #0ea5e9, #06b6d4);
  transition: width 0.5s ease;
}

.gen-progress-hint {
  margin: 10px 0 0;
  font-size: 12px;
  color: #64748b;
}

/* History bar */
.history-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 24px;
  padding: 12px 18px;
  border-radius: 12px;
  background: #f8fafc;
  border: 1.5px solid #e2e8f0;
}

.history-label { font-size: 13px; color: #64748b; font-weight: 500; margin-right: 4px; }

.date-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 14px;
  border-radius: 20px;
  border: 1.5px solid #e2e8f0;
  background: #fff;
  color: #475569;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: 0.15s;
}

.date-chip:hover { border-color: #0ea5e9; color: #0c4a6e; background: #f0f9ff; }
.date-chip.active { border-color: #0ea5e9; background: #0ea5e9; color: #fff; }

.chip-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 20px;
  height: 18px;
  padding: 0 6px;
  border-radius: 9px;
  background: rgba(14,165,233,.12);
  font-size: 11px;
  font-weight: 600;
}
.date-chip.active .chip-badge { background: rgba(255,255,255,.25); }

/* State boxes */
.state-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 64px 24px;
  border-radius: 16px;
  border: 2px dashed #d1d5db;
  background: #fafafa;
  text-align: center;
}

.state-box.empty .empty-icon { font-size: 48px; margin-bottom: 16px; }
.state-box p { margin: 0; color: #475569; font-size: 15px; }
.state-box .hint { margin-top: 10px; font-size: 13px; color: #94a3b8; max-width: 460px; line-height: 1.6; }
.state-box .hint-sub { margin-top: 6px; font-size: 12px; color: #b0b7c3; }

.cache-tag { color: #d97706; font-weight: 500; }
.storage-hint { margin-left: auto; color: #94a3b8; cursor: help; font-size: 12px; }

/* Report content */
.report-content {
  background: #fff;
  border-radius: 16px;
  padding: 28px 32px;
  border: 1px solid #e2e8f0;
  box-shadow: 0 1px 4px rgba(0,0,0,.04);
}

.report-meta {
  display: flex;
  gap: 20px;
  flex-wrap: wrap;
  font-size: 13px;
  color: #64748b;
  margin-bottom: 20px;
  padding-bottom: 16px;
  border-bottom: 1.5px solid #f1f5f9;
  font-weight: 500;
}

.markdown-body { line-height: 1.9; font-size: 15px; color: #1e293b; }

.markdown-body :deep(h2) {
  font-size: 22px; font-weight: 700; color: #0f172a;
  margin: 28px 0 14px; padding-bottom: 10px;
  border-bottom: 2px solid #e0f2fe;
}
.markdown-body :deep(h3) {
  font-size: 18px; font-weight: 700; color: #1e293b;
  margin: 22px 0 10px;
}
.markdown-body :deep(h4) {
  font-size: 16px; font-weight: 600; color: #334155;
  margin: 18px 0 8px;
}
.markdown-body :deep(p) { margin: 0 0 12px; }
.markdown-body :deep(li) {
  margin: 4px 0 4px 24px; color: #475569;
}
.markdown-body :deep(strong) { color: #0f172a; font-weight: 700; }
.markdown-body :deep(code) {
  padding: 2px 8px; border-radius: 5px;
  background: #f0f9ff; color: #0369a1; font-size: 13px;
  font-weight: 500;
}
.markdown-body :deep(em) { color: #64748b; }
</style>
