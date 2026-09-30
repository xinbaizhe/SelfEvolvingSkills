<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { useTeamStore } from '../stores/useTeamStore'
import { useVulnStore } from '../stores/useVulnStore'
import { getErrorMessage } from '../utils/error'
import { evaluateShareResource, fetchLlmConfig, testLlmConnection } from '../api/admin'
import { fetchAvailableModels, type TeamModelConfig } from '../api/team'
import LoginDialog from '../components/team/LoginDialog.vue'
import UrlScanForm from '../components/vuln/UrlScanForm.vue'
import ScanProgressPanel from '../components/vuln/ScanProgressPanel.vue'
import ScanResultPanel from '../components/vuln/ScanResultPanel.vue'
import type { VulnScanJob, VulnFinding, AgentCredential } from '../api/vuln'

const { t } = useI18n()
const store = useTeamStore()
const vulnStore = useVulnStore()
const loginDialog = ref<InstanceType<typeof LoginDialog> | null>(null)

const activeTab = ref('url')
const urlInput = ref('')
const urlCookie = ref('')
const urlAuthorization = ref('')
const urlHeaders = ref('')
const urlCustomPaths = ref('')
const portScanEnabled = ref(false)
const portSpec = ref('')
const scanProfile = ref<'quick' | 'standard' | 'deep'>('standard')
const maxDepth = ref(2)
const maxPages = ref(24)
const dirInput = ref('')
const showHistory = ref(false)
const historyPage = ref(1)
const historyPageSize = 10
const modelType = ref<'department' | 'personal'>('department')
const modelId = ref<number | undefined>(undefined)
const models = ref<TeamModelConfig[]>([])
const localResourceModel = ref<TeamModelConfig | null>(null)
const localLlmConfig = ref<Record<string, unknown> | null>(null)
const modelLoading = ref(false)
const modelLoadError = ref('')
const scanStep = ref(0)
const intelQuery = ref('')
const intelType = ref('')
const intelSeverity = ref('')
const intelStartDate = ref('')
const intelEndDate = ref('')
const intelPage = ref(1)
const intelPageSize = 10

// Dep monitor state
const depMonitorName = ref('')
const depUploadFiles = ref<{ name: string; content: string }[]>([])
const depFileInput = ref<HTMLInputElement | null>(null)
const expandedDepId = ref<number | null>(null)

const useAgent = ref(false)
const agentCredentials = ref<AgentCredential[]>([])

function addCredential() {
  agentCredentials.value = [...agentCredentials.value, {
    credId: 'cred' + Date.now(),
    role: 'user',
    username: '',
    cookie: '',
    authorization: '',
    permissions: '',
    sessionValid: true,
  }]
}

function removeCredential(idx: number) {
  agentCredentials.value = agentCredentials.value.filter((_, i) => i !== idx)
}

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

const scanProfileOptions = [
  {
    value: 'quick',
    labelKey: 'vuln.common.profile.quick.label',
    titleKey: 'vuln.common.profile.quick.title',
    descriptionKey: 'vuln.common.profile.quick.description',
  },
  {
    value: 'standard',
    labelKey: 'vuln.common.profile.standard.label',
    titleKey: 'vuln.common.profile.standard.title',
    descriptionKey: 'vuln.common.profile.standard.description',
  },
  {
    value: 'deep',
    labelKey: 'vuln.common.profile.deep.label',
    titleKey: 'vuln.common.profile.deep.title',
    descriptionKey: 'vuln.common.profile.deep.description',
  },
] as const

const selectedScanProfile = computed(() =>
  scanProfileOptions.find(item => item.value === scanProfile.value) || scanProfileOptions[1],
)

const scanStepDescriptions: Record<string, string> = {
  validate: 'vuln.scanner.stepDesc.validate',
  loginState: 'vuln.scanner.stepDesc.loginState',
  strategy: 'vuln.scanner.stepDesc.strategy',
  crawl: 'vuln.scanner.stepDesc.crawl',
  injection: 'vuln.scanner.stepDesc.injection',
  sensitivePaths: 'vuln.scanner.stepDesc.sensitivePaths',
  tls: 'vuln.scanner.stepDesc.tls',
  systemVulns: 'vuln.scanner.stepDesc.systemVulns',
  aiDiscover: 'vuln.scanner.stepDesc.aiDiscover',
  aiReview: 'vuln.scanner.stepDesc.aiReview',
  save: 'vuln.scanner.stepDesc.save',
}

const scanStepKeywords: Record<string, string[]> = {
  validate: ['校验目标', '准备扫描目标', '规范化 URL', '目标'], // i18n-exempt: matches Chinese SSE progress lines emitted by the Java backend, never rendered
  loginState: ['加载登录态', 'Cookie', 'Authorization', '自定义请求头', '登录态'], // i18n-exempt: backend SSE progress keyword matcher
  strategy: ['扫描策略', '快速扫描', '标准扫描', '深度扫描', 'maxDepth', 'maxPages'], // i18n-exempt: backend SSE progress keyword matcher
  crawl: ['爬取页面', '爬取入口', '读取源码文件', '读取文件'], // i18n-exempt: backend SSE progress keyword matcher
  injection: ['SQL 注入', 'XSS', 'CSRF', 'SSRF', 'NoSQL', 'SSTI', 'LFI', 'SQL错误', '布尔盲注', '反射型', '模板注入', '文件包含', '目录穿越'], // i18n-exempt: backend SSE progress keyword matcher
  sensitivePaths: ['敏感路径', 'heapdump', 'swagger', 'api-docs', 'actuator', '端口扫描', '开放端口', 'TCP'], // i18n-exempt: backend SSE progress keyword matcher
  tls: ['TLS 检查', 'HTTPS', '证书', 'SSL'], // i18n-exempt: backend SSE progress keyword matcher
  systemVulns: ['系统漏洞检测', 'HTTP 方法', 'CRLF', 'Host头', 'Host 头', '默认凭据', '源码泄露', '方法探测', '误报控制'], // i18n-exempt: backend SSE progress keyword matcher
  aiDiscover: ['AI 智能发现', '6角色并行分析', '大模型额外发现'], // i18n-exempt: backend SSE progress keyword matcher
  aiReview: ['AI 复核', '6角色并行复核', '模型复核'], // i18n-exempt: backend SSE progress keyword matcher
  save: ['保存结果', '保存扫描结果', '扫描完成'], // i18n-exempt: backend SSE progress keyword matcher
}

const filteredModels = computed(() => {
  if (modelType.value === 'department') {
    return models.value
  }
  return localResourceModel.value ? [localResourceModel.value] : []
})

const historyScanProgressGroups = computed(() => buildScanProgressGroups(vulnStore.selectedHistoryJob))

function severityType(severity: string): string {
  const map: Record<string, string> = {
    CRITICAL: 'danger',
    HIGH: 'warning',
    MEDIUM: '',
    LOW: 'info',
  }
  return map[severity] || 'info'
}

const SEVERITY_LABEL_KEYS: Record<string, string> = {
  CRITICAL: 'vuln.common.severity.critical',
  HIGH: 'vuln.common.severity.high',
  MEDIUM: 'vuln.common.severity.medium',
  LOW: 'vuln.common.severity.low',
}

function severityLabel(severity: string): string {
  const key = SEVERITY_LABEL_KEYS[severity]
  return key ? t(key) : severity
}

