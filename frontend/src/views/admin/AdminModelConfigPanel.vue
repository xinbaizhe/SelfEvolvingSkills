<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { updateLlmConfig } from '../../api/admin'
import {
  configureClaudeCodeModel,
  destroyPersonalModel,
  fetchAvailableModels,
  fetchMyPersonalModels,
  searchTeamUsers,
  sharePersonalModel,
  type TeamModelConfig,
  type TeamUserItem,
} from '../../api/team'
import LoginDialog from '../../components/team/LoginDialog.vue'
import { useTeamStore } from '../../stores/useTeamStore'
import { getErrorMessage } from '../../utils/error'

interface ProviderPreset {
  key: string
  label: string
  baseUrl: string
  anthropicBaseUrl?: string
  models: string[]
  defaultModel: string
  apiFormat?: string
}

const PROVIDER_PRESETS: ProviderPreset[] = [
  { key: 'openai', label: 'OpenAI', baseUrl: 'https://api.openai.com/v1', models: ['gpt-5.5', 'gpt-5.5-pro', 'gpt-5.4', 'gpt-5.4-pro', 'gpt-5.1', 'o3', 'gpt-4.1'], defaultModel: 'gpt-5.5' },
  { key: 'deepseek', label: 'DeepSeek', baseUrl: 'https://api.deepseek.com', anthropicBaseUrl: 'https://api.deepseek.com/anthropic', models: ['deepseek-v4-pro', 'deepseek-v4-flash', 'deepseek-chat', 'deepseek-reasoner'], defaultModel: 'deepseek-v4-pro' },
  { key: 'bailian', label: '阿里百炼', baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1', models: ['qwen3.7-max', 'qwen3.6-max-preview', 'qwen3.6-plus', 'qwen-plus', 'qwen-max'], defaultModel: 'qwen3.7-max' },
  { key: 'zhipu', label: '智谱 AI (GLM)', baseUrl: 'https://open.bigmodel.cn/api/paas/v4', anthropicBaseUrl: 'https://open.bigmodel.cn/api/anthropic', models: ['glm-5.1', 'glm-5', 'glm-4.7-flash', 'glm-4.6'], defaultModel: 'glm-5.1' },
  { key: 'moonshot', label: '月之暗面 (Moonshot)', baseUrl: 'https://api.moonshot.cn/v1', models: ['kimi-k2.6', 'kimi-k2.5', 'moonshot-v1-128k', 'moonshot-v1-32k'], defaultModel: 'kimi-k2.6' },
  { key: 'minimax', label: 'MiniMax', baseUrl: 'https://api.minimaxi.com/v1', anthropicBaseUrl: 'https://api.minimaxi.com/anthropic', models: ['MiniMax-M2.7'], defaultModel: 'MiniMax-M2.7', apiFormat: 'anthropic' },
  { key: 'custom', label: '自定义兼容接口', baseUrl: '', models: [], defaultModel: '' },
]

const store = useTeamStore()
const loginDialog = ref<InstanceType<typeof LoginDialog> | null>(null)
const loading = ref(false)
const recordsLoading = ref(false)
const sharing = ref(false)
const userSearching = ref(false)
const destroyingId = ref<number | null>(null)
const applyingId = ref<number | null>(null)
const configDialogVisible = ref(false)
const searchName = ref('')
const configs = ref<TeamModelConfig[]>([])
const personalRecords = ref<TeamModelConfig[]>([])
const selectedModel = ref<TeamModelConfig | null>(null)
const configTarget = ref<'system' | 'claude'>('system')
const userKeyword = ref('')
const userResults = ref<TeamUserItem[]>([])
const selectedRecipient = ref<TeamUserItem | null>(null)
const shareApiFormat = ref('openai')

const shareForm = reactive({
  name: '',
  provider: 'openai',
  baseUrl: 'https://api.openai.com/v1',
  model: 'gpt-5.5',
  apiKey: '',
})

const activeSharePreset = computed(() => PROVIDER_PRESETS.find((p) => p.key === shareForm.provider))

const shareModelOptions = computed(() => {
  const preset = activeSharePreset.value
  if (!preset || preset.key === 'custom') return []
  return preset.models.map((model) => ({ label: model, value: model }))
})

const shareBaseUrlOptions = computed(() => {
  const preset = activeSharePreset.value
  if (!preset) return []
  const options: { label: string; value: string }[] = []
  if (preset.baseUrl) options.push({ label: preset.baseUrl, value: preset.baseUrl })
  if (shareApiFormat.value === 'anthropic' && preset.anthropicBaseUrl) {
    options.push({ label: preset.anthropicBaseUrl, value: preset.anthropicBaseUrl })
  }
  return options
})

async function loadConfigs() {
  if (!store.isAuthenticated) return
  loading.value = true
  try {
    configs.value = await fetchAvailableModels(searchName.value)
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '加载团队模型列表失败'))
  } finally {
    loading.value = false
  }
}

