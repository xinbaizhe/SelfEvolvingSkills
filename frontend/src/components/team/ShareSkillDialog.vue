<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { invoke } from '@tauri-apps/api/core'
import { useTeamStore } from '../../stores/useTeamStore'
import { evaluateShareResource, evaluateSkillDirectory, fetchLlmConfig } from '../../api/admin'
import { fetchAvailableModels, shareSkillToTeam, type TeamModelConfig } from '../../api/team'
import { fetchSkills, fetchSkillDetail, type SkillItem } from '../../api/skills'
import { getErrorMessage } from '../../utils/error'
import { TEAM_CATEGORY_OPTIONS } from '../../constants/team'

type ShareType = 'skill' | 'url'
type EvaluationTargetType = 'directory' | 'url'
type EvaluationModelType = 'department' | 'personal'
type EvaluationResult = {
  score: number
  securityScore: number
  performanceScore: number
  passed: boolean
  summary: string
  penalties: string[]
  suggestions: string[]
  requiredChanges: string[]
  risks: string[]
  heuristicScore: number
  llmScore?: number
  source: 'llm' | 'heuristic'
  scan?: Record<string, unknown>
  agents?: Record<string, unknown>[]
  skills?: Record<string, unknown>[]
  pythonFiles?: Record<string, unknown>[]
}

const store = useTeamStore()
const visible = ref(false)
const loading = ref(false)
const sharing = ref(false)
const modelLoading = ref(false)
const evaluating = ref(false)

const form = reactive({
  shareType: 'skill' as ShareType,
  name: '',
  url: '',
  description: '',
  category: 'general',
  usageGuide: '',
  bodyMd: '',
  originAgent: '',
  compatibleAgents: '',
  evaluationModelType: 'department' as EvaluationModelType,
  evaluationModelId: undefined as number | undefined,
  evaluationTargetType: 'directory' as EvaluationTargetType,
  evaluationDirectory: '',
})

const localSkills = ref<SkillItem[]>([])
const availableModels = ref<TeamModelConfig[]>([])
const localResourceModel = ref<TeamModelConfig | null>(null)
const selectedSkillKey = ref('')
const uploadedZip = ref<{ filename: string; base64: string } | null>(null)
const evaluationResult = ref<EvaluationResult | null>(null)
const evaluationSignature = ref('')

const filteredModels = computed(() => {
  if (form.evaluationModelType === 'personal') {
    return localResourceModel.value ? [localResourceModel.value] : []
  }
  return availableModels.value.filter(model => (model.sourceType || 'department') === 'department')
})
const selectedEvaluationModel = computed(() => filteredModels.value.find(model => model.id === form.evaluationModelId))
const agentOptions = [
  'Claude Code',
  'Codex',
  'Hermes',
  'OpenClaw',
  'Cursor',
  'VSCode',
  'CodeBuddy',
  'TRAE',
  'ZeeLinClaw',
]

const emit = defineEmits<{
  (e: 'shared'): void
}>()

function currentEvaluationSignature() {
  return JSON.stringify({
    shareType: form.shareType,
    name: form.name,
    url: form.url,
    description: form.description,
    usageGuide: form.usageGuide,
    bodyMd: form.bodyMd,
    originAgent: form.originAgent,
    compatibleAgents: form.compatibleAgents,
    uploadedZip: uploadedZip.value?.filename || '',
    uploadedZipSize: uploadedZip.value?.base64.length || 0,
    evaluationModelType: form.evaluationModelType,
    evaluationModelId: form.evaluationModelId,
      evaluationTargetType: form.evaluationTargetType,
      evaluationDirectory: uploadedZip.value ? '' : form.evaluationDirectory,
  })
}

function invalidateEvaluation() {
  evaluationResult.value = null
  evaluationSignature.value = ''
}

async function loadLocalSkills() {
  try {
    const res = await fetchSkills({ size: 200 })
    if (res.success && res.data) {
      localSkills.value = res.data.items ?? []
    }
  } catch { /* Local Skill list failure should not block zip upload. */ }
}

async function loadModels() {
  modelLoading.value = true
  try {
    const [teamModels] = await Promise.all([
      fetchAvailableModels(),
      loadLocalResourceModel(),
    ])
    availableModels.value = teamModels
    ensureModelSelection()
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '加载可用模型失败'))
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
      return
    }
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
      deptName: '资源与配置',
      createdByName: '本机',
      recipientName: '本机',
    }
  } catch {
    localResourceModel.value = null
  }
}