function confidenceClass(confidence: number): string {
  if (confidence >= 80) return 'conf-high'
  if (confidence >= 60) return 'conf-mid'
  return 'conf-low'
}

function scanTypeLabel(type: string): string {
  return t(type === 'url' ? 'vuln.common.scanType.url' : 'vuln.common.scanType.code')
}

function modelLabel(model: TeamModelConfig): string {
  const name = model.id === -1 ? t('vuln.scanner.localModelConfig') : model.name
  return `${name} / ${model.model}`
}

async function handleUrlScan() {
  if (!urlInput.value.trim()) {
    ElMessage.warning(t('vuln.scanner.enterUrl'))
    return
  }
  if (!await validateEvaluationModel()) return
  scanStep.value = 1
  scanStep.value = 2

  let result: VulnScanJob | null
  if (useAgent.value) {
    result = await vulnStore.runUrlAgentStream(urlInput.value.trim(), {
      modelType: modelType.value,
      modelId: modelId.value,
      agentCredentials: agentCredentials.value,
    })
  } else {
    result = await vulnStore.runUrlScanStream(urlInput.value.trim(), {
      modelType: modelType.value,
      modelId: modelId.value,
      cookie: urlCookie.value.trim() || undefined,
      authorization: urlAuthorization.value.trim() || undefined,
      headers: urlHeaders.value.trim() || undefined,
      scanProfile: scanProfile.value,
      customPaths: urlCustomPaths.value.trim() || undefined,
      maxDepth: maxDepth.value,
      maxPages: maxPages.value,
      portScanEnabled: portScanEnabled.value,
      portSpec: portSpec.value.trim() || undefined,
    })
  }
  if (result) {
    try {
      if (modelType.value === 'personal') {
        scanStep.value = 9
        await reviewResultWithLocalModel(result)
      }
      scanStep.value = scanSteps.length
      ElMessage.success(t('vuln.scanner.scanComplete', { n: result.totalFindings }))
    } catch (e) {
      scanStep.value = scanSteps.length
      ElMessage.error(getErrorMessage(e))
    }
  } else if (vulnStore.error) {
    ElMessage.error(vulnStore.error)
  }
}

// Auto-advance scanStep based on SSE progress keywords
const { scanProgress: progressRef } = storeToRefs(vulnStore)
watch(progressRef, (messages) => {
  if (!vulnStore.scanning || !messages || messages.length === 0) return
  let bestIdx = scanStep.value - 1
  for (let i = 0; i < scanSteps.length; i++) {
    const keywords = scanStepKeywords[scanSteps[i]] || []
    if (messages.some(msg => keywords.some(kw => msg.includes(kw)))) {
      bestIdx = i
    }
  }
  if (bestIdx + 1 > scanStep.value) {
    scanStep.value = bestIdx + 1
  }
})

async function handleCodeScan() {
  if (!dirInput.value.trim()) {
    ElMessage.warning(t('vuln.scanner.enterDir'))
    return
  }
  if (!await validateEvaluationModel()) return
  scanStep.value = 1
  scanStep.value = 2
  const result = await vulnStore.runCodeScan(dirInput.value.trim(), { modelType: modelType.value, modelId: modelId.value })
  if (result) {
    try {
      if (modelType.value === 'personal') {
        scanStep.value = 9
        await reviewResultWithLocalModel(result)
      }
      scanStep.value = scanSteps.length
      ElMessage.success(t('vuln.scanner.scanComplete', { n: result.totalFindings }))
    } catch (e) {
      scanStep.value = scanSteps.length
      ElMessage.error(getErrorMessage(e))
    }
  } else if (vulnStore.error) {
    ElMessage.error(vulnStore.error)
  }
}

async function loadModels() {
  modelLoading.value = true
  modelLoadError.value = ''
  try {
    const [departmentModels] = await Promise.all([
      fetchAvailableModels(),
      loadLocalResourceModel(),
    ])
    models.value = departmentModels
    if (!modelId.value) modelId.value = filteredModels.value[0]?.id
  } catch (e) {
    modelLoadError.value = getErrorMessage(e)
    models.value = []
    ElMessage.error(t('vuln.scanner.deptModelLoadFailed', { error: modelLoadError.value }))
  } finally {
    modelLoading.value = false
  }
}

async function loadLocalResourceModel() {
  try {
    const res = await fetchLlmConfig()
    const data = res.success ? (res.data as {
      enabled?: boolean
      provider?: string
      base_url?: string
      model?: string
      api_format?: string
      api_key_configured?: boolean
      has_api_key?: boolean
    } | null) : null
    if (!data?.enabled || !data.model || !data.base_url) {
      localResourceModel.value = null
      localLlmConfig.value = null
      return
    }
    localLlmConfig.value = data as Record<string, unknown>
    localResourceModel.value = {
      id: -1,
      name: '',
      provider: data.provider || data.api_format || 'custom',
      baseUrl: data.base_url,
      model: data.model,
      deptId: null,
      sourceType: 'personal',
      apiKeyHash: data.api_key_configured || data.has_api_key ? 'configured' : undefined,
      isActive: 1,
      createdAt: '',
    }
  } catch {
    localResourceModel.value = null
    localLlmConfig.value = null
  }
}

async function reviewResultWithLocalModel(result: VulnScanJob) {
  const payload = {
    metadata: {
      shareType: 'vulnerability-scan',
      name: result.target,
      scanType: result.scanType,
      evaluationModelType: 'personal',
      evaluationModel: localResourceModel.value?.model || '',
    },
    heuristic: {
      score: Math.max(0, 100 - result.criticalCount * 25 - result.highCount * 15 - result.mediumCount * 8 - result.lowCount * 3),
      securityScore: Math.max(0, 100 - result.criticalCount * 25 - result.highCount * 15 - result.mediumCount * 8 - result.lowCount * 3),
      performanceScore: 100,
      penalties: result.findings.map(item => `${item.severity} ${item.type}: ${item.description}`),
    },
    content: [
      `# 漏洞扫描结果`, // i18n-exempt: LLM evaluation payload sent to evaluateShareResource, not UI
      `目标: ${result.target}`, // i18n-exempt: LLM evaluation payload
      `类型: ${result.scanType}`, // i18n-exempt: LLM evaluation payload
      '',
      ...result.findings.map(item => [
        `## ${item.severity} ${item.type}`,
        `位置: ${item.location}`, // i18n-exempt: LLM evaluation payload
        `描述: ${item.description}`, // i18n-exempt: LLM evaluation payload
        `建议: ${item.suggestion}`, // i18n-exempt: LLM evaluation payload
      ].join('\n')),
    ].join('\n\n'),
  }
  const res = await evaluateShareResource(payload)
  if (!res.success) {
    throw new Error(res.error || t('vuln.scanner.localModelReviewFailed'))
  }
  const data = res.data as {
    score?: number
    securityScore?: number
    performanceScore?: number
    summary?: string
    risks?: string[]
    suggestions?: string[]
    requiredChanges?: string[]
  }
  const lines = [
    t('vuln.scanner.localReviewLog'),
    t('vuln.scanner.aiReviewScores', {
      score: data.score ?? '-',
      security: data.securityScore ?? '-',
      performance: data.performanceScore ?? '-',
    }),
    data.summary ? t('vuln.scanner.aiReviewSummary', { summary: data.summary }) : '',
    ...(data.risks || []).map(item => t('vuln.scanner.aiRisk', { item })),
    ...(data.suggestions || []).map(item => t('vuln.scanner.aiSuggestion', { item })),
    ...(data.requiredChanges || []).map(item => t('vuln.scanner.aiRequiredChange', { item })),
  ].filter(Boolean)
  localReviewLines.value = [...localReviewLines.value, ...lines]
  result.progressText = [result.progressText, ...lines].filter(Boolean).join('\n')
}