async function loadPersonalRecords() {
  if (!store.isAuthenticated) return
  recordsLoading.value = true
  try {
    personalRecords.value = await fetchMyPersonalModels()
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '加载转赠记录失败'))
  } finally {
    recordsLoading.value = false
  }
}

async function loadAll() {
  await Promise.all([loadConfigs(), loadPersonalRecords()])
}

function onShareProviderChange(key: string) {
  const preset = PROVIDER_PRESETS.find((p) => p.key === key)
  if (!preset) return
  shareApiFormat.value = preset.apiFormat || 'openai'
  shareForm.baseUrl = shareApiFormat.value === 'anthropic' && preset.anthropicBaseUrl
    ? preset.anthropicBaseUrl
    : preset.baseUrl
  shareForm.model = preset.defaultModel
}

function onShareApiFormatChange(format: string) {
  const preset = activeSharePreset.value
  if (!preset) return
  if (format === 'anthropic' && preset.anthropicBaseUrl) {
    shareForm.baseUrl = preset.anthropicBaseUrl
  } else {
    shareForm.baseUrl = preset.baseUrl
  }
}

function resetShareForm() {
  shareForm.name = ''
  shareForm.provider = 'openai'
  shareForm.baseUrl = 'https://api.openai.com/v1'
  shareForm.model = 'gpt-5.5'
  shareForm.apiKey = ''
  shareApiFormat.value = 'openai'
  userKeyword.value = ''
  userResults.value = []
  selectedRecipient.value = null
}

async function submitPersonalModel() {
  if (!selectedRecipient.value) {
    ElMessage.warning('请先查询并选择接收用户')
    return
  }
  if (!shareForm.name.trim() || !shareForm.baseUrl.trim() || !shareForm.model.trim() || !shareForm.apiKey.trim()) {
    ElMessage.warning('请填写名称、Base URL、模型和 API Key')
    return
  }
  sharing.value = true
  try {
    await sharePersonalModel({
      name: shareForm.name.trim(),
      provider: shareForm.provider.trim() || 'custom',
      baseUrl: shareForm.baseUrl.trim(),
      model: shareForm.model.trim(),
      apiKeyHash: shareForm.apiKey.trim(),
      recipientId: selectedRecipient.value.userId,
    })
    ElMessage.success('已转赠个人 API 模型')
    resetShareForm()
    await loadAll()
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '转赠失败'))
  } finally {
    sharing.value = false
  }
}

async function queryUsers() {
  if (!userKeyword.value.trim()) {
    ElMessage.warning('请输入用户名称')
    return
  }
  userSearching.value = true
  selectedRecipient.value = null
  try {
    userResults.value = await searchTeamUsers(userKeyword.value)
    if (userResults.value.length === 0) {
      ElMessage.warning('未查询到用户')
    } else if (userResults.value.length === 1) {
      selectedRecipient.value = userResults.value[0]
    }
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '查询用户失败'))
  } finally {
    userSearching.value = false
  }
}

function selectRecipient(user: TeamUserItem) {
  selectedRecipient.value = user
}