function ensureModelSelection() {
  if (filteredModels.value.some(model => model.id === form.evaluationModelId)) return
  form.evaluationModelId = filteredModels.value[0]?.id
}

watch(selectedSkillKey, async (key) => {
  if (!key) return
  uploadedZip.value = null
  loading.value = true
  try {
    const [sourceType, name] = key.split(':', 2)
    const res = await fetchSkillDetail(name, sourceType)
    if (res.success && res.data) {
      const detail = res.data
      form.name = detail.name
      form.description = detail.description || ''
      form.category = detail.category || 'general'
      form.bodyMd = detail.body_text || ''
      form.originAgent = detail.source_type || ''
    }
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '加载 Skill 详情失败'))
  } finally {
    loading.value = false
  }
})

watch(() => form.evaluationModelType, () => {
  ensureModelSelection()
})

watch(() => [
  form.shareType,
  form.name,
  form.url,
  form.description,
  form.bodyMd,
  form.originAgent,
  form.compatibleAgents,
  form.evaluationModelType,
  form.evaluationModelId,
  form.evaluationTargetType,
  form.evaluationDirectory,
  uploadedZip.value?.filename || '',
  uploadedZip.value?.base64.length || 0,
], () => {
  invalidateEvaluation()
})

function resetForm() {
  form.shareType = 'skill'
  form.name = ''
  form.url = ''
  form.description = ''
  form.usageGuide = ''
  form.category = 'general'
  form.bodyMd = ''
  form.originAgent = ''
  form.compatibleAgents = ''
  form.evaluationModelType = 'department'
  form.evaluationModelId = undefined
  form.evaluationTargetType = 'directory'
  form.evaluationDirectory = ''
  selectedSkillKey.value = ''
  uploadedZip.value = null
  invalidateEvaluation()
}

function open() {
  if (!store.isAuthenticated) {
    ElMessage.warning('请先登录团队版')
    return
  }
  resetForm()
  visible.value = true
  loadLocalSkills()
  loadModels()
}

async function onZipSelected(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  if (!file.name.toLowerCase().endsWith('.zip')) {
    ElMessage.warning('请上传 .zip 文件')
    return
  }

  try {
    uploadedZip.value = {
      filename: file.name,
      base64: await fileToBase64(file),
    }
    selectedSkillKey.value = ''
    form.bodyMd = ''
    if (!form.name) form.name = file.name.replace(/\.zip$/i, '')
    ElMessage.success('zip 已选择')
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '读取 zip 文件失败'))
  }
}

function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      const value = String(reader.result || '')
      resolve(value.includes(',') ? value.split(',')[1] : value)
    }
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(file)
  })
}

function buildEvaluationSection() {
  const model = selectedEvaluationModel.value
  const targetLabel = form.evaluationTargetType === 'url' ? '工具网址' : '输入目录'
  const targetValue = form.evaluationTargetType === 'url' ? form.url : form.evaluationDirectory
  return [
    '',
    '## AI 评估配置',
    '',
    `- 评估维度: 安全, 性能`,
    `- 目标类型: ${targetLabel}`,
    `- 评估目标: ${targetValue}`,
    `- 模型类型: ${form.evaluationModelType === 'personal' ? '资源与配置的模型配置' : '部门模型'}`,
    `- 模型名称: ${model?.name || ''}`,
    `- 模型标识: ${model?.model || ''}`,
    `- 服务商: ${model?.provider || ''}`,
  ].join('\n')
}

function buildPayloadBody() {
  const usageSection = ['## 使用说明', '', form.usageGuide].join('\n')
  if (form.shareType === 'url') {
    return [
      `# ${form.name}`,
      '',
      `URL: ${form.url}`,
      '',
      form.description || '',
      '',
      usageSection,
      buildEvaluationSection(),
    ].join('\n').trim()
  }
  if (uploadedZip.value) {
    return [
      `# ${form.name}`,
      '',
      form.description || '',
      '',
      usageSection,
      buildEvaluationSection(),
    ].join('\n').trim()
  }
  return [form.bodyMd, usageSection, buildEvaluationSection()].join('\n\n').trim()
}

function isValidHttpUrl(value: string) {
  try {
    const url = new URL(value)
    return url.protocol === 'http:' || url.protocol === 'https:'
  } catch {
    return false
  }
}