async function validateEvaluationModel(): Promise<boolean> {
  if (modelType.value === 'department') {
    const model = filteredModels.value.find(item => item.id === modelId.value)
    if (!model) {
      ElMessage.error(t('vuln.scanner.selectDeptModel'))
      return false
    }
    if (!model.baseUrl || !model.model || !model.apiKeyHash) {
      ElMessage.error(t('vuln.scanner.deptModelInvalid'))
      return false
    }
    return true
  }

  await loadLocalResourceModel()
  if (!localResourceModel.value || !localLlmConfig.value) {
    ElMessage.error(t('vuln.scanner.localModelInvalid'))
    return false
  }
  if (!localResourceModel.value.apiKeyHash) {
    ElMessage.error(t('vuln.scanner.localModelMissingKey'))
    return false
  }
  const payload = {
    llm_enabled: true,
    llm_provider: localLlmConfig.value.provider || 'custom',
    llm_base_url: localLlmConfig.value.base_url,
    llm_model: localLlmConfig.value.model,
    llm_api_key: '',
    llm_api_format: localLlmConfig.value.api_format || localLlmConfig.value.provider || 'openai',
  }
  try {
    const res = await testLlmConnection(payload)
    if (!res.success) throw new Error(res.error || t('vuln.scanner.modelTestFailed'))
    return true
  } catch (e) {
    ElMessage.error(t('vuln.scanner.modelConfigBroken', { detail: getErrorMessage(e) }))
    return false
  }
}

watch(modelType, () => {
  modelId.value = filteredModels.value[0]?.id
})

const pagedIntel = computed(() => {
  const start = (intelPage.value - 1) * intelPageSize
  return vulnStore.intel.slice(start, start + intelPageSize)
})

async function queryIntel() {
  const params: Record<string, string> = {}
  if (intelQuery.value.trim()) params.keyword = intelQuery.value.trim()
  if (intelType.value) params.vulnType = intelType.value
  if (intelSeverity.value) params.severity = intelSeverity.value
  if (intelStartDate.value) params.startDate = intelStartDate.value
  if (intelEndDate.value) params.endDate = intelEndDate.value
  intelPage.value = 1
  await vulnStore.loadIntel(params)
}

async function toggleHistory() {
  showHistory.value = !showHistory.value
  if (showHistory.value) {
    historyPage.value = 1
    await vulnStore.loadHistory(historyPage.value, historyPageSize)
  } else {
    vulnStore.clearHistoryDetail()
  }
}

async function onHistoryPageChange(page: number) {
  historyPage.value = page
  await vulnStore.loadHistory(page, historyPageSize)
}

async function viewHistoryDetail(job: VulnScanJob) {
  await vulnStore.loadHistoryDetail(job.id)
}

function formatTime(dateStr: string): string {
  if (!dateStr) return '-'
  return dateStr.slice(0, 16).replace('T', ' ')
}

function progressLines(text?: string): string[] {
  if (!text) return []
  return text.split('\n').map(line => line.trim()).filter(Boolean)
}

/**
 * Lines this view writes itself after a local-model review that are folded into
 * the scan's progress text.
 *
 * They are attributed to their step by identity rather than by keyword:
 * `scanStepKeywords` describes the backend's log lines, which are always
 * Chinese, whereas these are rendered in the interface language and would fall
 * through to "Other" in every non-Chinese locale.
 */
const localReviewLines = ref<string[]>([])

function buildScanProgressGroups(result?: VulnScanJob | null) {
  const lines = progressLines(result?.progressText)
  const streamLines = vulnStore.scanning ? vulnStore.scanProgress : []
  const allLines = streamLines.length > 0 ? streamLines : lines
  const localReview = new Set(localReviewLines.value)
  const grouped = scanSteps.map((step, index) => {
    const stepLines = allLines.filter(line => {
      if (localReview.has(line)) return step === 'aiReview'
      return lineBelongsToStep(line, step)
    })
    const doneByResult = !!result
    const active = !result && vulnStore.scanning && index === Math.max(scanStep.value - 1, 0)
    return {
      step,
      index,
      labelKey: `vuln.common.steps.${step}`,
      descriptionKey: scanStepDescriptions[step],
      lines: stepLines,
      status: doneByResult ? 'done' : active ? 'running' : index < scanStep.value ? 'done' : 'pending',
    }
  })

  const matched = new Set(grouped.flatMap(group => group.lines))
  const unmatched = allLines.filter(line => !matched.has(line))
  if (unmatched.length) {
    grouped.push({
      step: 'other',
      index: grouped.length,
      labelKey: 'vuln.scanner.otherSteps',
      descriptionKey: 'vuln.scanner.otherStepsDesc',
      lines: unmatched,
      status: 'done',
    })
  }
  return grouped
}

function lineBelongsToStep(line: string, step: string): boolean {
  const normalized = line.replace(/^\d+[.、]\s*/, '')
  // Kept as-is: backend lines are Chinese narration, so this rarely fires, but
  // tightening the matcher is not what this change is about.
  if (normalized.startsWith(step)) return true
  return (scanStepKeywords[step] || []).some(keyword => normalized.includes(keyword))
}

// Dep monitor handlers
async function handleDepFilesSelected(event: Event) {
  const target = event.target as HTMLInputElement
  if (!target.files) return
  const files: { name: string; content: string }[] = []
  for (const file of Array.from(target.files)) {
    const content = await file.text()
    files.push({ name: file.name, content })
  }
  depUploadFiles.value = files
  if (!depMonitorName.value.trim()) {
    depMonitorName.value = files[0]?.name.replace(/\.[^.]+$/, '') || ''
  }
}

async function handleDepDrop(event: DragEvent) {
  const dt = event.dataTransfer
  if (!dt?.files) return
  const files: { name: string; content: string }[] = []
  for (const file of Array.from(dt.files)) {
    const content = await file.text()
    files.push({ name: file.name, content })
  }
  depUploadFiles.value = files
  if (!depMonitorName.value.trim()) {
    depMonitorName.value = files[0]?.name.replace(/\.[^.]+$/, '') || ''
  }
}

async function handleDepUpload() {
  if (!depMonitorName.value.trim() || depUploadFiles.value.length === 0) {
    ElMessage.warning(t('vuln.scanner.enterProjectAndFiles'))
    return
  }
  const result = await vulnStore.uploadMonitor(depMonitorName.value.trim(), depUploadFiles.value)
  if (result) {
    ElMessage.success(t('vuln.scanner.depParsed', { n: result.deps.length }))
    depUploadFiles.value = []
  } else if (vulnStore.error) {
    ElMessage.error(vulnStore.error)
  }
}

onMounted(() => {
  if (store.isAuthenticated) {
    vulnStore.loadHistory(1, historyPageSize)
    vulnStore.loadIntel()
    loadModels()
  }
})
</script>