async function destroyRecord(row: TeamModelConfig) {
  try {
    await ElMessageBox.confirm(`确定销毁转赠记录「${row.name}」？销毁后接收人将无法再通过团队模型一键配置该 API。`, '销毁转赠记录', {
      type: 'warning',
      confirmButtonText: '销毁',
      cancelButtonText: '取消',
    })
  } catch {
    return
  }
  destroyingId.value = row.id
  try {
    await destroyPersonalModel(row.id)
    ElMessage.success('转赠记录已销毁')
    await loadAll()
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '销毁失败'))
  } finally {
    destroyingId.value = null
  }
}

function openApplyDialog(row: TeamModelConfig) {
  if (!row.baseUrl || !row.model) {
    ElMessage.warning('该模型缺少 Base URL 或模型名')
    return
  }
  if (!row.apiKeyHash) {
    ElMessage.warning('该模型未包含可用 API Key，无法一键配置')
    return
  }
  selectedModel.value = row
  configTarget.value = 'system'
  configDialogVisible.value = true
}

async function applyToSystemConfig(row: TeamModelConfig) {
  const res = await updateLlmConfig({
    llm_enabled: true,
    llm_provider: row.provider || 'custom',
    llm_base_url: row.baseUrl,
    llm_model: row.model,
    llm_api_key: row.apiKeyHash,
    llm_api_format: row.provider === 'anthropic' ? 'anthropic' : 'openai',
  })
  if (!res.success) {
    throw new Error(res.error || '配置失败')
  }
}

async function applyToClaudeCode(row: TeamModelConfig) {
  await configureClaudeCodeModel({
    provider: row.provider || 'custom',
    baseUrl: row.baseUrl,
    model: row.model,
    apiKey: row.apiKeyHash || '',
  })
}

async function confirmApplyModel() {
  const row = selectedModel.value
  if (!row) return
  applyingId.value = row.id
  try {
    if (configTarget.value === 'system') {
      await applyToSystemConfig(row)
      ElMessage.success(`已写入系统管理资源配置 LLM：${row.name}`)
    } else {
      await applyToClaudeCode(row)
      ElMessage.success(`已写入 Claude Code：${row.name}`)
    }
    configDialogVisible.value = false
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '配置失败'))
  } finally {
    applyingId.value = null
  }
}

onMounted(loadAll)
</script>