async function selectEvaluationDirectory() {
  if (!window.__TAURI_INTERNALS__) {
    ElMessage.warning('当前环境不支持原生目录选择')
    return
  }
  const selected = await invoke<string | null>('select_directory')
  if (selected) form.evaluationDirectory = selected
}

function validateForm() {
  if (!form.name) {
    ElMessage.warning('请填写名称')
    return false
  }
  if (!form.usageGuide.trim()) {
    ElMessage.warning('请填写使用说明')
    return false
  }
  if (form.shareType === 'skill' && !form.bodyMd && !uploadedZip.value) {
    ElMessage.warning('请选择本地 Skill 或上传 Skill zip')
    return false
  }
  if (form.shareType === 'url' && !form.url) {
    ElMessage.warning('请填写工具网址')
    return false
  }
  if (form.shareType === 'url' && !isValidHttpUrl(form.url)) {
    ElMessage.warning('请输入 http 或 https 开头的有效网址')
    return false
  }
  if (!form.evaluationModelId) {
    ElMessage.warning('请选择用于 AI 评估的模型')
    return false
  }
  if (form.evaluationTargetType === 'directory' && !uploadedZip.value && !form.evaluationDirectory) {
    ElMessage.warning('请选择评估目录')
    return false
  }
  if (form.evaluationTargetType === 'url' && !form.url) {
    ElMessage.warning('请填写工具网址作为评估目标')
    return false
  }
  return true
}

function clampScore(value: number) {
  if (!Number.isFinite(value)) return 0
  return Math.max(0, Math.min(100, Math.round(value)))
}

function addPenalty(penalties: string[], label: string, points: number) {
  penalties.push(`${label} -${points}`)
  return points
}

function scoreSecurity(penalties: string[]) {
  const body = `${form.bodyMd}\n${form.description}\n${form.url}`.toLowerCase()
  let deductions = 0
  if (/api[_-]?key|secret|token|password|bearer\s+[a-z0-9._-]{16,}/i.test(body)) {
    deductions += addPenalty(penalties, '内容疑似包含密钥、令牌或密码', 18)
  }
  if (/(rm\s+-rf|del\s+\/[sq]|format\s+[a-z]:|shutdown\s+\/|powershell\s+-enc|curl\s+.*\|\s*(sh|bash)|wget\s+.*\|\s*(sh|bash))/i.test(body)) {
    deductions += addPenalty(penalties, '内容包含高危系统命令或远程脚本执行模式', 20)
  }
  if (form.shareType === 'url' && form.url && !isValidHttpUrl(form.url)) {
    deductions += addPenalty(penalties, '工具网址不是有效 HTTPS/HTTP 地址', 12)
  }
  if (form.shareType === 'url' && form.url.startsWith('http://')) {
    deductions += addPenalty(penalties, '工具网址未使用 HTTPS', 6)
  }
  if (form.shareType === 'skill' && !form.originAgent) {
    deductions += addPenalty(penalties, '未选择来源 Agent', 5)
  }
  if (form.shareType === 'skill' && !form.compatibleAgents.trim()) {
    deductions += addPenalty(penalties, '未填写兼容 Agent 范围', 4)
  }
  if (!selectedEvaluationModel.value?.model) {
    deductions += addPenalty(penalties, '未选择可用评估模型', 25)
  }
  if (form.evaluationTargetType === 'directory' && !uploadedZip.value && !form.evaluationDirectory) {
    deductions += addPenalty(penalties, '未选择评估目录', 12)
  }
  return clampScore(100 - deductions)
}

function scorePerformance(penalties: string[]) {
  let deductions = 0
  const contentLength = uploadedZip.value?.base64.length || form.bodyMd.length
  if (contentLength === 0) {
    deductions += addPenalty(penalties, '缺少可评估内容', 18)
  } else if (contentLength > 1_500_000) {
    deductions += addPenalty(penalties, '内容体积过大，离线分发和解析成本高', 16)
  } else if (contentLength > 300_000) {
    deductions += addPenalty(penalties, '内容体积偏大', 8)
  }
  if (form.shareType === 'skill' && !uploadedZip.value && form.bodyMd.length < 120) {
    deductions += addPenalty(penalties, 'Skill 内容过短，缺少足够执行说明', 8)
  }
  if (form.description.trim().length < 8) {
    deductions += addPenalty(penalties, '描述过短，不利于检索和复用', 4)
  }
  if (form.evaluationTargetType === 'url' && form.url.length > 220) {
    deductions += addPenalty(penalties, '评估 URL 过长，稳定性较差', 3)
  }
  if (form.evaluationModelType === 'personal' && !selectedEvaluationModel.value?.apiKeyHash) {
    deductions += addPenalty(penalties, '资源与配置模型未确认 API Key 可用', 8)
  }
  return clampScore(100 - deductions)
}

