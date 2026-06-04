<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useTeamStore } from '../../stores/useTeamStore'
import { evaluateShareResource, fetchLlmConfig } from '../../api/admin'
import { fetchAvailableModels, fetchTeamSkills, fetchTeamSkillDetail, submitEvolution, type TeamModelConfig, type TeamSkill } from '../../api/team'
import { getErrorMessage } from '../../utils/error'

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
}

const store = useTeamStore()
const visible = ref(false)
const loading = ref(false)
const submitting = ref(false)
const modelLoading = ref(false)
const evaluating = ref(false)
const skills = ref<TeamSkill[]>([])
const availableModels = ref<TeamModelConfig[]>([])
const localResourceModel = ref<TeamModelConfig | null>(null)
const currentBody = ref('')
const evaluationResult = ref<EvaluationResult | null>(null)
const evaluationSignature = ref('')

const form = reactive({
  skillId: 0,
  skillName: '',
  currentVersion: 0,
  proposedChange: '',
  reason: '',
  evaluationModelType: 'department' as EvaluationModelType,
  evaluationModelId: undefined as number | undefined,
})

const selectedVersionLabel = computed(() =>
  form.currentVersion > 0 ? `${form.skillName} v${form.currentVersion}` : '未选择 Skill'
)
const filteredModels = computed(() => {
  if (form.evaluationModelType === 'personal') {
    return localResourceModel.value ? [localResourceModel.value] : []
  }
  return availableModels.value.filter(model => (model.sourceType || 'department') === 'department')
})
const selectedEvaluationModel = computed(() => filteredModels.value.find(model => model.id === form.evaluationModelId))

const emit = defineEmits<{
  (e: 'submitted'): void
}>()

async function loadSkills() {
  loading.value = true
  try {
    const res = await fetchTeamSkills({ pageSize: '100' })
    skills.value = res.items
  } catch { /* 团队 Skill 加载失败时仍保留弹窗 */ }
  finally { loading.value = false }
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

function invalidateEvaluation() {
  evaluationResult.value = null
  evaluationSignature.value = ''
}

function currentEvaluationSignature() {
  return JSON.stringify({
    skillId: form.skillId,
    proposedChange: form.proposedChange,
    reason: form.reason,
    evaluationModelType: form.evaluationModelType,
    evaluationModelId: form.evaluationModelId,
  })
}

async function onSkillSelect(skillId: number) {
  if (!skillId) {
    currentBody.value = ''
    form.currentVersion = 0
    form.skillName = ''
    form.proposedChange = ''
    invalidateEvaluation()
    return
  }
  loading.value = true
  try {
    const skill = await fetchTeamSkillDetail(skillId)
    currentBody.value = skill.bodyMd || ''
    form.currentVersion = skill.version || 1
    form.skillName = skill.name
    form.proposedChange = skill.bodyMd || ''
    invalidateEvaluation()
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '加载团队 Skill 详情失败'))
  } finally {
    loading.value = false
  }
}

function resetForm(skillId?: number) {
  form.skillId = skillId || 0
  form.proposedChange = ''
  form.reason = ''
  form.currentVersion = 0
  form.skillName = ''
  form.evaluationModelType = 'department'
  form.evaluationModelId = undefined
  currentBody.value = ''
  invalidateEvaluation()
}

function open(skillId?: number) {
  if (!store.isAuthenticated) {
    ElMessage.warning('请先登录团队版')
    return
  }
  resetForm(skillId)
  visible.value = true
  loadSkills()
  loadModels()
  if (skillId) onSkillSelect(skillId)
}