<template>
  <div class="model-config-panel">
    <div class="panel-header">
      <div>
        <h3>团队模型</h3>
        <p>登录团队版后查看部门模型和个人转赠模型，并可一键写入本地大模型配置。</p>
      </div>
      <el-button size="small" :loading="loading || recordsLoading" @click="loadAll">刷新</el-button>
    </div>

    <template v-if="!store.isAuthenticated">
      <div class="login-prompt">
        <div class="login-card">
          <h3>团队模型</h3>
          <p>登录团队版后可查看部门模型和个人转赠 API 模型。</p>
          <el-button type="primary" @click="loginDialog?.open()">登录团队版</el-button>
        </div>
      </div>
    </template>

    <template v-else>
      <div class="toolbar">
        <el-input
          v-model="searchName"
          clearable
          placeholder="按名称查询部门或个人转赠模型"
          @keyup.enter="loadConfigs"
          @clear="loadConfigs"
        />
        <el-button type="primary" :loading="loading" @click="loadConfigs">查询</el-button>
      </div>

      <el-table :data="configs" v-loading="loading" size="small" style="width: 100%">
        <el-table-column prop="name" label="名称" width="150" />
        <el-table-column label="来源" width="110">
          <template #default="{ row }">
            <el-tag :type="row.sourceType === 'personal' ? 'warning' : 'success'" size="small">
              {{ row.sourceType === 'personal' ? '个人转赠' : '部门' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="provider" label="提供商" width="120" />
        <el-table-column prop="model" label="模型" min-width="180" />
        <el-table-column prop="baseUrl" label="Base URL" min-width="220">
          <template #default="{ row }">
            <span class="mono">{{ row.baseUrl || '-' }}</span>
          </template>
        </el-table-column>
        <el-table-column label="部门名称" width="140">
          <template #default="{ row }">
            {{ row.deptName || (row.sourceType === 'personal' ? '个人转赠' : '-') }}
          </template>
        </el-table-column>
        <el-table-column label="转赠人" width="120">
          <template #default="{ row }">{{ row.createdByName || '-' }}</template>
        </el-table-column>
        <el-table-column label="接收人" width="120">
          <template #default="{ row }">{{ row.recipientName || '-' }}</template>
        </el-table-column>
        <el-table-column label="操作" width="120" fixed="right">
          <template #default="{ row }">
            <el-button size="small" type="primary" :loading="applyingId === row.id" @click="openApplyDialog(row)">
              一键配置
            </el-button>
          </template>
        </el-table-column>
      </el-table>

      <div v-if="!loading && configs.length === 0" class="empty-hint">
        暂无可用模型。可联系管理员新增部门模型，或在下方转赠个人 API 模型。
      </div>

      <div class="share-card">
        <div class="share-head">
          <h3>个人转赠 API 模型</h3>
          <p>把自己的 Base URL 和 API Key 转赠给别人；其他人可按名称查询并一键配置。</p>
        </div>
        <el-form label-width="90px" class="share-form">
          <el-row :gutter="12">
            <el-col :xs="24">
              <el-form-item label="接收用户">
                <div class="user-search">
                  <el-input
                    v-model="userKeyword"
                    placeholder="输入手机号或昵称模糊查询"
                    clearable
                    @keyup.enter="queryUsers"
                    @clear="selectedRecipient = null"
                  />
                  <el-button :loading="userSearching" @click="queryUsers">查询用户</el-button>
                </div>
                <div v-if="userResults.length > 0" class="user-results">
                  <div class="user-search-hint">
                    仅显示前 10 条，结果过多时请输入更精确的手机号或昵称。
                  </div>
                  <button
                    v-for="user in userResults"
                    :key="user.userId"
                    type="button"
                    class="user-result"
                    :class="{ selected: selectedRecipient?.userId === user.userId }"
                    @click="selectRecipient(user)"
                  >
                    <div class="user-main">
                      <strong>手机号：{{ user.userName }}</strong>
                      <span>昵称：{{ user.nickName || '未设置昵称' }}</span>
                    </div>
                    <small>{{ user.deptName || '无部门' }}</small>
                  </button>
                </div>
                <div v-if="selectedRecipient" class="selected-user">
                  已选择：{{ selectedRecipient.userName }}（{{ selectedRecipient.deptName || '无部门' }}）
                </div>
              </el-form-item>
            </el-col>
            <el-col :xs="24" :md="12">
              <el-form-item label="名称">
                <el-input v-model="shareForm.name" placeholder="例如 张三的 DeepSeek" maxlength="100" />
              </el-form-item>
            </el-col>
            <el-col :xs="24" :md="12">
              <el-form-item label="提供商">
                <el-select
                  v-model="shareForm.provider"
                  filterable
                  allow-create
                  default-first-option
                  style="width: 100%"
                  @change="onShareProviderChange"
                >
                  <el-option
                    v-for="provider in PROVIDER_PRESETS"
                    :key="provider.key"
                    :label="provider.label"
                    :value="provider.key"
                  />
                </el-select>
              </el-form-item>
            </el-col>
            <el-col :xs="24" :md="12">
              <el-form-item label="API 格式">
                <el-select v-model="shareApiFormat" style="width: 100%" @change="onShareApiFormatChange">
                  <el-option label="OpenAI 兼容" value="openai" />
                  <el-option label="Anthropic" value="anthropic" />
                </el-select>
              </el-form-item>
            </el-col>
            <el-col :xs="24" :md="12">
              <el-form-item label="Base URL">
                <el-select
                  v-model="shareForm.baseUrl"
                  placeholder="选择或输入 Base URL"
                  style="width: 100%"
                  filterable
                  allow-create
                  default-first-option
                >
                  <el-option
                    v-for="item in shareBaseUrlOptions"
                    :key="item.value"
                    :label="item.label"
                    :value="item.value"
                  />
                </el-select>
              </el-form-item>
            </el-col>
            <el-col :xs="24" :md="12">
              <el-form-item label="模型">
                <el-select
                  v-model="shareForm.model"
                  placeholder="选择或输入模型"
                  style="width: 100%"
                  filterable
                  allow-create
                  default-first-option
                >
                  <el-option
                    v-for="model in shareModelOptions"
                    :key="model.value"
                    :label="model.label"
                    :value="model.value"
                  />
                </el-select>
              </el-form-item>
            </el-col>
            <el-col :xs="24">
              <el-form-item label="API Key">
                <el-input v-model="shareForm.apiKey" type="password" show-password placeholder="转赠后其他人可用于一键配置" />
              </el-form-item>
            </el-col>
          </el-row>
          <div class="share-actions">
            <el-button @click="resetShareForm">清空</el-button>
            <el-button type="primary" :loading="sharing" :disabled="!selectedRecipient" @click="submitPersonalModel">确认转赠</el-button>
          </div>
        </el-form>

        <div class="record-section">
          <div class="record-head">
            <h4>转赠记录</h4>
            <el-button size="small" text :loading="recordsLoading" @click="loadPersonalRecords">刷新记录</el-button>
          </div>
          <el-table :data="personalRecords" v-loading="recordsLoading" size="small" class="record-table">
            <el-table-column prop="name" label="名称" min-width="140" />
            <el-table-column prop="provider" label="提供商" width="110" />
            <el-table-column prop="model" label="模型" min-width="150" />
            <el-table-column prop="baseUrl" label="Base URL" min-width="220">
              <template #default="{ row }">
                <span class="mono">{{ row.baseUrl || '-' }}</span>
              </template>
            </el-table-column>
            <el-table-column label="接收人" width="140">
              <template #default="{ row }">{{ row.recipientName || '-' }}</template>
            </el-table-column>
            <el-table-column label="状态" width="80">
              <template #default="{ row }">
                <el-tag :type="row.isActive ? 'success' : 'info'" size="small">
                  {{ row.isActive ? '有效' : '停用' }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="操作" width="100" fixed="right">
              <template #default="{ row }">
                <el-button size="small" type="danger" text :loading="destroyingId === row.id" @click="destroyRecord(row)">
                  销毁
                </el-button>
              </template>
            </el-table-column>
          </el-table>
          <div v-if="!recordsLoading && personalRecords.length === 0" class="record-empty">
            暂无转赠记录。
          </div>
        </div>
      </div>
    </template>

    <el-dialog v-model="configDialogVisible" title="选择配置目标" width="520px" top="16vh">
      <div v-if="selectedModel" class="config-target-dialog">
        <div class="config-model-summary">
          <strong>{{ selectedModel.name }}</strong>
          <span>{{ selectedModel.provider || 'custom' }} / {{ selectedModel.model }}</span>
          <small>{{ selectedModel.baseUrl }}</small>
        </div>

        <el-radio-group v-model="configTarget" class="config-targets">
          <el-radio value="system" border>
            <div class="target-option">
              <strong>系统管理资源配置 LLM</strong>
              <span>写入本应用的资源配置，后续资源扫描、模型调用使用该配置。</span>
            </div>
          </el-radio>
          <el-radio value="claude" border>
            <div class="target-option">
              <strong>Claude Code</strong>
              <span>写入用户目录下的 .claude/settings.json，供 Claude Code 使用。</span>
            </div>
          </el-radio>
        </el-radio-group>
      </div>
      <template #footer>
        <el-button @click="configDialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="applyingId !== null" @click="confirmApplyModel">确认配置</el-button>
      </template>
    </el-dialog>

    <LoginDialog ref="loginDialog" @logged-in="loadAll" />
  </div>
</template>

<style scoped>
.model-config-panel {
  padding: 0;
}

.panel-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 14px;
  margin-bottom: 14px;
}

.panel-header h3,
.share-head h3,
.login-card h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}

.panel-header p,
.share-head p,
.login-card p {
  margin: 6px 0 0;
  color: var(--muted);
  font-size: 13px;
}

.toolbar {
  display: grid;
  grid-template-columns: minmax(220px, 360px) auto;
  gap: 10px;
  margin-bottom: 14px;
  justify-content: start;
}

.mono {
  font-family: Consolas, monospace;
  font-size: 12px;
  color: var(--muted);
}

.empty-hint {
  padding: 32px 0;
  text-align: center;
  color: var(--muted);
  font-size: 13px;
}

.config-target-dialog {
  display: grid;
  gap: 14px;
}

.config-model-summary {
  display: grid;
  gap: 6px;
  padding: 12px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: #f8fafc;
}

.config-model-summary strong {
  color: var(--ink);
  font-size: 14px;
}

.config-model-summary span,
.config-model-summary small {
  color: var(--muted);
  font-size: 12px;
  word-break: break-all;
}

.config-targets {
  display: grid;
  gap: 10px;
}

.config-targets :deep(.el-radio) {
  align-items: flex-start;
  height: auto;
  margin-right: 0;
  padding: 12px;
  white-space: normal;
}

.config-targets :deep(.el-radio__label) {
  min-width: 0;
}

.target-option {
  display: grid;
  gap: 5px;
  line-height: 1.45;
}

.target-option strong {
  color: var(--ink);
}

.target-option span {
  color: var(--muted);
  font-size: 12px;
}

.share-card {
  margin-top: 18px;
  padding: 16px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: #fff;
}

.share-head {
  margin-bottom: 14px;
}

.share-form :deep(.el-form-item) {
  margin-bottom: 12px;
}

.share-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.record-section {
  margin-top: 18px;
  padding-top: 16px;
  border-top: 1px solid var(--line);
}

.record-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
}

