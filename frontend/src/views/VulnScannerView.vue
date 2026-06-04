<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useTeamStore } from '../stores/useTeamStore'
import { useVulnStore } from '../stores/useVulnStore'
import { getErrorMessage } from '../utils/error'
import { evaluateShareResource, fetchLlmConfig, testLlmConnection } from '../api/admin'
import { fetchAvailableModels, type TeamModelConfig } from '../api/team'
import LoginDialog from '../components/team/LoginDialog.vue'
import type { VulnScanJob, VulnFinding, AgentCredential } from '../api/vuln'

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

const scanSteps = [
  '校验目标',
  '加载登录态',
  '策略配置',
  '爬取入口',
  'SQL/XSS/SSRF等注入检测',
  '敏感路径/端口检测',
  'TLS/证书检查',
  '系统漏洞检测',
  'AI 多角色智能发现',
  'AI 多角色复核',
  '保存结果',
]

const scanProfileOptions = [
  {
    value: 'quick',
    label: '快速',
    title: '快速扫描',
    description: '少量页面、常见端口、基础安全头和敏感路径检查，适合先判断目标是否有明显问题。',
  },
  {
    value: 'standard',
    label: '标准',
    title: '标准扫描',
    description: '默认模式，爬取更多入口，执行 SQL/XSS/CSRF/SSRF/NoSQL/SSTI/LFI、系统漏洞检测（HTTP方法/CRLF/Host头/默认凭据/源码泄露），6角色 AI 并行复核。',
  },
  {
    value: 'deep',
    label: '深度',
    title: '深度扫描',
    description: '更多页面、更全端口集合、更完整敏感路径和全部 payload 变种 + 6角色 AI 并行发现与复核，适合正式排查但耗时更长。',
  },
] as const

const selectedScanProfile = computed(() =>
  scanProfileOptions.find(item => item.value === scanProfile.value) || scanProfileOptions[1],
)

const scanStepDescriptions: Record<string, string> = {
  校验目标: '校验 URL、目录路径、模型配置和扫描参数是否可用。',
  加载登录态: '装载 Cookie、Authorization 和自定义请求头，用于访问需要登录的页面。',
  策略配置: '按快速、标准、深度模式确定爬取深度、页面数量、payload 和敏感路径范围。',
  爬取入口: '请求目标页面并收集同源链接、表单、参数和可测试入口。',
  'SQL/XSS/SSRF等注入检测': '对 URL 参数和表单输入点执行 SQL 注入（含盲注/堆叠）、XSS、CSRF、SSRF、NoSQL、SSTI、LFI 全部注入类型检测。',
  '敏感路径/端口检测': '探测常见敏感路径（Swagger、Actuator、配置文件等），扫描目标 IP 开放端口并识别服务用途。',
  'TLS/证书检查': '检查 HTTPS 证书有效性、TLS 协议版本、自签名证书等传输层安全问题。',
  '系统漏洞检测': 'HTTP 方法探测（TRACE/PUT/DELETE）、CRLF 注入、Host 头注入、默认凭据爆破（18组常见凭据）、源码泄露路径扫描。',
  'AI 多角色智能发现': '注入专家/认证审计/信息泄露/HTTP配置/客户端安全/基础设施 6角色并行分析原始响应，发现规则扫描遗漏的漏洞。',
  'AI 多角色复核': '6个安全角色并行复核候选漏洞，各角色验证领域内漏洞真实性，去重汇总结果。',
  保存结果: '保存扫描结果、统计分级数量，并写入扫描历史。',
}

const scanStepKeywords: Record<string, string[]> = {
  校验目标: ['校验目标', '准备扫描目标', '规范化 URL', '目标'],
  加载登录态: ['加载登录态', 'Cookie', 'Authorization', '自定义请求头', '登录态'],
  策略配置: ['扫描策略', '快速扫描', '标准扫描', '深度扫描', 'maxDepth', 'maxPages'],
  爬取入口: ['爬取页面', '爬取入口', '读取源码文件', '读取文件'],
  'SQL/XSS/SSRF等注入检测': ['SQL 注入', 'XSS', 'CSRF', 'SSRF', 'NoSQL', 'SSTI', 'LFI', 'SQL错误', '布尔盲注', '反射型', '模板注入', '文件包含', '目录穿越'],
  '敏感路径/端口检测': ['敏感路径', 'heapdump', 'swagger', 'api-docs', 'actuator', '端口扫描', '开放端口', 'TCP'],
  'TLS/证书检查': ['TLS 检查', 'HTTPS', '证书', 'SSL'],
  '系统漏洞检测': ['系统漏洞检测', 'HTTP 方法', 'CRLF', 'Host 头', '默认凭据', '源码泄露', '方法探测'],
  'AI 多角色智能发现': ['AI 智能发现', '6角色并行分析', '大模型额外发现'],
  'AI 多角色复核': ['AI 复核', '6角色并行复核', '模型复核'],
  保存结果: ['保存结果', '保存扫描结果', '扫描完成'],
}