function validateForm() {
  if (!form.skillId || !form.proposedChange.trim()) {
    ElMessage.warning('请选择 Skill，并填写改进后的完整内容')
    return false
  }
  if (!form.reason.trim()) {
    ElMessage.warning('请填写改进理由，方便团队审核')
    return false
  }
  if (!form.evaluationModelId) {
    ElMessage.warning('请选择用于 AI 评估的模型')
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

function calculateEvaluationScore(): EvaluationResult {
  const penalties: string[] = []
  const body = `${form.proposedChange}\n${form.reason}`.toLowerCase()
  let securityDeductions = 0
  let performanceDeductions = 0
  if (/api[_-]?key|secret|token|password|bearer\s+[a-z0-9._-]{16,}/i.test(body)) {
    securityDeductions += addPenalty(penalties, '改进内容疑似包含密钥、令牌或密码', 18)
  }
  if (/(rm\s+-rf|del\s+\/[sq]|format\s+[a-z]:|shutdown\s+\/|powershell\s+-enc|curl\s+.*\|\s*(sh|bash)|wget\s+.*\|\s*(sh|bash))/i.test(body)) {
    securityDeductions += addPenalty(penalties, '改进内容包含高危系统命令或远程脚本执行模式', 20)
  }
  if (form.proposedChange.trim().length < 120) {
    performanceDeductions += addPenalty(penalties, '改进后的 Skill 内容过短，缺少完整执行说明', 10)
  }
  if (form.reason.trim().length < 8) {
    performanceDeductions += addPenalty(penalties, '改进理由过短，不利于团队审核', 5)
  }
  if (form.proposedChange.length > 300_000) {
    performanceDeductions += addPenalty(penalties, '改进内容体积偏大，审核和分发成本高', 8)
  }
  if (!selectedEvaluationModel.value?.model) {
    securityDeductions += addPenalty(penalties, '未选择可用评估模型', 25)
  }
  const securityScore = clampScore(100 - securityDeductions)
  const performanceScore = clampScore(100 - performanceDeductions)
  const score = clampScore(securityScore * 0.6 + performanceScore * 0.4)
  return {
    score,
    securityScore,
    performanceScore,
    passed: score >= 95,
    summary: score >= 95 ? '评估通过，可以提交提案。' : '评估未通过，需要总分达到 95 分以上才能提交提案。',
    penalties,
    suggestions: [],
    requiredChanges: [],
    risks: penalties,
    heuristicScore: score,
  }
}

function stringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.map(item => String(item)).filter(Boolean) : []
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
      shareType: 'evolution-proposal',
      name: `${form.skillName} 进化提案`,
      description: form.reason,
      category: 'evolution',
      originAgent: 'team-evolution',
      compatibleAgents: '',
      evaluationTargetType: 'content',
      evaluationTarget: form.skillName,
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
    content: [
      `# ${form.skillName} 进化提案`,
      '',
      `当前版本: v${form.currentVersion || 1}`,
      '',
      '## 改进理由',
      form.reason,
      '',
      '## 改进后的完整内容',
      form.proposedChange,
    ].join('\n'),
  }
}

function evaluationErrorFallback() {
  return form.evaluationModelType === 'department'
    ? '大模型评估失败，请检查所选部门模型配置、API Key、Base URL 和模型标识'
    : '大模型评估失败，请检查资源与配置中的模型配置'
}

async function runEvaluation() {
  if (!validateForm()) return
  evaluating.value = true
  try {
    const heuristic = calculateEvaluationScore()
    const res = await evaluateShareResource(buildEvaluationPayload(heuristic) as Record<string, unknown>)
    if (!res.success) throw new Error(res.error || '大模型评估失败')
    const data = (res.data || {}) as Record<string, unknown>
    const llmScore = clampScore(Number(data.score ?? 0))
    const llmSecurityScore = clampScore(Number(data.securityScore ?? llmScore))
    const llmPerformanceScore = clampScore(Number(data.performanceScore ?? llmScore))
    const finalScore = Math.min(heuristic.score, llmScore)
    evaluationResult.value = {
      score: finalScore,
      securityScore: Math.min(heuristic.securityScore, llmSecurityScore),
      performanceScore: Math.min(heuristic.performanceScore, llmPerformanceScore),
      passed: finalScore >= 95,
      summary: String(data.summary || (finalScore >= 95 ? '大模型评估通过，可以提交提案。' : '大模型评估未通过，需要按建议修改后重新评估。')),
      penalties: heuristic.penalties,
      suggestions: stringArray(data.suggestions),
      requiredChanges: stringArray(data.requiredChanges),
      risks: stringArray(data.risks),
      heuristicScore: heuristic.score,
      llmScore,
    }
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
    ElMessage.warning('评估分数需达到 95 分以上才能提交提案')
    return false
  }
  return true
}

