<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { VulnScanJob } from '../../api/vuln'

const { t } = useI18n()

const props = defineProps<{
  scanning?: boolean
  result?: VulnScanJob | null
  progressMessages?: string[]
}>()

// Step ids are stable identifiers; their display text lives in
// `vuln.common.steps.*`. Keywords below match the backend's Chinese progress
// log lines (see VulnScanServiceImpl), so they stay Chinese on purpose.
const scanSteps = [
  'validate',
  'loginState',
  'strategy',
  'crawl',
  'injection',
  'sensitivePaths',
  'tls',
  'systemVulns',
  'aiDiscover',
  'aiReview',
  'save',
]

const scanStepDescriptions: Record<string, string> = {
  validate: 'vuln.progress.stepDesc.validate',
  loginState: 'vuln.progress.stepDesc.loginState',
  strategy: 'vuln.progress.stepDesc.strategy',
  crawl: 'vuln.progress.stepDesc.crawl',
  injection: 'vuln.progress.stepDesc.injection',
  sensitivePaths: 'vuln.progress.stepDesc.sensitivePaths',
  tls: 'vuln.progress.stepDesc.tls',
  systemVulns: 'vuln.progress.stepDesc.systemVulns',
  aiDiscover: 'vuln.progress.stepDesc.aiDiscover',
  aiReview: 'vuln.progress.stepDesc.aiReview',
  save: 'vuln.progress.stepDesc.save',
}

const stepKeywords: Record<string, string[]> = {
  validate: ['校验目标', '规范化 URL', '目标'], // i18n-exempt: matches Chinese SSE progress lines emitted by the Java backend, never rendered
  loginState: ['加载登录态', 'Cookie', 'Authorization'], // i18n-exempt: backend SSE progress keyword matcher
  strategy: ['扫描策略', '快速扫描', '标准扫描', '深度扫描'], // i18n-exempt: backend SSE progress keyword matcher
  crawl: ['爬取页面', '爬取入口', '读取源码文件'], // i18n-exempt: backend SSE progress keyword matcher
  injection: ['SQL 注入', 'XSS', 'SSRF', 'NoSQL', 'SSTI', 'LFI', 'SQL错误', '布尔盲注', '模板注入', '文件包含'], // i18n-exempt: backend SSE progress keyword matcher
  sensitivePaths: ['敏感路径', '端口扫描', '开放端口', 'TCP'], // i18n-exempt: backend SSE progress keyword matcher
  tls: ['TLS', 'HTTPS', '证书', 'SSL'], // i18n-exempt: backend SSE progress keyword matcher
  systemVulns: ['系统漏洞检测', 'HTTP 方法', 'CRLF', 'Host头', '默认凭据', '源码泄露', '误报控制'], // i18n-exempt: backend SSE progress keyword matcher
  aiDiscover: ['AI 智能发现', '6角色并行分析'], // i18n-exempt: backend SSE progress keyword matcher
  aiReview: ['AI 复核', '6角色并行复核', '模型复核'], // i18n-exempt: backend SSE progress keyword matcher
  save: ['保存结果', '扫描完成'], // i18n-exempt: backend SSE progress keyword matcher
}

function progressLines(text?: string): string[] {
  if (!text) return []
  return text.split('\n').map(line => line.trim()).filter(Boolean)
}

function parseProgressMessage(msg: string): { step: number; text: string } | null {
  try {
    const json = JSON.parse(msg)
    if (typeof json.step === 'number' && typeof json.msg === 'string') {
      return { step: json.step, text: json.msg }
    }
  } catch {}
  return null
}

interface StepGroup {
  step: string
  index: number
  status: 'done' | 'running' | 'pending'
  description: string
  lines: string[]
}

const groups = computed<StepGroup[]>(() => {
  const messages = props.progressMessages || []
  const allText = props.result?.progressText || messages.join('\n')
  const lines = progressLines(allText)

  return scanSteps.map((step, idx) => {
    const hasMatch = lines.some(line => {
      const kws = stepKeywords[step] || []
      return kws.some(kw => line.includes(kw))
    })
    const stepLines = lines.filter(line => {
      const kws = stepKeywords[step] || []
      return kws.some(kw => line.includes(kw))
    })
    let status: 'done' | 'running' | 'pending' = 'pending'
    if (props.result) {
      status = 'done'
    } else if (props.scanning && hasMatch) {
      let nextIdx = -1
      for (let i = scanSteps.length - 1; i > idx; i--) {
        if (lines.some(l => (stepKeywords[scanSteps[i]] || []).some(kw => l.includes(kw)))) {
          nextIdx = i
          break
        }
      }
      status = nextIdx === -1 ? 'running' : 'done'
    }
    return { step, index: idx, status, description: scanStepDescriptions[step] || '', lines: props.result ? stepLines : stepLines.slice(0, 8) }
  })
})

const percent = computed(() => {
  if (props.result) return 100
  if (props.scanning) {
    const done = groups.value.filter(g => g.status === 'done').length
    const running = groups.value.some(g => g.status === 'running') ? 0.5 : 0
    return Math.min(99, Math.max(5, Math.round(((done + running) / scanSteps.length) * 100)))
  }
  return 0
})
</script>

<template>
  <div v-if="scanning || result" class="scan-progress-panel">
    <div class="scan-progress-header">
      <div>
        <h3>{{ result ? t('vuln.progress.done') : t('vuln.progress.scanning') }}</h3>
        <p v-if="result">{{ result.target }}</p>
      </div>
      <el-tag :type="result ? 'success' : 'warning'" effect="light">
        {{ result ? t('vuln.progress.completed') : t('vuln.progress.inProgress', { percent }) }}
      </el-tag>
    </div>
    <div class="progress-bar-track">
      <div class="progress-bar-fill" :style="{ width: percent + '%' }"></div>
    </div>
    <div class="scan-progress-grid">
      <section
        v-for="group in groups"
        :key="group.step"
        class="scan-step-card"
        :class="`scan-step-${group.status}`"
      >
        <div class="scan-step-head">
          <span class="scan-step-index">{{ group.index + 1 }}</span>
          <div>
            <h4>{{ t('vuln.common.steps.' + group.step) }}</h4>
            <p>{{ group.description ? t(group.description) : '' }}</p>
          </div>
          <el-tag v-if="group.status === 'done'" size="small" type="success">{{ t('vuln.common.done') }}</el-tag>
          <el-tag v-else-if="group.status === 'running'" size="small" type="warning">{{ t('vuln.progress.statusRunning') }}</el-tag>
          <el-tag v-else size="small" type="info">{{ t('vuln.progress.statusPending') }}</el-tag>
        </div>
        <div class="scan-step-lines">
          <div v-if="group.lines.length === 0" class="scan-step-empty">{{ t('vuln.progress.noDetail') }}</div>
          <div v-for="(line, i) in group.lines" :key="i" class="scan-step-line">
            <span class="scan-step-dot"></span>
            <span>{{ line }}</span>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>