const filteredModels = computed(() => {
  if (modelType.value === 'department') {
    return models.value
  }
  return localResourceModel.value ? [localResourceModel.value] : []
})

const findingCounts = computed(() => {
  const r = vulnStore.currentResult
  if (!r) return null
  return [
    { label: '严重', count: r.criticalCount, type: 'danger' },
    { label: '高危', count: r.highCount, type: 'warning' },
    { label: '中危', count: r.mediumCount, type: '' },
    { label: '低危', count: r.lowCount, type: 'info' },
  ]
})

const scanProgressGroups = computed(() => buildScanProgressGroups(vulnStore.currentResult))

const historyScanProgressGroups = computed(() => buildScanProgressGroups(vulnStore.selectedHistoryJob))

const scanProgressPercent = computed(() => {
  if (vulnStore.currentResult) return 100
  if (vulnStore.scanning) {
    const streamCount = vulnStore.scanProgress.length
    if (streamCount > 0) {
      return Math.min(99, Math.max(5, Math.round((streamCount / 25) * 100)))
    }
    const done = scanProgressGroups.value.filter(group => group.status === 'done').length
    const running = scanProgressGroups.value.some(group => group.status === 'running') ? 0.5 : 0
    return Math.min(99, Math.max(5, Math.round(((done + running) / scanSteps.length) * 100)))
  }
  return 0
})

const currentScanTarget = computed(() => {
  if (vulnStore.currentResult?.target) return vulnStore.currentResult.target
  return activeTab.value === 'url' ? urlInput.value.trim() : dirInput.value.trim()
})

function severityType(severity: string): string {
  const map: Record<string, string> = {
    CRITICAL: 'danger',
    HIGH: 'warning',
    MEDIUM: '',
    LOW: 'info',
  }
  return map[severity] || 'info'
}

function severityLabel(severity: string): string {
  const map: Record<string, string> = {
    CRITICAL: '严重',
    HIGH: '高危',
    MEDIUM: '中危',
    LOW: '低危',
  }
  return map[severity] || severity
}

function scanTypeLabel(type: string): string {
  return type === 'url' ? '网址扫描' : '代码扫描'
}

async function handleUrlScan() {
  if (!urlInput.value.trim()) {
    ElMessage.warning('请输入网址')
    return
  }
  if (!await validateEvaluationModel()) return
  scanStep.value = 1
  scanStep.value = 2

  let result: VulnScanJob | null
  if (useAgent.value) {
    const { scanUrlWithAgent } = await import('../api/vuln')
    result = await scanUrlWithAgent(
      urlInput.value.trim(),
      { modelType: modelType.value, modelId: modelId.value },
      agentCredentials.value,
    )
    vulnStore.currentResult = result
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
      ElMessage.success(`扫描完成，发现 ${result.totalFindings} 个漏洞`)
    } catch (e) {
      scanStep.value = scanSteps.length
      ElMessage.error(getErrorMessage(e))
    }
  } else if (vulnStore.error) {
    ElMessage.error(vulnStore.error)
  }
}

async function handleCodeScan() {
  if (!dirInput.value.trim()) {
    ElMessage.warning('请选择或输入目录路径')
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
      ElMessage.success(`扫描完成，发现 ${result.totalFindings} 个漏洞`)
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
    ElMessage.error(`部门模型加载失败：${modelLoadError.value}`)
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
      name: '资源与配置的模型配置',
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
      `# 漏洞扫描结果`,
      `目标: ${result.target}`,
      `类型: ${result.scanType}`,
      '',
      ...result.findings.map(item => [
        `## ${item.severity} ${item.type}`,
        `位置: ${item.location}`,
        `描述: ${item.description}`,
        `建议: ${item.suggestion}`,
      ].join('\n')),
    ].join('\n\n'),
  }
  const res = await evaluateShareResource(payload)
  if (!res.success) {
    throw new Error(res.error || '资源与配置中的模型复核失败')
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
    '10. AI 多角色复核：调用资源与配置的本地模型复核漏洞扫描结果',
    `AI 复核：综合分 ${data.score ?? '-'}，安全分 ${data.securityScore ?? '-'}，性能分 ${data.performanceScore ?? '-'}`,
    data.summary ? `AI 复核：${data.summary}` : '',
    ...(data.risks || []).map(item => `AI 风险：${item}`),
    ...(data.suggestions || []).map(item => `AI 建议：${item}`),
    ...(data.requiredChanges || []).map(item => `AI 必改：${item}`),
  ].filter(Boolean)
  result.progressText = [result.progressText, ...lines].filter(Boolean).join('\n')
}