async function doSubmit() {
  if (!validateForm()) return
  if (!validateEvaluationResult()) return
  submitting.value = true
  try {
    await submitEvolution({
      skillId: form.skillId,
      proposedChange: form.proposedChange,
      reason: [
        form.reason,
        '',
        '## AI 安全与性能评估',
        `综合分: ${evaluationResult.value?.score ?? 0}`,
        `安全分: ${evaluationResult.value?.securityScore ?? 0}`,
        `性能分: ${evaluationResult.value?.performanceScore ?? 0}`,
        `大模型分: ${evaluationResult.value?.llmScore ?? 0}`,
        `评估摘要: ${evaluationResult.value?.summary || ''}`,
      ].join('\n'),
      previousVersion: form.currentVersion > 0 ? `v${form.currentVersion}` : undefined,
    })
    ElMessage.success('进化提案已提交')
    visible.value = false
    emit('submitted')
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '提交失败'))
  } finally {
    submitting.value = false
  }
}

watch(() => form.evaluationModelType, () => {
  ensureModelSelection()
})

watch(() => [
  form.skillId,
  form.proposedChange,
  form.reason,
  form.evaluationModelType,
  form.evaluationModelId,
], () => {
  invalidateEvaluation()
})

defineExpose({ open })
</script>

<template>
  <el-dialog v-model="visible" title="提交协同进化提案" width="760px" top="6vh">
    <el-form label-position="top" @submit.prevent="doSubmit">
      <el-form-item label="选择团队 Skill">
        <el-select
          v-model="form.skillId"
          placeholder="选择要进化的团队 Skill..."
          filterable
          style="width: 100%"
          :loading="loading"
          @change="onSkillSelect"
        >
          <el-option
            v-for="skill in skills"
            :key="skill.id"
            :label="`${skill.name} (v${skill.version})`"
            :value="skill.id"
          />
        </el-select>
      </el-form-item>

      <div v-if="currentBody" class="version-panel">
        <div class="version-header">
          <span>当前版本：{{ selectedVersionLabel }}</span>
          <el-button size="small" text type="primary" @click="form.proposedChange = currentBody">恢复为当前版本</el-button>
        </div>
        <div class="body-preview">{{ currentBody }}</div>
      </div>

      <el-form-item label="改进后的完整内容 (Markdown)">
        <el-input
          v-model="form.proposedChange"
          type="textarea"
          :rows="10"
          placeholder="选择 Skill 后会自动带入当前内容，请在此基础上修改。"
        />
      </el-form-item>

      <el-form-item label="改进理由">
        <el-input
          v-model="form.reason"
          type="textarea"
          :rows="3"
          placeholder="说明触发场景、改动点和预期收益。"
        />
      </el-form-item>

      <section class="evaluation-panel">
        <div class="evaluation-head">
          <div>
            <div class="evaluation-title">AI 安全与性能评估</div>
            <p>总分 95 分以上才能提交提案。</p>
          </div>
          <el-button type="primary" plain :loading="evaluating" @click="runEvaluation">开始评估</el-button>
        </div>

        <div class="evaluation-grid">
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
      </section>
    </el-form>

    <template #footer>
      <el-button @click="visible = false">取消</el-button>
      <el-button type="primary" :loading="submitting" :disabled="!evaluationResult?.passed" @click="doSubmit">提交提案</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.version-panel {
  margin-bottom: 16px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--panel);
}

.version-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--line);
  font-size: 13px;
  font-weight: 600;
}

.body-preview {
  max-height: 220px;
  overflow-y: auto;
  padding: 12px;
  background: rgba(148, 163, 184, .08);
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
  color: var(--muted);
  font-family: Consolas, monospace;
}

.evaluation-panel {
  margin: 12px 0;
  padding: 14px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--panel);
}

.evaluation-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.evaluation-title {
  font-weight: 600;
  color: var(--text);
}

.evaluation-head p {
  margin: 4px 0 0;
  color: var(--muted);
  font-size: 12px;
}

.evaluation-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
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

.option-meta {
  float: right;
  color: var(--muted);
  font-size: 12px;
}

@media (max-width: 640px) {
  .evaluation-grid,
  .evaluation-result {
    grid-template-columns: 1fr;
  }

  .evaluation-head {
    flex-direction: column;
  }
}
</style>