<template>
  <div class="page-view">
    <div class="page-header">
      <div>
        <h2>{{ t('vuln.scanner.title') }}</h2>
        <p class="subtitle">{{ t('vuln.scanner.subtitle') }}</p>
      </div>
      <div class="header-actions">
        <el-button @click="toggleHistory">
          {{ showHistory ? t('vuln.scanner.backToScan') : t('vuln.scanner.history') }}
        </el-button>
      </div>
    </div>

    <div v-if="vulnStore.offline" class="offline-banner">
      <span class="offline-icon">
        <svg width="18" height="18" viewBox="0 0 18 18" fill="none"><path d="M9 1.5C4.86 1.5 1.5 4.86 1.5 9s3.36 7.5 7.5 7.5 7.5-3.36 7.5-7.5S13.14 1.5 9 1.5zM9 6v4M9 12h.01" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>
      </span>
      {{ t('vuln.scanner.offline') }}
      <el-button size="small" @click="vulnStore.loadHistory()">{{ t('common.retry') }}</el-button>
    </div>

    <template v-if="!store.isAuthenticated">
      <div class="login-prompt">
        <div class="login-card">
          <h3>{{ t('vuln.scanner.loginTitle') }}</h3>
          <p>{{ t('vuln.scanner.loginBody') }}</p>
          <el-button type="primary" size="large" @click="loginDialog?.open()">{{ t('app.actions.login') }}</el-button>
        </div>
      </div>
    </template>

    <template v-else-if="showHistory">
      <section class="history-section">
        <h3 class="section-title">{{ t('vuln.scanner.historyTitle') }}</h3>
        <div v-if="vulnStore.history.length === 0 && !vulnStore.offline" class="empty-state">
          <p>{{ t('vuln.scanner.noHistory') }}</p>
        </div>

        <!-- History detail view -->
        <div v-if="vulnStore.selectedHistoryJob" class="history-detail-view">
          <div class="detail-back-row">
            <el-button size="small" @click="vulnStore.clearHistoryDetail()">&larr; {{ t('vuln.scanner.backToHistory') }}</el-button>
          </div>
          <div class="result-section">
            <div class="result-header">
              <div>
                <h3>{{ t('vuln.common.scanDetail') }}</h3>
                <p class="result-target">
                  <el-tag size="small" :type="vulnStore.selectedHistoryJob.scanType === 'url' ? 'primary' : 'success'">
                    {{ scanTypeLabel(vulnStore.selectedHistoryJob.scanType) }}
                  </el-tag>
                  {{ vulnStore.selectedHistoryJob.target }}
                </p>
              </div>
              <div class="result-summary">
                <div class="finding-counts">
                  <div class="finding-badge badge-danger">
                    <span class="badge-count">{{ vulnStore.selectedHistoryJob.criticalCount }}</span>
                    <span class="badge-label">{{ t('vuln.common.severity.critical') }}</span>
                  </div>
                  <div class="finding-badge badge-warning">
                    <span class="badge-count">{{ vulnStore.selectedHistoryJob.highCount }}</span>
                    <span class="badge-label">{{ t('vuln.common.severity.high') }}</span>
                  </div>
                  <div class="finding-badge">
                    <span class="badge-count">{{ vulnStore.selectedHistoryJob.mediumCount }}</span>
                    <span class="badge-label">{{ t('vuln.common.severity.medium') }}</span>
                  </div>
                  <div class="finding-badge badge-info">
                    <span class="badge-count">{{ vulnStore.selectedHistoryJob.lowCount }}</span>
                    <span class="badge-label">{{ t('vuln.common.severity.low') }}</span>
                  </div>
                </div>
              </div>
            </div>
            <div class="history-progress-panel">
              <div class="scan-progress-header">
                <div>
                  <h3>{{ t('vuln.scanner.executionLog') }}</h3>
                  <p>{{ vulnStore.selectedHistoryJob.target }}</p>
                </div>
                <el-tag type="success" effect="light">{{ t('vuln.scanner.saved') }}</el-tag>
              </div>
              <div class="scan-progress-grid">
                <section
                  v-for="group in historyScanProgressGroups"
                  :key="group.step"
                  class="scan-step-card"
                  :class="`scan-step-${group.status}`"
                >
                  <div class="scan-step-head">
                    <span class="scan-step-index">{{ group.index + 1 }}</span>
                    <div>
                      <h4>{{ t(group.labelKey) }}</h4>
                      <p>{{ t(group.descriptionKey) }}</p>
                    </div>
                    <el-tag size="small" type="success">{{ t('vuln.common.done') }}</el-tag>
                  </div>
                  <div class="scan-step-lines">
                    <div v-if="group.lines.length === 0" class="scan-step-empty">{{ t('vuln.scanner.noStepDetail') }}</div>
                    <div
                      v-for="(line, lineIndex) in group.lines"
                      :key="`${group.step}-${lineIndex}-${line}`"
                      class="scan-step-line"
                    >
                      <span class="scan-step-dot"></span>
                      <span>{{ line }}</span>
                    </div>
                  </div>
                </section>
              </div>
            </div>
            <div v-if="vulnStore.selectedHistoryJob.findings.length === 0" class="empty-state safe-state">
              <p class="safe-text">{{ t('vuln.common.noVulnerabilities') }}</p>
            </div>
            <div v-else class="findings-table-wrapper">
              <table class="findings-table">
                <thead>
                  <tr>
                    <th style="width:72px">{{ t('vuln.common.table.severity') }}</th>
                    <th style="width:110px">{{ t('vuln.common.table.type') }}</th>
                    <th style="width:180px">{{ t('vuln.common.table.location') }}</th>
                    <th>{{ t('vuln.common.table.description') }}</th>
                    <th>{{ t('vuln.common.table.suggestion') }}</th>
                    <th style="width:72px">{{ t('vuln.common.table.confidence') }}</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="(f, idx) in vulnStore.selectedHistoryJob.findings" :key="idx">
                    <td><el-tag :type="severityType(f.severity)" size="small" effect="dark">{{ severityLabel(f.severity) }}</el-tag></td>
                    <td><span class="finding-type">{{ f.type }}</span></td>
                    <td><code class="finding-location">{{ f.location }}</code></td>
                    <td class="finding-desc">{{ f.description }}</td>
                    <td class="finding-suggestion">{{ f.suggestion }}</td>
                    <td>
                      <span v-if="f.confidence != null" class="confidence-badge" :class="confidenceClass(f.confidence)">
                        {{ f.confidence }}%
                      </span>
                      <span v-else class="confidence-na">-</span>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        </div>

        <!-- History list -->
        <template v-else>
          <div v-if="vulnStore.history.length > 0" class="history-list">
            <article
              v-for="job in vulnStore.history"
              :key="job.id"
              class="history-card"
              @click="viewHistoryDetail(job)"
            >
              <div class="history-main">
                <div class="history-header">
                  <el-tag size="small" :type="job.scanType === 'url' ? 'primary' : 'success'">
                    {{ scanTypeLabel(job.scanType) }}
                  </el-tag>
                  <span class="history-target">{{ job.target }}</span>
                </div>
                <div class="history-meta">
                  <span>{{ t('vuln.scanner.totalFindings', { n: job.totalFindings }) }}</span>
                  <span v-if="job.criticalCount" class="count-danger">{{ t('vuln.common.severity.critical') }} {{ job.criticalCount }}</span>
                  <span v-if="job.highCount" class="count-warning">{{ t('vuln.common.severity.high') }} {{ job.highCount }}</span>
                  <span v-if="job.mediumCount" class="count-default">{{ t('vuln.common.severity.medium') }} {{ job.mediumCount }}</span>
                  <span>{{ formatTime(job.createdAt) }}</span>
                </div>
              </div>
              <div class="history-arrow">&rarr;</div>
            </article>
          </div>
          <div v-if="vulnStore.historyTotal > historyPageSize" class="history-pagination">
            <el-pagination
              background
              layout="prev, pager, next"
              v-model:current-page="historyPage"
              :page-size="historyPageSize"
              :total="vulnStore.historyTotal"
              @current-change="onHistoryPageChange"
            />
          </div>
        </template>
      </section>
    </template>

    <template v-else>
      <div class="scan-container">
        <div class="model-row">
          <el-segmented
            v-model="modelType"
            :options="[
              { label: t('vuln.scanner.deptModel'), value: 'department' },
              { label: t('vuln.scanner.localModelConfig'), value: 'personal' },
            ]"
            @change="modelId = filteredModels[0]?.id"
          />
          <el-select v-model="modelId" :placeholder="t('vuln.scanner.selectModel')" style="width: 280px">
            <el-option v-for="model in filteredModels" :key="model.id" :label="modelLabel(model)" :value="model.id" />
          </el-select>
          <el-button size="small" :loading="modelLoading" @click="loadModels">{{ t('vuln.scanner.refreshModels') }}</el-button>
          <span v-if="modelType === 'department' && !modelLoading && filteredModels.length === 0" class="model-warning">
            {{ modelLoadError ? t('vuln.scanner.deptModelLoadFailed', { error: modelLoadError }) : t('vuln.scanner.noDeptModel') }}
          </span>
        </div>

        <el-tabs v-model="activeTab" class="scan-tabs">
          <el-tab-pane :label="t('vuln.common.scanType.url')" name="url">
            <UrlScanForm
              v-model="urlInput"
              :scanning="vulnStore.scanning"
              v-model:cookie="urlCookie"
              v-model:authorization="urlAuthorization"
              v-model:headers="urlHeaders"
              v-model:custom-paths="urlCustomPaths"
              v-model:port-scan-enabled="portScanEnabled"
              v-model:port-spec="portSpec"
              v-model:scan-profile="scanProfile"
              v-model:max-depth="maxDepth"
              v-model:max-pages="maxPages"
              v-model:use-agent="useAgent"
              v-model:agent-credentials="agentCredentials"
              @scan="handleUrlScan"
              @add-credential="addCredential"
              @remove-credential="removeCredential"
            />
          </el-tab-pane>

          <el-tab-pane :label="t('vuln.common.scanType.code')" name="code">
            <div class="scan-input-row">
              <el-input
                v-model="dirInput"
                :placeholder="t('vuln.scanner.dirPlaceholder')"
                size="large"
                clearable
                @keyup.enter="handleCodeScan"
              >
                <template #prefix>
                  <span class="input-prefix-icon">
                    <svg width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M2 4.5v-1a1 1 0 011-1h3.5L8 4h4a1 1 0 011 1v1M2 4.5v7a1 1 0 001 1h10a1 1 0 001-1v-7M2 4.5h12" stroke="currentColor" stroke-width="1.2"/></svg>
                  </span>
                </template>
              </el-input>
              <el-button
                type="primary"
                size="large"
                :loading="vulnStore.scanning"
                @click="handleCodeScan"
              >
                {{ vulnStore.scanning ? t('vuln.common.scanning') : t('vuln.common.startScan') }}
              </el-button>
            </div>
            <p class="scan-hint">{{ t('vuln.scanner.codeHint') }}</p>
          </el-tab-pane>

          <el-tab-pane :label="t('vuln.scanner.intelTab')" name="intel">
            <div class="intel-panel" v-loading="vulnStore.intelLoading">
              <div class="intel-header">
                <div>
                  <h3>{{ t('vuln.scanner.intelTitle') }}</h3>
                  <p>{{ t('vuln.scanner.intelSubtitle') }}</p>
                </div>
              </div>
              <div class="intel-filters">
                <el-input v-model="intelQuery" :placeholder="t('vuln.scanner.intelSearchPlaceholder')" clearable style="width: 220px" />
                <el-select v-model="intelType" :placeholder="t('vuln.scanner.intelTypePlaceholder')" clearable filterable allow-create style="width: 170px">
                  <el-option :label="t('vuln.scanner.intelType.sqlInjection')" value="SQL注入" /><!-- i18n-exempt: backend vulnType field value, not display text -->
                  <el-option :label="t('vuln.scanner.intelType.xss')" value="XSS" />
                  <el-option :label="t('vuln.scanner.intelType.rce')" value="远程代码执行" /><!-- i18n-exempt: backend vulnType field value, not display text -->
                  <el-option :label="t('vuln.scanner.intelType.privesc')" value="权限提升" /><!-- i18n-exempt: backend vulnType field value, not display text -->
                  <el-option :label="t('vuln.scanner.intelType.systemVuln')" value="系统漏洞" /><!-- i18n-exempt: backend vulnType field value, not display text -->
                  <el-option :label="t('vuln.scanner.intelType.supplyChain')" value="供应链投毒" /><!-- i18n-exempt: backend vulnType field value, not display text -->
                </el-select>
                <el-select v-model="intelSeverity" :placeholder="t('vuln.scanner.severityPlaceholder')" clearable filterable allow-create style="width: 130px">
                  <el-option :label="t('vuln.common.severity.critical')" value="CRITICAL" />
                  <el-option :label="t('vuln.common.severity.high')" value="HIGH" />
                  <el-option :label="t('vuln.common.severity.medium')" value="MEDIUM" />
                  <el-option :label="t('vuln.common.severity.low')" value="LOW" />
                </el-select>
                <el-date-picker v-model="intelStartDate" format="YYYY-MM-DD" value-format="YYYY-MM-DD" type="date" :placeholder="t('vuln.scanner.startDate')" style="width: 150px" />
                <el-date-picker v-model="intelEndDate" format="YYYY-MM-DD" value-format="YYYY-MM-DD" type="date" :placeholder="t('vuln.scanner.endDate')" style="width: 150px" />
                <el-button type="primary" @click="queryIntel">{{ t('vuln.scanner.query') }}</el-button>
              </div>
              <el-table :data="pagedIntel" size="small" class="intel-table">
                <el-table-column prop="cveId" label="CVE" width="150" />
                <el-table-column prop="vulnType" :label="t('vuln.scanner.colVulnType')" width="130" />
                <el-table-column :label="t('vuln.scanner.poisoning')" width="80">
                  <template #default="{ row }">
                    <el-tag v-if="row.isPoisoning" type="danger" size="small" effect="dark">{{ t('vuln.scanner.poisoning') }}</el-tag>
                  </template>
                </el-table-column>
                <el-table-column prop="severity" :label="t('vuln.scanner.colSeverity')" width="90">
                  <template #default="{ row }">{{ severityLabel(row.severity) }}</template>
                </el-table-column>
                <el-table-column prop="ecosystem" :label="t('vuln.scanner.colEcosystem')" width="90" show-overflow-tooltip />
                <el-table-column prop="vendorProject" :label="t('vuln.scanner.colVendor')" width="150" show-overflow-tooltip />
                <el-table-column prop="product" :label="t('vuln.scanner.colProduct')" width="150" show-overflow-tooltip />
                <el-table-column prop="title" :label="t('vuln.scanner.colTitle')" min-width="260" show-overflow-tooltip />
                <el-table-column prop="publishedAt" :label="t('vuln.scanner.colPublishedAt')" width="160">
                  <template #default="{ row }">{{ formatTime(row.publishedAt) }}</template>
                </el-table-column>
              </el-table>
              <div class="intel-pagination">
                <el-pagination
                  background
                  layout="prev, pager, next"
                  v-model:current-page="intelPage"
                  :page-size="intelPageSize"
                  :total="vulnStore.intel.length"
                />
              </div>
            </div>
          </el-tab-pane>

          <el-tab-pane :label="t('vuln.scanner.monitorTab')" name="monitor">
            <div class="dep-monitor-panel" v-loading="vulnStore.depLoading">
              <!-- Upload area -->
              <div v-if="!vulnStore.snapshots.length && depUploadFiles.length === 0" class="dep-upload-area">
                <div class="dep-upload-drop"
                  @dragover.prevent
                  @drop.prevent="handleDepDrop">
                  <p class="upload-icon">+</p>
                  <p>{{ t('vuln.scanner.dropHint') }}</p>
                  <p class="upload-hint">{{ t('vuln.scanner.uploadFormats') }}</p>
                </div>
                <div class="dep-upload-row">
                  <input type="file" multiple @change="handleDepFilesSelected" accept=".json,.xml,.toml,.txt,.gradle,.kts,.mod,.csproj" style="display:none" ref="depFileInput" />
                  <el-button @click="depFileInput?.click()">{{ t('vuln.scanner.chooseFile') }}</el-button>
                </div>
              </div>

              <!-- Selected files preview -->
              <div v-if="depUploadFiles.length > 0" class="dep-files-preview">
                <h4>{{ t('vuln.scanner.selectedFiles', { n: depUploadFiles.length }) }}</h4>
                <ul>
                  <li v-for="(f, i) in depUploadFiles" :key="i">{{ f.name }} ({{ t('vuln.scanner.charCount', { n: f.content.length }) }})</li>
                </ul>
                <div class="dep-upload-form">
                  <el-input v-model="depMonitorName" :placeholder="t('vuln.scanner.projectName')" style="width: 260px" size="small" />
                  <el-button type="primary" size="small" @click="handleDepUpload" :loading="vulnStore.depLoading">{{ t('vuln.scanner.uploadAnalyze') }}</el-button>
                  <el-button size="small" @click="depUploadFiles = []">{{ t('common.cancel') }}</el-button>
                </div>
              </div>

              <!-- Snapshots list -->
              <div v-if="vulnStore.snapshots.length > 0" class="dep-snapshots">
                <div class="dep-snapshots-header">
                  <h3>{{ t('vuln.scanner.snapshots', { n: vulnStore.snapshots.length }) }}</h3>
                  <el-button size="small" @click="depUploadFiles = []; vulnStore.clearDepDetail()">+ {{ t('vuln.scanner.newSnapshot') }}</el-button>
                </div>
                <div class="dep-snapshot-cards">
                  <div v-for="snap in vulnStore.snapshots" :key="snap.id" class="dep-snapshot-card">
                    <div class="dep-snapshot-main">
                      <div class="dep-snapshot-name">{{ snap.name }}</div>
                      <div class="dep-snapshot-meta">
                        <span :class="{ 'count-danger': snap.poisoningCount > 0 }">{{ t('vuln.scanner.poisoning') }} {{ snap.poisoningCount }}</span>
                        <span>{{ t('vuln.scanner.vulnLabel') }} {{ snap.vulnCount }}</span>
                        <span>{{ formatTime(snap.lastCheckedAt || snap.createdAt) }}</span>
                      </div>
                    </div>
                    <div class="dep-snapshot-actions">
                      <el-button size="small" @click="vulnStore.loadDepDeps(snap.id, snap.name).then(() => expandedDepId = snap.id)">{{ t('vuln.scanner.viewDeps') }}</el-button>
                      <el-button size="small" type="warning" :loading="vulnStore.depLoading" @click="vulnStore.refreshSnapshot(snap.id)">{{ t('common.refresh') }}</el-button>
                      <el-button size="small" type="danger" @click="vulnStore.removeSnapshot(snap.id)">{{ t('common.delete') }}</el-button>
                    </div>
                  </div>
                </div>
              </div>

              <!-- Expanded dependency details -->
              <div v-if="expandedDepId && vulnStore.currentSnapshotDeps.length > 0" class="dep-detail">
                <h4>{{ t('vuln.scanner.depList') }}</h4>
                <el-table :data="vulnStore.currentSnapshotDeps" size="small">
                  <el-table-column prop="ecosystem" :label="t('vuln.scanner.colEcosystem')" width="90" />
                  <el-table-column prop="packageName" :label="t('vuln.scanner.colPackage')" min-width="220" show-overflow-tooltip />
                  <el-table-column prop="version" :label="t('vuln.scanner.colVersion')" width="140" show-overflow-tooltip />
                  <el-table-column :label="t('vuln.scanner.poisoning')" width="70">
                    <template #default="{ row }">
                      <el-tag v-if="row.poisoningCount > 0" type="danger" size="small" effect="dark">{{ row.poisoningCount }}</el-tag>
                      <span v-else>-</span>
                    </template>
                  </el-table-column>
                  <el-table-column :label="t('vuln.scanner.vulnLabel')" width="70">
                    <template #default="{ row }">{{ row.vulnCount > 0 ? row.vulnCount : '-' }}</template>
                  </el-table-column>
                  <el-table-column :label="t('vuln.scanner.colActions')" width="80">
                    <template #default="{ row }">
                      <el-button size="small" @click="vulnStore.loadDepFindings(row.id)">{{ t('common.detail') }}</el-button>
                    </template>
                  </el-table-column>
                </el-table>

                <!-- Findings sub-table -->
                <div v-if="vulnStore.depFindings.length > 0" class="dep-findings">
                  <h5>{{ t('vuln.scanner.relatedFindings', { n: vulnStore.depFindings.length }) }}</h5>
                  <el-table :data="vulnStore.depFindings" size="small">
                    <el-table-column :label="t('vuln.scanner.poisoning')" width="70">
                      <template #default="{ row }">
                        <el-tag v-if="row.isPoisoning" type="danger" size="small" effect="dark">{{ t('vuln.scanner.poisoning') }}</el-tag>
                      </template>
                    </el-table-column>
                    <el-table-column prop="severity" :label="t('vuln.scanner.colSeverity')" width="80">
                      <template #default="{ row }">{{ severityLabel(row.severity) }}</template>
                    </el-table-column>
                    <el-table-column prop="cveId" label="ID" width="160" show-overflow-tooltip />
                    <el-table-column prop="title" :label="t('vuln.scanner.colHeading')" min-width="260" show-overflow-tooltip />
                    <el-table-column :label="t('vuln.scanner.colSource')" width="80">
                      <template #default="{ row }">
                        <a v-if="row.referenceUrl" :href="row.referenceUrl" target="_blank" class="ref-link">{{ t('vuln.scanner.link') }}</a>
                      </template>
                    </el-table-column>
                  </el-table>
                </div>
              </div>
            </div>
          </el-tab-pane>
        </el-tabs>

        <ScanProgressPanel
          v-if="activeTab !== 'intel' && activeTab !== 'monitor'"
          :scanning="vulnStore.scanning"
          :result="vulnStore.currentResult"
          :progress-messages="vulnStore.scanProgress"
        />
      </div>

      <ScanResultPanel v-if="vulnStore.currentResult" :result="vulnStore.currentResult" />
    </template>

    <LoginDialog ref="loginDialog" @logged-in="() => { vulnStore.loadHistory() }" />
  </div>