function calculateEvaluationScore(): EvaluationResult {
  const penalties: string[] = []
  const securityScore = scoreSecurity(penalties)
  const performanceScore = scorePerformance(penalties)
  const score = clampScore(securityScore * 0.6 + performanceScore * 0.4)
  return {
    score,
    securityScore,
    performanceScore,
    passed: score >= 95,
    summary: score >= 95 ? '评估通过，可以提交分享。' : '评估未通过，需要总分达到 95 分以上才能提交分享。',
    penalties,
    suggestions: [],
    requiredChanges: [],
    risks: penalties,
    heuristicScore: score,
    source: 'heuristic',
    agents: [],
    skills: [],
  }
}

function stringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.map(item => String(item)).filter(Boolean)
    : []
}

function textValue(value: unknown) {
  return value === undefined || value === null ? '' : String(value)
}

function scoreValue(value: unknown) {
  const score = Number(value)
  return Number.isFinite(score) ? Math.round(score) : 0
}

function evaluationErrorFallback() {
  return form.evaluationModelType === 'department'
    ? '大模型评估失败，请检查所选部门模型配置、API Key、Base URL 和模型标识'
    : '大模型评估失败，请检查资源与配置中的模型配置'
}

function selectedDepartmentEvaluationConfig() {
  if (form.evaluationModelType !== 'department' || !selectedEvaluationModel.value) return null
  return {
    provider: selectedEvaluationModel.value.provider,
    baseUrl: selectedEvaluationModel.value.baseUrl,
    model: selectedEvaluationModel.value.model,
    apiKey: selectedEvaluationModel.value.apiKeyHash,
    apiFormat: selectedEvaluationModel.value.provider,
  }
}

function buildEvaluationPayload(heuristic: EvaluationResult) {
  return {
    metadata: {
      shareType: form.shareType,
      name: form.name,
      url: form.url,
      description: form.description,
      usageGuide: form.usageGuide,
      category: form.category,
      originAgent: form.originAgent,
      compatibleAgents: form.compatibleAgents,
      uploadedZip: uploadedZip.value?.filename || '',
      evaluationTargetType: form.evaluationTargetType,
      evaluationTarget: form.evaluationTargetType === 'url' ? form.url : form.evaluationDirectory,
      evaluationModelType: form.evaluationModelType,
      evaluationModel: selectedEvaluationModel.value?.model || '',
      evaluationModelConfig: selectedDepartmentEvaluationConfig(),
    },
    heuristic: {
      score: heuristic.score,
      securityScore: heuristic.securityScore,
      performanceScore: heuristic.performanceScore,
      penalties: heuristic.penalties,
    },
    content: uploadedZip.value
      ? [
        buildPayloadBody(),
        '',
        `ZIP_FILE: ${uploadedZip.value.filename}`,
        `ZIP_BASE64: ${uploadedZip.value.base64}`,
      ].join('\n')
      : buildPayloadBody(),
  }
}

function buildDirectoryEvaluationPayload(heuristic: EvaluationResult) {
  return {
    directory: form.evaluationDirectory,
    metadata: {
      shareType: form.shareType,
      name: form.name,
      description: form.description,
      usageGuide: form.usageGuide,
      category: form.category,
      originAgent: form.originAgent,
      compatibleAgents: form.compatibleAgents,
      uploadedZip: uploadedZip.value?.filename || '',
      evaluationModelType: form.evaluationModelType,
      evaluationModel: selectedEvaluationModel.value?.model || '',
      evaluationModelConfig: selectedDepartmentEvaluationConfig(),
    },
    heuristic: {
      score: heuristic.score,
      securityScore: heuristic.securityScore,
      performanceScore: heuristic.performanceScore,
      penalties: heuristic.penalties,
    },
  }
}