async function validateEvaluationModel(): Promise<boolean> {
  if (modelType.value === 'department') {
    const model = filteredModels.value.find(item => item.id === modelId.value)
    if (!model) {
      ElMessage.error('请选择部门模型')
      return false
    }
    if (!model.baseUrl || !model.model || !model.apiKeyHash) {
      ElMessage.error('部门模型配置有问题，请检查部门模型的 Base URL、模型名称和 API-Key')
      return false
    }
    return true
  }

  await loadLocalResourceModel()
  if (!localResourceModel.value || !localLlmConfig.value) {
    ElMessage.error('资源与配置中的模型配置未启用或不完整，请检查模型配置')
    return false
  }
  if (!localResourceModel.value.apiKeyHash) {
    ElMessage.error('资源与配置中的模型缺少 API-Key，请检查模型配置')
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
    if (!res.success) throw new Error(res.error || '模型连接测试失败')
    return true
  } catch (e) {
    ElMessage.error(`资源与配置中的模型配置有问题：${getErrorMessage(e)}`)
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

function buildScanProgressGroups(result?: VulnScanJob | null) {
  const lines = progressLines(result?.progressText)
  const streamLines = vulnStore.scanning ? vulnStore.scanProgress : []
  const allLines = streamLines.length > 0 ? streamLines : lines
  const grouped = scanSteps.map((step, index) => {
    const stepLines = allLines.filter(line => lineBelongsToStep(line, step))
    const doneByResult = !!result
      const active = !result && vulnStore.scanning && index === Math.max(scanStep.value - 1, 0)
    return {
      step,
      index,
      description: scanStepDescriptions[step],
      lines: stepLines,
      status: doneByResult ? 'done' : active ? 'running' : index < scanStep.value ? 'done' : 'pending',
    }
  })

  const matched = new Set(grouped.flatMap(group => group.lines))
  const unmatched = allLines.filter(line => !matched.has(line))
  if (unmatched.length) {
    grouped.push({
      step: '其他执行记录',
      index: grouped.length,
      description: '后端返回但无法归类到固定阶段的执行记录。',
      lines: unmatched,
      status: 'done',
    })
  }
  return grouped
}

function lineBelongsToStep(line: string, step: string): boolean {
  const normalized = line.replace(/^\d+[.、]\s*/, '')
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
    ElMessage.warning('请输入项目名称并选择依赖文件')
    return
  }
  const result = await vulnStore.uploadMonitor(depMonitorName.value.trim(), depUploadFiles.value)
  if (result) {
    ElMessage.success(`已解析 ${result.deps.length} 个依赖包，已查询关联漏洞`)
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
        <h2>漏洞查询</h2>
        <p class="subtitle">输入网址进行安全漏洞扫描，或选择本地目录扫描代码中的安全漏洞。</p>
      </div>
      <div class="header-actions">
        <el-button @click="toggleHistory">
          {{ showHistory ? '返回扫描' : '扫描历史' }}
        </el-button>
      </div>
    </div>

    <div v-if="vulnStore.offline" class="offline-banner">
      <span class="offline-icon">
        <svg width="18" height="18" viewBox="0 0 18 18" fill="none"><path d="M9 1.5C4.86 1.5 1.5 4.86 1.5 9s3.36 7.5 7.5 7.5 7.5-3.36 7.5-7.5S13.14 1.5 9 1.5zM9 6v4M9 12h.01" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>
      </span>
      离线模式 — 无法连接到服务器，请确认服务已启动。接口恢复后页面将自动重试。
      <el-button size="small" @click="vulnStore.loadHistory()">重试</el-button>
    </div>

    <template v-if="!store.isAuthenticated">
      <div class="login-prompt">
        <div class="login-card">
          <h3>登录团队版</h3>
          <p>登录后可以使用漏洞扫描功能，对网址和代码进行安全分析。</p>
          <el-button type="primary" size="large" @click="loginDialog?.open()">登录团队版</el-button>
        </div>
      </div>
    </template>

    <template v-else-if="showHistory">
      <section class="history-section">
        <h3 class="section-title">扫描历史</h3>
        <div v-if="vulnStore.history.length === 0 && !vulnStore.offline" class="empty-state">
          <p>暂无扫描记录</p>
        </div>

        <!-- History detail view -->
        <div v-if="vulnStore.selectedHistoryJob" class="history-detail-view">
          <div class="detail-back-row">
            <el-button size="small" @click="vulnStore.clearHistoryDetail()">&larr; 返回历史列表</el-button>
          </div>
          <div class="result-section">
            <div class="result-header">
              <div>
                <h3>扫描详情</h3>
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
                    <span class="badge-label">严重</span>
                  </div>
                  <div class="finding-badge badge-warning">
                    <span class="badge-count">{{ vulnStore.selectedHistoryJob.highCount }}</span>
                    <span class="badge-label">高危</span>
                  </div>
                  <div class="finding-badge">
                    <span class="badge-count">{{ vulnStore.selectedHistoryJob.mediumCount }}</span>
                    <span class="badge-label">中危</span>
                  </div>
                  <div class="finding-badge badge-info">
                    <span class="badge-count">{{ vulnStore.selectedHistoryJob.lowCount }}</span>
                    <span class="badge-label">低危</span>
                  </div>
                </div>
              </div>
            </div>
            <div class="history-progress-panel">
              <div class="scan-progress-header">
                <div>
                  <h3>执行记录</h3>
                  <p>{{ vulnStore.selectedHistoryJob.target }}</p>
                </div>
                <el-tag type="success" effect="light">已入库</el-tag>
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
                      <h4>{{ group.step }}</h4>
                      <p>{{ group.description }}</p>
                    </div>
                    <el-tag size="small" type="success">完成</el-tag>
                  </div>
                  <div class="scan-step-lines">
                    <div v-if="group.lines.length === 0" class="scan-step-empty">该步骤未返回明细</div>
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
              <p class="safe-text">未发现安全漏洞</p>
            </div>
            <div v-else class="findings-table-wrapper">
              <table class="findings-table">
                <thead>
                  <tr>
                    <th style="width:72px">严重程度</th>
                    <th style="width:110px">类型</th>
                    <th style="width:180px">位置</th>
                    <th>描述</th>
                    <th>修复建议</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="(f, idx) in vulnStore.selectedHistoryJob.findings" :key="idx">
                    <td><el-tag :type="severityType(f.severity)" size="small" effect="dark">{{ severityLabel(f.severity) }}</el-tag></td>
                    <td><span class="finding-type">{{ f.type }}</span></td>
                    <td><code class="finding-location">{{ f.location }}</code></td>
                    <td class="finding-desc">{{ f.description }}</td>
                    <td class="finding-suggestion">{{ f.suggestion }}</td>
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
                  <span>共 {{ job.totalFindings }} 个漏洞</span>
                  <span v-if="job.criticalCount" class="count-danger">严重 {{ job.criticalCount }}</span>
                  <span v-if="job.highCount" class="count-warning">高危 {{ job.highCount }}</span>
                  <span v-if="job.mediumCount" class="count-default">中危 {{ job.mediumCount }}</span>
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
              { label: '部门模型', value: 'department' },
              { label: '资源与配置的模型配置', value: 'personal' },
            ]"
            @change="modelId = filteredModels[0]?.id"
          />
          <el-select v-model="modelId" placeholder="选择评估模型" style="width: 280px">
            <el-option v-for="model in filteredModels" :key="model.id" :label="`${model.name} / ${model.model}`" :value="model.id" />
          </el-select>
          <el-button size="small" :loading="modelLoading" @click="loadModels">刷新模型</el-button>
          <span v-if="modelType === 'department' && !modelLoading && filteredModels.length === 0" class="model-warning">
            {{ modelLoadError ? `部门模型加载失败：${modelLoadError}` : '未查询到可用部门模型' }}
          </span>
        </div>

        <el-tabs v-model="activeTab" class="scan-tabs">
          <el-tab-pane label="网址扫描" name="url">
            <div class="scan-input-row">
              <el-input
                v-model="urlInput"
                placeholder="输入网址，例如 https://example.com"
                size="large"
                clearable
                @keyup.enter="handleUrlScan"
              >
                <template #prefix>
                  <span class="input-prefix-icon">
                    <svg width="16" height="16" viewBox="0 0 16 16" fill="none"><circle cx="8" cy="8" r="6" stroke="currentColor" stroke-width="1.2"/><path d="M2 8h12M8 2c1.66 2 1.66 10 0 12M8 2c-1.66 2-1.66 10 0 12" stroke="currentColor" stroke-width="1.2"/></svg>
                  </span>
                </template>
              </el-input>
              <el-button
                type="primary"
                size="large"
                :loading="vulnStore.scanning"
                @click="handleUrlScan"
              >
                {{ vulnStore.scanning ? '扫描中...' : '开始扫描' }}
              </el-button>
            </div>
            <p class="scan-hint">系统会爬取同源页面并检测 SQL 注入、XSS、CSRF、信息泄露、安全响应头缺失等常见漏洞。</p>
            <el-checkbox v-model="useAgent" class="agent-toggle" style="margin-top:12px">
              启用 AI 自主渗透测试 Agent（需配置LLM模型，扫描耗时 30-60 分钟）
            </el-checkbox>
            <div v-if="useAgent" class="credential-section" style="margin-top:12px">
              <div class="cred-header" style="display:flex;justify-content:space-between;align-items:center;margin-bottom:8px">
                <span style="font-size:14px;font-weight:500">多角色凭据（Agent 多角色越权测试用）</span>
                <el-button size="small" @click="addCredential">+ 添加凭据</el-button>
              </div>
              <div
                v-for="(cred, idx) in agentCredentials"
                :key="idx"
                class="cred-row"
                style="display:flex;gap:8px;margin-bottom:6px;align-items:center"
              >
                <el-input v-model="cred.username" placeholder="用户名" size="small" style="width:100px" />
                <el-input v-model="cred.role" placeholder="角色(admin/user)" size="small" style="width:120px" />
                <el-input v-model="cred.cookie" placeholder="Cookie" size="small" style="width:160px" />
                <el-input v-model="cred.authorization" placeholder="Authorization" size="small" style="width:160px" />
                <el-button @click="removeCredential(idx)" size="small" type="danger" circle>×</el-button>
              </div>
            </div>
            <div class="url-scan-options">
              <div class="scan-mode-row">
                <div class="scan-mode-select">
                  <span class="option-label">扫描模式</span>
                  <el-select v-model="scanProfile" style="width: 100%" popper-class="scan-mode-select-dropdown">
                    <el-option
                      v-for="option in scanProfileOptions"
                      :key="option.value"
                      :label="option.title"
                      :value="option.value"
                    >
                      <div class="scan-mode-option">
                        <strong>{{ option.title }}</strong>
                        <span>{{ option.description }}</span>
                      </div>
                    </el-option>
                  </el-select>
                  <p class="scan-mode-desc">{{ selectedScanProfile.description }}</p>
                </div>
                <label class="number-field">
                  <span>最大深度</span>
                  <el-input-number v-model="maxDepth" :min="0" :max="4" size="small" controls-position="right" />
                </label>
                <label class="number-field">
                  <span>最多页面</span>
                  <el-input-number v-model="maxPages" :min="1" :max="80" size="small" controls-position="right" />
                </label>
              </div>
              <div class="auth-grid">
                <el-input v-model="urlCookie" type="textarea" :rows="2" placeholder="登录态 Cookie，可选，例如 JSESSIONID=...; token=..." />
                <el-input v-model="urlAuthorization" placeholder="Authorization，可选，例如 Bearer eyJ..." clearable />
              </div>
              <div class="auth-grid">
                <el-input v-model="urlHeaders" type="textarea" :rows="2" placeholder="自定义请求头，每行一个：X-Token: xxx" />
                <el-input v-model="urlCustomPaths" type="textarea" :rows="2" placeholder="自定义敏感路径，每行一个：/actuator/heapdump" />
              </div>
              <div class="port-scan-row">
                <el-checkbox v-model="portScanEnabled">如果目标是公网 IP，同时扫描服务器开放端口</el-checkbox>
                <el-input
                  v-model="portSpec"
                  :disabled="!portScanEnabled"
                  placeholder="端口范围，可选：22,80,443,3306 或 1-1024，最多 80 个"
                  clearable
                />
              </div>
            </div>
            <p class="scan-hint">系统会携带登录态爬取同源页面，执行 SQL/XSS/SSRF/NoSQL/SSTI/LFI 注入检测、HTTP 方法/CRLF/Host头/默认凭据/源码泄露等系统漏洞检测、TLS 证书检查、IP 端口扫描，并调用 AI 大模型 6 角色并行发现与复核。</p>
          </el-tab-pane>

          <el-tab-pane label="代码扫描" name="code">
            <div class="scan-input-row">
              <el-input
                v-model="dirInput"
                placeholder="输入或选择本地目录路径"
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
                {{ vulnStore.scanning ? '扫描中...' : '开始扫描' }}
              </el-button>
            </div>
            <p class="scan-hint">系统会扫描目录下的源代码文件，检测硬编码密钥、SQL 注入、XSS、命令注入、路径遍历、不安全加密、XXE、反序列化、JWT 安全、原型污染、LDAP/XPath 注入等漏洞。</p>
          </el-tab-pane>

          <el-tab-pane label="漏洞列表" name="intel">
            <div class="intel-panel" v-loading="vulnStore.intelLoading">
              <div class="intel-header">
                <div>
                  <h3>漏洞列表</h3>
                  <p>公开漏洞情报由后台每半小时自动更新，这里仅提供查询。</p>
                </div>
              </div>
              <div class="intel-filters">
                <el-input v-model="intelQuery" placeholder="CVE / 厂商 / 产品" clearable style="width: 220px" />
                <el-select v-model="intelType" placeholder="漏洞类型" clearable filterable allow-create style="width: 170px">
                  <el-option label="SQL注入" value="SQL注入" />
                  <el-option label="XSS" value="XSS" />
                  <el-option label="远程代码执行" value="远程代码执行" />
                  <el-option label="权限提升" value="权限提升" />
                  <el-option label="系统漏洞" value="系统漏洞" />
                  <el-option label="供应链投毒" value="供应链投毒" />
                </el-select>
                <el-select v-model="intelSeverity" placeholder="等级" clearable filterable allow-create style="width: 130px">
                  <el-option label="严重" value="CRITICAL" />
                  <el-option label="高危" value="HIGH" />
                  <el-option label="中危" value="MEDIUM" />
                  <el-option label="低危" value="LOW" />
                </el-select>
                <el-date-picker v-model="intelStartDate" format="YYYY-MM-DD" value-format="YYYY-MM-DD" type="date" placeholder="开始时间" style="width: 150px" />
                <el-date-picker v-model="intelEndDate" format="YYYY-MM-DD" value-format="YYYY-MM-DD" type="date" placeholder="结束时间" style="width: 150px" />
                <el-button type="primary" @click="queryIntel">查询</el-button>
              </div>
              <el-table :data="pagedIntel" size="small" class="intel-table">
                <el-table-column prop="cveId" label="CVE" width="150" />
                <el-table-column prop="vulnType" label="漏洞类型" width="130" />
                <el-table-column label="投毒" width="80">
                  <template #default="{ row }">
                    <el-tag v-if="row.isPoisoning" type="danger" size="small" effect="dark">投毒</el-tag>
                  </template>
                </el-table-column>
                <el-table-column prop="severity" label="等级" width="90">
                  <template #default="{ row }">{{ severityLabel(row.severity) }}</template>
                </el-table-column>
                <el-table-column prop="ecosystem" label="生态" width="90" show-overflow-tooltip />
                <el-table-column prop="vendorProject" label="厂商" width="150" show-overflow-tooltip />
                <el-table-column prop="product" label="产品" width="150" show-overflow-tooltip />
                <el-table-column prop="title" label="漏洞名称" min-width="260" show-overflow-tooltip />
                <el-table-column prop="publishedAt" label="发布时间" width="160">
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

          <el-tab-pane label="依赖监控" name="monitor">
            <div class="dep-monitor-panel" v-loading="vulnStore.depLoading">
              <!-- Upload area -->
              <div v-if="!vulnStore.snapshots.length && depUploadFiles.length === 0" class="dep-upload-area">
                <div class="dep-upload-drop"
                  @dragover.prevent
                  @drop.prevent="handleDepDrop">
                  <p class="upload-icon">+</p>
                  <p>拖拽依赖清单文件到此处，或点击下方选择</p>
                  <p class="upload-hint">支持 package.json / pom.xml / go.mod / Cargo.toml / composer.json / requirements.txt / *.csproj / build.gradle</p>
                </div>
                <div class="dep-upload-row">
                  <input type="file" multiple @change="handleDepFilesSelected" accept=".json,.xml,.toml,.txt,.gradle,.kts,.mod,.csproj" style="display:none" ref="depFileInput" />
                  <el-button @click="depFileInput?.click()">选择文件</el-button>
                </div>
              </div>

              <!-- Selected files preview -->
              <div v-if="depUploadFiles.length > 0" class="dep-files-preview">
                <h4>已选择 {{ depUploadFiles.length }} 个文件</h4>
                <ul>
                  <li v-for="(f, i) in depUploadFiles" :key="i">{{ f.name }} ({{ f.content.length }} 字符)</li>
                </ul>
                <div class="dep-upload-form">
                  <el-input v-model="depMonitorName" placeholder="项目名称" style="width: 260px" size="small" />
                  <el-button type="primary" size="small" @click="handleDepUpload" :loading="vulnStore.depLoading">上传并分析</el-button>
                  <el-button size="small" @click="depUploadFiles = []">取消</el-button>
                </div>
              </div>

              <!-- Snapshots list -->
              <div v-if="vulnStore.snapshots.length > 0" class="dep-snapshots">
                <div class="dep-snapshots-header">
                  <h3>监控快照 ({{ vulnStore.snapshots.length }})</h3>
                  <el-button size="small" @click="depUploadFiles = []; vulnStore.clearDepDetail()">+ 新建</el-button>
                </div>
                <div class="dep-snapshot-cards">
                  <div v-for="snap in vulnStore.snapshots" :key="snap.id" class="dep-snapshot-card">
                    <div class="dep-snapshot-main">
                      <div class="dep-snapshot-name">{{ snap.name }}</div>
                      <div class="dep-snapshot-meta">
                        <span :class="{ 'count-danger': snap.poisoningCount > 0 }">投毒 {{ snap.poisoningCount }}</span>
                        <span>漏洞 {{ snap.vulnCount }}</span>
                        <span>{{ formatTime(snap.lastCheckedAt || snap.createdAt) }}</span>
                      </div>
                    </div>
                    <div class="dep-snapshot-actions">
                      <el-button size="small" @click="vulnStore.loadDepDeps(snap.id, snap.name).then(() => expandedDepId = snap.id)">查看依赖</el-button>
                      <el-button size="small" type="warning" :loading="vulnStore.depLoading" @click="vulnStore.refreshSnapshot(snap.id)">刷新</el-button>
                      <el-button size="small" type="danger" @click="vulnStore.removeSnapshot(snap.id)">删除</el-button>
                    </div>
                  </div>
                </div>
              </div>

              <!-- Expanded dependency details -->
              <div v-if="expandedDepId && vulnStore.currentSnapshotDeps.length > 0" class="dep-detail">
                <h4>依赖清单</h4>
                <el-table :data="vulnStore.currentSnapshotDeps" size="small">
                  <el-table-column prop="ecosystem" label="生态" width="90" />
                  <el-table-column prop="packageName" label="包名" min-width="220" show-overflow-tooltip />
                  <el-table-column prop="version" label="版本" width="140" show-overflow-tooltip />
                  <el-table-column label="投毒" width="70">
                    <template #default="{ row }">
                      <el-tag v-if="row.poisoningCount > 0" type="danger" size="small" effect="dark">{{ row.poisoningCount }}</el-tag>
                      <span v-else>-</span>
                    </template>
                  </el-table-column>
                  <el-table-column label="漏洞" width="70">
                    <template #default="{ row }">{{ row.vulnCount > 0 ? row.vulnCount : '-' }}</template>
                  </el-table-column>
                  <el-table-column label="操作" width="80">
                    <template #default="{ row }">
                      <el-button size="small" @click="vulnStore.loadDepFindings(row.id)">详情</el-button>
                    </template>
                  </el-table-column>
                </el-table>

                <!-- Findings sub-table -->
                <div v-if="vulnStore.depFindings.length > 0" class="dep-findings">
                  <h5>关联漏洞/投毒 ({{ vulnStore.depFindings.length }})</h5>
                  <el-table :data="vulnStore.depFindings" size="small">
                    <el-table-column label="投毒" width="70">
                      <template #default="{ row }">
                        <el-tag v-if="row.isPoisoning" type="danger" size="small" effect="dark">投毒</el-tag>
                      </template>
                    </el-table-column>
                    <el-table-column prop="severity" label="等级" width="80">
                      <template #default="{ row }">{{ severityLabel(row.severity) }}</template>
                    </el-table-column>
                    <el-table-column prop="cveId" label="ID" width="160" show-overflow-tooltip />
                    <el-table-column prop="title" label="标题" min-width="260" show-overflow-tooltip />
                    <el-table-column label="来源" width="80">
                      <template #default="{ row }">
                        <a v-if="row.referenceUrl" :href="row.referenceUrl" target="_blank" class="ref-link">链接</a>
                      </template>
                    </el-table-column>
                  </el-table>
                </div>
              </div>
            </div>
          </el-tab-pane>
        </el-tabs>

        <div v-if="activeTab !== 'intel' && activeTab !== 'monitor'" class="scan-progress-panel">
          <div class="scan-progress-header">
            <div>
              <h3>扫描过程</h3>
              <p>{{ currentScanTarget || '等待输入目标' }}</p>
            </div>
            <el-tag v-if="vulnStore.scanning" type="warning" effect="light">执行中</el-tag>
            <el-tag v-else-if="vulnStore.currentResult" type="success" effect="light">已完成</el-tag>
            <el-tag v-else type="info" effect="light">未开始</el-tag>
          </div>

          <div class="scan-progress-bar">
            <el-progress
              :percentage="scanProgressPercent"
              :status="vulnStore.currentResult ? 'success' : undefined"
              :stroke-width="10"
              striped
              striped-flow
            />
          </div>

          <div class="scan-progress-grid">
            <section
              v-for="group in scanProgressGroups"
              :key="group.step"
              class="scan-step-card"
              :class="`scan-step-${group.status}`"
            >
              <div class="scan-step-head">
                <span class="scan-step-index">{{ group.index + 1 }}</span>
                <div>
                  <h4>{{ group.step }}</h4>
                  <p>{{ group.description }}</p>
                </div>
                <el-tag size="small" :type="group.status === 'done' ? 'success' : group.status === 'running' ? 'warning' : 'info'">
                  {{ group.status === 'done' ? '完成' : group.status === 'running' ? '执行中' : '等待' }}
                </el-tag>
              </div>
              <div class="scan-step-lines">
                <div v-if="group.lines.length === 0" class="scan-step-empty">
                  {{ group.status === 'pending' ? '等待后端执行' : group.status === 'running' ? '正在等待后端返回该步骤明细' : '该步骤未返回明细' }}
                </div>
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
      </div>

      <div v-if="vulnStore.currentResult" class="result-section">
        <div class="result-header">
          <div>
            <h3>扫描结果</h3>
            <p class="result-target">
              <el-tag size="small" :type="vulnStore.currentResult.scanType === 'url' ? 'primary' : 'success'">
                {{ scanTypeLabel(vulnStore.currentResult.scanType) }}
              </el-tag>
              {{ vulnStore.currentResult.target }}
            </p>
          </div>
          <div class="result-summary">
            <div v-if="findingCounts" class="finding-counts">
              <div
                v-for="fc in findingCounts"
                :key="fc.label"
                class="finding-badge"
                :class="`badge-${fc.type}`"
              >
                <span class="badge-count">{{ fc.count }}</span>
                <span class="badge-label">{{ fc.label }}</span>
              </div>
            </div>
          </div>
        </div>

        <div v-if="vulnStore.currentResult.findings.length === 0" class="empty-state safe-state">
          <div class="safe-icon">
            <svg width="48" height="48" viewBox="0 0 48 48" fill="none">
              <circle cx="24" cy="24" r="20" stroke="#22c55e" stroke-width="2.5"/>
              <path d="M16 24l6 5 10-10" stroke="#22c55e" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </div>
          <p class="safe-text">未发现安全漏洞</p>
          <p class="safe-hint">扫描未检测到已知的安全漏洞模式</p>
        </div>

        <div v-else class="findings-table-wrapper">
          <table class="findings-table">
            <thead>
              <tr>
                <th style="width: 72px">严重程度</th>
                <th style="width: 110px">类型</th>
                <th style="width: 180px">位置</th>
                <th>描述</th>
                <th>修复建议</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(f, idx) in vulnStore.currentResult.findings" :key="idx">
                <td>
                  <el-tag :type="severityType(f.severity)" size="small" effect="dark">
                    {{ severityLabel(f.severity) }}
                  </el-tag>
                </td>
                <td>
                  <span class="finding-type">{{ f.type }}</span>
                </td>
                <td>
                  <code class="finding-location">{{ f.location }}</code>
                </td>
                <td class="finding-desc">{{ f.description }}</td>
                <td class="finding-suggestion">{{ f.suggestion }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
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