</template>

<style scoped>
.page-view {
  max-width: 1200px;
  margin: 0 auto;
}

.page-header {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  align-items: flex-start;
  margin-bottom: 20px;
}

.page-header h2 {
  margin: 0;
  font-size: 22px;
  font-weight: 700;
}

.subtitle {
  margin: 6px 0 0;
  font-size: 13px;
  color: var(--muted);
}

.header-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.login-prompt {
  display: grid;
  place-items: center;
  min-height: 360px;
}

.login-card {
  text-align: center;
  padding: 48px;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  background: var(--panel);
  box-shadow: var(--shadow);
}

.login-card h3 {
  margin: 0 0 8px;
  font-size: 20px;
  color: var(--ink);
}

.login-card p {
  margin: 0 0 20px;
  color: var(--muted);
  font-size: 14px;
}

.scan-container {
  margin-bottom: 24px;
}

.model-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
  flex-wrap: wrap;
}

.model-warning {
  color: #d98612;
  font-size: 12px;
  line-height: 1.4;
}

.scan-tabs {
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: var(--radius);
  padding: 20px;
  box-shadow: var(--shadow);
}

.scan-input-row {
  display: flex;
  gap: 10px;
  align-items: center;
  margin-top: 8px;
}

.scan-input-row .el-input {
  flex: 1;
  min-width: 280px;
}