async function evaluateWithLlm(heuristic: EvaluationResult): Promise<EvaluationResult> {
  const isDirectoryEvaluation = form.evaluationTargetType === 'directory' && !uploadedZip.value
  const res = isDirectoryEvaluation
    ? await evaluateSkillDirectory(buildDirectoryEvaluationPayload(heuristic) as Record<string, unknown>)
    : await evaluateShareResource(buildEvaluationPayload(heuristic) as Record<string, unknown>)
  if (!res.success) {
    throw new Error(res.error || '大模型评估失败')
  }
  const data = (res.data || {}) as Record<string, unknown>
  const llmScore = clampScore(Number(data.score ?? 0))
  const llmSecurityScore = clampScore(Number(data.securityScore ?? llmScore))
  const llmPerformanceScore = clampScore(Number(data.performanceScore ?? llmScore))
  const finalScore = Math.min(heuristic.score, llmScore)
  const securityScore = Math.min(heuristic.securityScore, llmSecurityScore)
  const performanceScore = Math.min(heuristic.performanceScore, llmPerformanceScore)
  return {
    score: finalScore,
    securityScore,
    performanceScore,
    passed: finalScore >= 95,
    summary: String(data.summary || (finalScore >= 95 ? '大模型评估通过，可以提交分享。' : '大模型评估未通过，需要按建议修改后重新评估。')),
    penalties: heuristic.penalties,
    suggestions: stringArray(data.suggestions),
    requiredChanges: stringArray(data.requiredChanges),
    risks: stringArray(data.risks),
    heuristicScore: heuristic.score,
    llmScore,
    source: 'llm',
    scan: (data.scan || undefined) as Record<string, unknown> | undefined,
    agents: Array.isArray(data.agents) ? data.agents as Record<string, unknown>[] : [],
    skills: Array.isArray(data.skills) ? data.skills as Record<string, unknown>[] : [],
    pythonFiles: Array.isArray(data.pythonFiles) ? data.pythonFiles as Record<string, unknown>[] : [],
  }
}

async function runEvaluation() {
  if (!validateForm()) return
  evaluating.value = true
  try {
    const heuristic = calculateEvaluationScore()
    evaluationResult.value = await evaluateWithLlm(heuristic)
    evaluationSignature.value = currentEvaluationSignature()
    if (evaluationResult.value.passed) {
      ElMessage.success(`评估通过：${evaluationResult.value.score} 分`)
    } else {
      ElMessage.warning(`评估未通过：${evaluationResult.value.score} 分，需达到 95 分以上`)
    }
  } catch (e: unknown) {
    evaluationResult.value = null
    evaluationSignature.value = ''
    ElMessage.error(getErrorMessage(e, evaluationErrorFallback()))
  } finally {
    evaluating.value = false
  }
}

function validateEvaluationResult() {
  if (!evaluationResult.value || evaluationSignature.value !== currentEvaluationSignature()) {
    ElMessage.warning('请先完成 AI 安全与性能评估')
    return false
  }
  if (!evaluationResult.value.passed) {
    ElMessage.warning('评估分数需达到 95 分以上才能提交分享')
    return false
  }
  return true
}

async function doShare() {
  if (!validateForm()) return
  if (!validateEvaluationResult()) return
  sharing.value = true
  try {
    await shareSkillToTeam({
      name: form.name,
      description: form.description,
      usageGuide: form.usageGuide,
      category: form.category || 'general',
      sourceType: form.shareType,
      bodyMd: buildPayloadBody(),
      originAgent: form.shareType === 'url' ? 'tool-url' : (form.originAgent || 'skill-zip'),
      compatibleModels: selectedEvaluationModel.value?.model,
      compatibleAgents: form.compatibleAgents || undefined,
      zipFileName: uploadedZip.value?.filename,
      zipBase64: uploadedZip.value?.base64,
    })
    ElMessage.success(form.shareType === 'url' ? '工具网址已分享到团队' : 'Skill zip 已分享到团队')
    visible.value = false
    emit('shared')
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '分享失败'))
  } finally {
    sharing.value = false
  }
}

defineExpose({ open })
</script>