.record-head h4 {
  margin: 0;
  color: var(--ink);
  font-size: 14px;
  font-weight: 600;
}

.record-table {
  width: 100%;
}

.record-empty {
  padding: 20px 0 6px;
  color: var(--muted);
  font-size: 13px;
  text-align: center;
}

.user-search {
  display: grid;
  grid-template-columns: minmax(220px, 1fr) auto;
  gap: 10px;
  width: 100%;
}

.user-results {
  display: grid;
  gap: 8px;
  margin-top: 10px;
  width: 100%;
}

.user-result {
  display: flex;
  gap: 14px;
  align-items: center;
  width: 100%;
  padding: 12px 14px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: linear-gradient(180deg, #ffffff, #fbfdff);
  color: var(--ink);
  text-align: left;
  transition: border-color .15s, background .15s, box-shadow .15s;
}

.user-result:hover {
  border-color: rgba(13, 148, 136, 0.32);
  box-shadow: 0 8px 22px rgba(15, 23, 42, 0.06);
}

.user-result.selected {
  border-color: #0d9488;
  background: #f0fdfa;
}

.user-main {
  display: flex;
  align-items: center;
  gap: 18px;
  min-width: 0;
  flex: 1;
}

.user-main strong,
.user-main span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.user-main strong {
  color: #0f172a;
  font-size: 13px;
}

.user-search-hint,
.user-main span,
.user-result small,
.selected-user {
  color: var(--muted);
  font-size: 12px;
}

.user-result small {
  flex: 0 0 140px;
  text-align: right;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.user-search-hint {
  line-height: 1.5;
}

.selected-user {
  margin-top: 8px;
}

.login-prompt {
  display: grid;
  place-items: center;
  min-height: 200px;
}

.login-card {
  text-align: center;
  padding: 36px;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  background: var(--panel);
}

.login-card p {
  margin-bottom: 18px;
}

@media (max-width: 900px) {
  .toolbar {
    grid-template-columns: 1fr;
  }

  .user-search {
    grid-template-columns: 1fr;
  }

  .user-result,
  .user-main {
    align-items: flex-start;
    flex-direction: column;
  }

  .user-result small {
    flex: 0 0 auto;
    text-align: left;
  }
}
</style>