.input-prefix-icon {
  display: flex;
  align-items: center;
  color: var(--muted);
  margin-right: 4px;
}

.scan-hint {
  margin: 12px 0 0;
  font-size: 12px;
  color: var(--muted);
  line-height: 1.5;
}

.url-scan-options {
  margin-top: 12px;
  display: grid;
  gap: 10px;
}

.scan-mode-row,
.auth-grid {
  display: grid;
  grid-template-columns: minmax(420px, 1fr) 148px 148px;
  gap: 16px;
  align-items: center;
}

.scan-mode-row {
  grid-template-columns: minmax(0, 2fr) 130px 130px;
  gap: 16px;
  align-items: stretch;
}

.scan-mode-select,
.number-field {
  min-width: 0;
  padding: 12px 14px;
  border: 1px solid rgba(226, 232, 240, .9);
  border-radius: 8px;
  background: #fbfcff;
}

.number-field {
  display: grid;
  align-content: start;
}

.number-field :deep(.el-input-number) {
  width: 100%;
}

.number-field {
  padding: 10px 14px;
}

.option-label,
.number-field span {
  display: block;
  margin-bottom: 6px;
  color: var(--muted);
  font-size: 12px;
  line-height: 1;
}

.scan-mode-desc {
  margin: 8px 0 0;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.45;
}