<template>
  <el-dialog v-model="visible" title="分享 Skill 到团队" width="720px" top="6vh">
    <el-form label-position="top" @submit.prevent="doShare">
      <el-alert
        class="share-tip"
        type="info"
        :closable="false"
        show-icon
        title="可以上传 Skill zip 或选择本地 Skill，提交前必须完成 AI 安全与性能评估配置。"
      />

      <el-form-item label="分享类型">
        <el-segmented
          v-model="form.shareType"
          :options="[
            { label: 'Skill zip', value: 'skill' },
            { label: '工具网址', value: 'url' },
          ]"
        />
      </el-form-item>

      <template v-if="form.shareType === 'skill'">
        <el-form-item label="上传 Skill zip">
          <label class="upload-box">
            <input class="hidden-input" type="file" accept=".zip,application/zip" @change="onZipSelected" />
            <span class="upload-icon">ZIP</span>
            <div>
              <strong>{{ uploadedZip?.filename || '选择 zip 文件' }}</strong>
              <span>上传 zip 后会作为团队 Skill zip 分享；也可以改为选择下方本地 Skill。</span>
            </div>
          </label>
        </el-form-item>

        <el-form-item label="或选择本地 Skill">
          <el-select
            v-model="selectedSkillKey"
            placeholder="选择要分享的 Skill..."
            filterable
            clearable
            style="width: 100%"
            :loading="loading"
          >
            <el-option
              v-for="skill in localSkills"
              :key="`${skill.source_type}:${skill.name}`"
              :label="skill.name"
              :value="`${skill.source_type}:${skill.name}`"
            >
              <span>{{ skill.name }}</span>
              <span class="option-meta">{{ skill.category || 'general' }} · {{ skill.source_type }}</span>
            </el-option>
          </el-select>
        </el-form-item>
      </template>

      <div class="form-grid">
        <el-form-item label="名称">
          <el-input v-model="form.name" placeholder="Skill 名称" />
        </el-form-item>

        <el-form-item label="分类">
          <el-select v-model="form.category" style="width: 100%">
            <el-option v-for="item in TEAM_CATEGORY_OPTIONS" :key="item.value" :label="item.label" :value="item.value" />
          </el-select>
        </el-form-item>
      </div>

      <el-form-item v-if="form.shareType === 'url'" label="工具网址">
        <el-input v-model="form.url" placeholder="https://example.com/tool" />
      </el-form-item>

      <el-form-item label="描述">
        <el-input v-model="form.description" type="textarea" :rows="2" placeholder="简短说明这个资源解决什么问题" />
      </el-form-item>

      <el-form-item label="使用说明">
        <el-input
          v-model="form.usageGuide"
          type="textarea"
          :rows="4"
          placeholder="必填：说明安装方式、调用方式、输入输出、适用场景和注意事项。"
        />
      </el-form-item>

      <div v-if="form.shareType === 'skill'" class="form-grid">
        <el-form-item label="来源 Agent">
          <el-select v-model="form.originAgent" placeholder="请选择来源 Agent" clearable style="width: 100%">
            <el-option v-for="agent in agentOptions" :key="agent" :label="agent" :value="agent" />
          </el-select>
        </el-form-item>
      </div>

      <el-form-item v-if="form.shareType === 'skill'" label="兼容 Agent">
        <el-input v-model="form.compatibleAgents" placeholder="逗号分隔，如 codex, claude-code, cursor" />
      </el-form-item>

      <el-form-item v-if="form.shareType === 'skill' && !uploadedZip" label="Skill 内容">
        <el-input
          v-model="form.bodyMd"
          type="textarea"
          :rows="7"
          placeholder="SKILL.md 内容"
        />
      </el-form-item>

      <section class="evaluation-panel">
        <div class="evaluation-head">
          <div>
            <div class="evaluation-title">AI 安全与性能评估</div>
            <p>总分 95 分以上才能提交分享。</p>
          </div>
          <el-button type="primary" plain :loading="evaluating" @click="runEvaluation">开始评估</el-button>
        </div>

        <div class="form-grid evaluation-grid">
          <el-form-item label="模型类型">
            <el-segmented
              v-model="form.evaluationModelType"
              :options="[
                { label: '部门模型', value: 'department' },
                { label: '资源与配置的模型配置', value: 'personal' },
              ]"
            />
          </el-form-item>

          <el-form-item label="评估模型">
            <el-select
              v-model="form.evaluationModelId"
              placeholder="请选择评估模型"
              filterable
              style="width: 100%"
              :loading="modelLoading"
              no-data-text="暂无可用模型，请先到资源与配置中启用模型配置"
            >
              <el-option
                v-for="model in filteredModels"
                :key="model.id"
                :label="`${model.name} · ${model.model}`"
                :value="model.id"
              >
                <span>{{ model.name }}</span>
                <span class="option-meta">{{ model.provider }} · {{ model.model }}</span>
              </el-option>
            </el-select>
          </el-form-item>
        </div>

        <el-form-item label="评估目标">
          <div class="target-row">
            <template v-if="uploadedZip">
              <el-tag type="info" effect="plain">上传 zip</el-tag>
              <el-input :model-value="uploadedZip.filename" readonly />
            </template>
            <template v-else>
              <el-segmented
                v-model="form.evaluationTargetType"
                :options="[
                  { label: '目录', value: 'directory' },
                  { label: '工具网址', value: 'url' },
                ]"
              />
            </template>
            <template v-if="!uploadedZip && form.evaluationTargetType === 'directory'">
              <el-input v-model="form.evaluationDirectory" readonly placeholder="请选择本地目录" />
              <el-button @click="selectEvaluationDirectory">选择目录</el-button>
            </template>
            <el-input v-else-if="!uploadedZip" v-model="form.url" placeholder="https://example.com/tool" clearable />
          </div>
        </el-form-item>

        <div v-if="evaluationResult" class="evaluation-result" :class="{ passed: evaluationResult.passed }">
          <div class="score-block">
            <strong>{{ evaluationResult.score }}</strong>
            <span>综合分</span>
          </div>
          <div class="score-detail">
            <div>{{ evaluationResult.summary }}</div>
            <small>
              安全 {{ evaluationResult.securityScore }} / 性能 {{ evaluationResult.performanceScore }}
              / 规则 {{ evaluationResult.heuristicScore }}
              <template v-if="evaluationResult.llmScore !== undefined"> / 大模型 {{ evaluationResult.llmScore }}</template>
            </small>
            <ul v-if="evaluationResult.requiredChanges.length">
              <li v-for="item in evaluationResult.requiredChanges" :key="`required-${item}`">必须修改：{{ item }}</li>
            </ul>
            <ul v-if="evaluationResult.suggestions.length">
              <li v-for="item in evaluationResult.suggestions" :key="`suggestion-${item}`">建议：{{ item }}</li>
            </ul>
            <ul v-if="evaluationResult.risks.length">
              <li v-for="item in evaluationResult.risks" :key="`risk-${item}`">风险：{{ item }}</li>
            </ul>
            <ul v-if="evaluationResult.penalties.length">
              <li v-for="item in evaluationResult.penalties" :key="`penalty-${item}`">规则扣分：{{ item }}</li>
            </ul>
          </div>
        </div>

        <div v-if="evaluationResult?.scan" class="scan-summary">
          <span>目录扫描</span>
          <strong>{{ evaluationResult.scan.skillCount || 0 }}</strong>
          <span>个 Skills</span>
          <strong>{{ evaluationResult.scan.pythonFileCount || 0 }}</strong>
          <span>个 Python 文件</span>
          <small>扫描文件 {{ evaluationResult.scan.scannedFiles || 0 }} 个</small>
        </div>

        <div v-if="evaluationResult?.agents?.length" class="agent-review-grid">
          <div v-for="agent in evaluationResult.agents" :key="textValue(agent.agent)" class="agent-review-item">
            <div class="agent-review-head">
              <strong>{{ textValue(agent.agent) }}</strong>
              <span>{{ scoreValue(agent.score) }}</span>
            </div>
            <small>{{ textValue(agent.verdict) }}</small>
            <ul v-if="Array.isArray(agent.findings) && agent.findings.length">
              <li v-for="item in agent.findings" :key="textValue(item)">{{ textValue(item) }}</li>
            </ul>
            <ul v-if="Array.isArray(agent.suggestions) && agent.suggestions.length">
              <li v-for="item in agent.suggestions" :key="`agent-suggestion-${textValue(item)}`">建议：{{ textValue(item) }}</li>
            </ul>
          </div>
        </div>

        <div v-if="evaluationResult?.skills?.length" class="skill-review-list">
          <div v-for="skill in evaluationResult.skills" :key="textValue(skill.name)" class="skill-review-item">
            <strong>{{ textValue(skill.name) }}</strong>
            <span>{{ scoreValue(skill.score) }}</span>
            <small v-if="Array.isArray(skill.risks) && skill.risks.length">风险：{{ skill.risks.map(textValue).join('；') }}</small>
            <small v-if="Array.isArray(skill.suggestions) && skill.suggestions.length">建议：{{ skill.suggestions.map(textValue).join('；') }}</small>
          </div>
        </div>

        <div v-if="evaluationResult?.pythonFiles?.length" class="skill-review-list">
          <div v-for="file in evaluationResult.pythonFiles" :key="textValue(file.path)" class="skill-review-item">
            <strong>{{ textValue(file.path) }}</strong>
            <span>{{ scoreValue(file.score) }}</span>
            <small v-if="Array.isArray(file.risks) && file.risks.length">风险：{{ file.risks.map(textValue).join('；') }}</small>
            <small v-if="Array.isArray(file.suggestions) && file.suggestions.length">建议：{{ file.suggestions.map(textValue).join('；') }}</small>
          </div>
        </div>
      </section>
    </el-form>

    <template #footer>
      <el-button @click="visible = false">取消</el-button>
      <el-button type="primary" :loading="sharing" :disabled="!evaluationResult?.passed" @click="doShare">提交分享</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.share-tip {
  margin-bottom: 16px;
}

.hidden-input {
  display: none;
}

.upload-box {
  display: flex;
  gap: 12px;
  width: 100%;
  padding: 14px;
  border: 1px dashed var(--line);
  border-radius: 8px;
  background: #fafbfe;
  cursor: pointer;
  transition: border-color .2s, background .2s;
}

.upload-box:hover {
  border-color: #0d9488;
  background: rgba(13, 148, 136, .04);
}

.upload-box strong,
.upload-box span {
  display: block;
}

.upload-box span {
  margin-top: 4px;
  color: var(--muted);
  font-size: 12px;
}

.upload-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: 0 0 40px;
  width: 40px;
  height: 40px;
  border-radius: 6px;
  background: rgba(13, 148, 136, .1);
  color: #0d9488;
  font-size: 12px;
  font-weight: 700;
}

.form-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.evaluation-panel {
  margin: 12px 0;
  padding: 14px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--panel);
}

.evaluation-title {
  font-weight: 600;
  color: var(--text);
}

.evaluation-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.evaluation-head p {
  margin: 4px 0 0;
  color: var(--muted);
  font-size: 12px;
}

.evaluation-grid {
  margin-top: 12px;
}

.evaluation-result {
  display: grid;
  grid-template-columns: 88px minmax(0, 1fr);
  gap: 12px;
  margin-top: 12px;
  padding: 12px;
  border: 1px solid #f3c7c7;
  border-radius: 8px;
  background: #fff7f7;
}