.scan-mode-option {
  display: grid;
  gap: 3px;
  padding: 4px 0;
  line-height: 1.35;
}

.scan-mode-option strong {
  color: var(--ink);
  font-size: 13px;
}

.scan-mode-option span {
  color: var(--muted);
  font-size: 12px;
}

:global(.scan-mode-select-dropdown .el-select-dropdown__item) {
  height: auto;
  min-height: 58px;
  padding: 8px 12px;
  line-height: normal;
}

:global(.scan-mode-select-dropdown .el-select-dropdown__item.is-selected) {
  font-weight: 400;
}

.auth-grid {
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
}

.port-scan-row {
  display: grid;
  grid-template-columns: minmax(260px, 360px) minmax(0, 1fr);
  gap: 10px;
  align-items: center;
  padding: 10px 12px;
  border: 1px solid rgba(226, 232, 240, .9);
  border-radius: 8px;
  background: #fbfcff;
}

.scan-progress-panel {
  margin-top: 16px;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  background: #fbfcff;
  overflow: hidden;
}

.history-progress-panel {
  margin: 16px 20px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: #fbfcff;
  overflow: hidden;
}

.scan-progress-header {
  padding: 16px 18px;
  border-bottom: 1px solid var(--line);
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 12px;
}

.scan-progress-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 700;
  color: var(--ink);
}

.scan-progress-header p {
  margin: 5px 0 0;
  max-width: 760px;
  color: var(--muted);
  font-size: 12px;
  word-break: break-all;
}

.scan-progress-bar {
  padding: 14px 18px;
  border-bottom: 1px solid var(--line);
  background: var(--panel);
}

.scan-progress-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  padding: 14px;
}

.scan-step-card {
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--panel);
  overflow: hidden;
}

.scan-step-card.scan-step-running {
  border-color: rgba(217, 134, 18, .42);
  box-shadow: 0 0 0 3px rgba(217, 134, 18, .08);
}

.scan-step-card.scan-step-done {
  border-color: rgba(34, 197, 94, .24);
}

.scan-step-head {
  display: grid;
  grid-template-columns: 28px minmax(0, 1fr) auto;
  gap: 10px;
  align-items: flex-start;
  padding: 12px;
  border-bottom: 1px solid rgba(226, 232, 240, .78);
}

.scan-step-index {
  width: 26px;
  height: 26px;
  border-radius: 999px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: #eef2ff;
  color: #4338ca;
  font-size: 12px;
  font-weight: 700;
}

.scan-step-done .scan-step-index {
  background: rgba(34, 197, 94, .12);
  color: #15803d;
}

.scan-step-running .scan-step-index {
  background: rgba(217, 134, 18, .14);
  color: #b45309;
}

.scan-step-head h4 {
  margin: 0;
  font-size: 14px;
  line-height: 1.25;
  color: var(--ink);
}

.scan-step-head p {
  margin: 4px 0 0;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.45;
}

.scan-step-lines {
  min-height: 72px;
  max-height: 180px;
  overflow: auto;
  display: grid;
  align-content: start;
}

.scan-step-empty {
  padding: 14px 14px 16px 50px;
  color: #98a2b3;
  font-size: 12px;
}

.scan-step-line {
  display: grid;
  grid-template-columns: 12px minmax(0, 1fr);
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid rgba(226, 232, 240, .62);
  color: var(--ink);
  font-size: 12px;
  line-height: 1.55;
  word-break: break-word;
}

.scan-step-line:last-child {
  border-bottom: 0;
}

.scan-step-dot {
  width: 6px;
  height: 6px;
  border-radius: 999px;
  margin-top: 7px;
  background: #667085;
}

.intel-panel {
  padding-top: 4px;
}

.intel-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 14px;
  margin-bottom: 14px;
}

.intel-header h3 {
  margin: 0;
  font-size: 17px;
}

.intel-header p {
  margin: 6px 0 0;
  color: var(--muted);
  font-size: 13px;
}

.intel-filters {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
  align-items: center;
  margin-bottom: 12px;
}

.intel-pagination {
  display: flex;
  justify-content: center;
  margin-top: 14px;
}

/* Dep monitor styles */
.dep-monitor-panel { padding-top: 4px; }

.dep-upload-area { margin-bottom: 16px; }
.dep-upload-drop {
  border: 2px dashed var(--line);
  border-radius: var(--radius);
  padding: 40px 20px;
  text-align: center;
  cursor: pointer;
  transition: border-color .2s;
}
.dep-upload-drop:hover { border-color: var(--blue); }
.upload-icon { font-size: 32px; color: var(--muted); margin: 0 0 8px; }
.upload-hint { font-size: 12px; color: var(--muted); margin-top: 8px; }
.dep-upload-row { display: flex; justify-content: center; margin-top: 10px; }