.evaluation-result.passed {
  border-color: #b9e3cb;
  background: #f3fbf6;
}

.score-block {
  display: grid;
  place-items: center;
  align-content: center;
  min-height: 72px;
  border-radius: 8px;
  background: #fff;
}

.score-block strong {
  color: #b42318;
  font-size: 28px;
  line-height: 1;
}

.evaluation-result.passed .score-block strong {
  color: #15803d;
}

.score-block span,
.score-detail small {
  color: var(--muted);
  font-size: 12px;
}

.score-detail {
  min-width: 0;
  color: var(--text);
  font-size: 13px;
  line-height: 1.5;
}

.score-detail ul {
  margin: 6px 0 0;
  padding-left: 18px;
  color: #7f1d1d;
}

.evaluation-result.passed .score-detail ul {
  color: #166534;
}

.scan-summary {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin-top: 12px;
  padding: 10px 12px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: #fff;
  color: var(--muted);
  font-size: 12px;
}

.scan-summary strong {
  color: var(--text);
  font-size: 18px;
}

.scan-summary small {
  margin-left: auto;
}

.agent-review-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  margin-top: 12px;
}

.agent-review-item,
.skill-review-item {
  min-width: 0;
  padding: 10px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: #fff;
  font-size: 12px;
}

.agent-review-head,
.skill-review-item {
  display: flex;
  gap: 8px;
  align-items: flex-start;
}

.agent-review-head {
  justify-content: space-between;
}

.agent-review-head strong,
.skill-review-item strong {
  min-width: 0;
  color: var(--text);
}

.agent-review-head span,
.skill-review-item span {
  flex: 0 0 auto;
  color: #0d9488;
  font-weight: 700;
}

.agent-review-item small,
.skill-review-item small {
  display: block;
  margin-top: 4px;
  color: var(--muted);
}

.agent-review-item ul {
  margin: 6px 0 0;
  padding-left: 16px;
  color: var(--muted);
}

.skill-review-list {
  display: grid;
  gap: 8px;
  margin-top: 12px;
}

.skill-review-item {
  flex-wrap: wrap;
}

.skill-review-item small {
  flex-basis: 100%;
}

.target-row {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  gap: 10px;
  width: 100%;
  align-items: center;
}

.option-meta {
  float: right;
  color: var(--muted);
  font-size: 12px;
}

@media (max-width: 640px) {
  .form-grid,
  .target-row,
  .evaluation-result,
  .agent-review-grid {
    grid-template-columns: 1fr;
  }

  .evaluation-head {
    flex-direction: column;
  }
}
</style>