.dep-files-preview { margin-bottom: 16px; }
.dep-files-preview h4 { margin: 0 0 8px; font-size: 14px; }
.dep-files-preview ul { margin: 0 0 12px; padding-left: 20px; font-size: 13px; color: var(--muted); }
.dep-upload-form { display: flex; gap: 10px; align-items: center; }

.dep-snapshots-header {
  display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px;
}
.dep-snapshots-header h3 { margin: 0; font-size: 16px; }

.dep-snapshot-cards { display: grid; gap: 10px; }
.dep-snapshot-card {
  display: flex; justify-content: space-between; align-items: center;
  padding: 14px 16px;
  border: 1px solid var(--line); border-radius: 8px; background: var(--panel);
}
.dep-snapshot-name { font-size: 15px; font-weight: 600; color: var(--ink); }
.dep-snapshot-meta { display: flex; gap: 12px; font-size: 12px; color: var(--muted); margin-top: 4px; }
.dep-snapshot-actions { display: flex; gap: 6px; }

.dep-detail { margin-top: 16px; }
.dep-detail h4 { margin: 0 0 8px; font-size: 15px; }
.dep-findings { margin-top: 14px; }
.dep-findings h5 { margin: 0 0 8px; font-size: 14px; color: var(--muted); }
.ref-link { color: var(--blue); text-decoration: none; }
.ref-link:hover { text-decoration: underline; }

.offline-banner {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  margin-bottom: 16px;
  background: rgba(217, 134, 18, .08);
  border: 1px solid rgba(217, 134, 18, .28);
  border-radius: 8px;
  color: #d98612;
  font-size: 13px;
}

.offline-icon {
  display: flex;
  align-items: center;
  flex-shrink: 0;
}

.result-section {
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: var(--radius);
  box-shadow: var(--shadow);
  overflow: hidden;
}

.result-header {
  padding: 20px;
  border-bottom: 1px solid var(--line);
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 16px;
}

.result-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}

.result-target {
  margin: 6px 0 0;
  font-size: 13px;
  color: var(--muted);
  display: flex;
  align-items: center;
  gap: 8px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 500px;
}

.result-summary {
  display: flex;
  align-items: center;
}

.finding-counts {
  display: flex;
  gap: 10px;
}

.finding-badge {
  text-align: center;
  padding: 6px 14px;
  border-radius: 8px;
  min-width: 58px;
}

.badge-danger {
  background: rgba(214, 79, 79, .12);
  border: 1px solid rgba(214, 79, 79, .28);
}

.badge-warning {
  background: rgba(217, 134, 18, .1);
  border: 1px solid rgba(217, 134, 18, .24);
}

.badge- {
  background: rgba(102, 112, 133, .1);
  border: 1px solid rgba(102, 112, 133, .2);
}

.badge-info {
  background: rgba(102, 112, 133, .06);
  border: 1px solid rgba(102, 112, 133, .14);
}

.badge-count {
  display: block;
  font-size: 20px;
  font-weight: 700;
  line-height: 1;
}

.badge-danger .badge-count { color: #d64f4f; }
.badge-warning .badge-count { color: #d98612; }
.badge- .badge-count { color: #667085; }
.badge-info .badge-count { color: #909399; }

.badge-label {
  display: block;
  font-size: 11px;
  margin-top: 4px;
  color: var(--muted);
}

.findings-table-wrapper {
  overflow-x: auto;
}

.findings-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.findings-table th {
  text-align: left;
  padding: 13px 16px;
  border-bottom: 1px solid var(--line);
  color: var(--muted);
  font-size: 12px;
  background: #fafbfe;
  white-space: nowrap;
}

.confidence-badge {
  display: inline-block;
  padding: 2px 8px;
  border-radius: 10px;
  font-size: 12px;
  font-weight: 600;
}
.conf-high { background: #dcfce7; color: #16a34a; }
.conf-mid  { background: #fef3c7; color: #d97706; }
.conf-low  { background: #fee2e2; color: #dc2626; }
.confidence-na { color: var(--muted); font-size: 12px; }

.findings-table td {
  padding: 13px 16px;
  border-bottom: 1px solid var(--line);
  vertical-align: top;
}

.findings-table tbody tr:hover {
  background: #f7faff;
}

.finding-type {
  font-weight: 500;
  color: var(--ink);
}

.finding-location {
  font-size: 12px;
  color: var(--blue);
  background: rgba(13, 148, 136, .06);
  padding: 2px 6px;
  border-radius: 4px;
  word-break: break-all;
}

.finding-desc {
  color: var(--ink);
  line-height: 1.5;
}

.finding-suggestion {
  color: var(--muted);
  font-size: 12px;
  line-height: 1.5;
}

.empty-state {
  padding: 56px 20px;
  text-align: center;
  color: var(--muted);
}

.empty-state p {
  margin: 0;
  font-size: 16px;
}

.safe-state {
  padding: 48px 20px;
}

.safe-icon {
  margin-bottom: 16px;
  display: flex;
  justify-content: center;
}

.safe-text {
  font-size: 18px;
  color: #22c55e;
  font-weight: 600;
  margin: 0 0 8px;
}

.safe-hint {
  font-size: 13px;
  color: var(--muted);
  margin: 0;
}

.history-section {
  margin-top: 4px;
}

.section-title {
  margin: 0 0 16px;
  font-size: 18px;
  font-weight: 600;
}

.history-list {
  display: grid;
  gap: 12px;
}

.history-card {
  padding: 16px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--panel);
  box-shadow: var(--shadow);
  transition: all .2s;
  display: flex;
  justify-content: space-between;
  align-items: center;
  cursor: pointer;
}

.history-card:hover {
  box-shadow: var(--shadow-hover);
  border-color: rgba(13, 148, 136, .18);
}

.history-arrow {
  font-size: 16px;
  color: var(--muted);
  flex-shrink: 0;
  margin-left: 12px;
}

.history-detail-view { margin-top: 4px; }
.detail-back-row { margin-bottom: 16px; }

.history-pagination {
  display: flex;
  justify-content: center;
  margin-top: 20px;
}

.history-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
}

.history-target {
  font-size: 14px;
  font-weight: 500;
  color: var(--ink);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.history-meta {
  display: flex;
  gap: 12px;
  font-size: 12px;
  color: var(--muted);
}

.count-danger { color: #d64f4f; font-weight: 500; }
.count-warning { color: #d98612; font-weight: 500; }
.count-default { color: #667085; font-weight: 500; }

@media (max-width: 760px) {
  .page-header {
    flex-direction: column;
  }
  .scan-input-row {
    flex-direction: column;
    align-items: stretch;
  }
  .scan-mode-row,
  .auth-grid,
  .port-scan-row {
    grid-template-columns: 1fr;
  }
  .scan-progress-grid {
    grid-template-columns: 1fr;
  }
  .scan-step-head {
    grid-template-columns: 28px minmax(0, 1fr);
  }
  .scan-step-head .el-tag {
    grid-column: 2;
    justify-self: start;
  }
  .result-header {
    flex-direction: column;
  }
  .finding-counts {
    flex-wrap: wrap;
  }
  .findings-table {
    font-size: 12px;
  }
}
</style>
