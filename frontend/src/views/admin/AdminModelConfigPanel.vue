<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { useI18n } from 'vue-i18n'
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
import { failureCode, getErrorMessage } from '../../utils/error'

interface ProviderPreset {
  key: string
  labelKey: string
  baseUrl: string
  anthropicBaseUrl?: string
  models: string[]
  defaultModel: string
  apiFormat?: string
}

const PROVIDER_PRESETS: ProviderPreset[] = [
  { key: 'openai', labelKey: 'admin.common.providerPreset.openai', baseUrl: 'https://api.openai.com/v1', models: ['gpt-5.5', 'gpt-5.5-pro', 'gpt-5.4', 'gpt-5.4-pro', 'gpt-5.1', 'o3', 'gpt-4.1'], defaultModel: 'gpt-5.5' },
  { key: 'deepseek', labelKey: 'admin.common.providerPreset.deepseek', baseUrl: 'https://api.deepseek.com', anthropicBaseUrl: 'https://api.deepseek.com/anthropic', models: ['deepseek-v4-pro', 'deepseek-v4-flash', 'deepseek-chat', 'deepseek-reasoner'], defaultModel: 'deepseek-v4-pro' },
  { key: 'bailian', labelKey: 'admin.common.providerPreset.bailian', baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1', models: ['qwen3.7-max', 'qwen3.6-max-preview', 'qwen3.6-plus', 'qwen-plus', 'qwen-max'], defaultModel: 'qwen3.7-max' },
  { key: 'zhipu', labelKey: 'admin.common.providerPreset.zhipu', baseUrl: 'https://open.bigmodel.cn/api/paas/v4', anthropicBaseUrl: 'https://open.bigmodel.cn/api/anthropic', models: ['glm-5.1', 'glm-5', 'glm-4.7-flash', 'glm-4.6'], defaultModel: 'glm-5.1' },
  { key: 'moonshot', labelKey: 'admin.common.providerPreset.moonshot', baseUrl: 'https://api.moonshot.cn/v1', models: ['kimi-k2.6', 'kimi-k2.5', 'moonshot-v1-128k', 'moonshot-v1-32k'], defaultModel: 'kimi-k2.6' },
  { key: 'minimax', labelKey: 'admin.common.providerPreset.minimax', baseUrl: 'https://api.minimaxi.com/v1', anthropicBaseUrl: 'https://api.minimaxi.com/anthropic', models: ['MiniMax-M2.7'], defaultModel: 'MiniMax-M2.7', apiFormat: 'anthropic' },
  { key: 'custom', labelKey: 'admin.common.providerPreset.custom', baseUrl: '', models: [], defaultModel: '' },
]

const { t } = useI18n()
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
const offline = ref(false)
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
    offline.value = false
  } catch (e: unknown) {
    if (isConnectionError(e)) {
      offline.value = true
    } else {
      ElMessage.error(getErrorMessage(e, t('admin.modelConfig.loadTeamModelsFailed')))
    }
  } finally {
    loading.value = false
  }
}

async function loadPersonalRecords() {
  if (!store.isAuthenticated) return
  recordsLoading.value = true
  try {
    personalRecords.value = await fetchMyPersonalModels()
    offline.value = false
  } catch (e: unknown) {
    if (isConnectionError(e)) {
      offline.value = true
    } else {
      ElMessage.error(getErrorMessage(e, t('admin.modelConfig.loadGiftRecordsFailed')))
    }
  } finally {
    recordsLoading.value = false
  }
}

async function loadAll() {
  await Promise.all([loadConfigs(), loadPersonalRecords()])
}

/**
 * Transport failures the team API raises, as catalog codes. Keying on the code
 * means a translated envelope still reads as a connection problem; the English
 * substrings below cover reqwest's own text, which no catalog reaches.
 */
const CONNECTION_ERROR_CODES = new Set([
  'team.connectFailed',
  'team.loginRequestFailed',
  'team.apiRequestFailed',
  'team.refreshFailed',
  'team.downloadRequestFailed',
])

function isConnectionError(e: unknown): boolean {
  if (CONNECTION_ERROR_CODES.has(failureCode(e) ?? '')) return true
  const msg = e instanceof Error ? e.message : String(e)
  return msg.includes('error sending request for url')
    || msg.includes('ConnectError')
    || msg.includes('connection refused')
    || msg.includes('NetworkError')
    || msg.includes('fetch failed')
    || msg.includes('Failed to fetch')
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
    ElMessage.warning(t('admin.modelConfig.selectRecipientFirst'))
    return
  }
  if (!shareForm.name.trim() || !shareForm.baseUrl.trim() || !shareForm.model.trim() || !shareForm.apiKey.trim()) {
    ElMessage.warning(t('admin.modelConfig.fillFields'))
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
    ElMessage.success(t('admin.modelConfig.giftSuccess'))
    resetShareForm()
    await loadAll()
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, t('admin.modelConfig.giftFailed')))
  } finally {
    sharing.value = false
  }
}

async function queryUsers() {
  if (!userKeyword.value.trim()) {
    ElMessage.warning(t('admin.modelConfig.enterUserName'))
    return
  }
  userSearching.value = true
  selectedRecipient.value = null
  try {
    userResults.value = await searchTeamUsers(userKeyword.value)
    if (userResults.value.length === 0) {
      ElMessage.warning(t('admin.modelConfig.noUserFound'))
    } else if (userResults.value.length === 1) {
      selectedRecipient.value = userResults.value[0]
    }
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, t('admin.modelConfig.searchUserFailed')))
  } finally {
    userSearching.value = false
  }
}

function selectRecipient(user: TeamUserItem) {
  selectedRecipient.value = user
}

async function destroyRecord(row: TeamModelConfig) {
  try {
    await ElMessageBox.confirm(t('admin.modelConfig.destroyConfirm', { name: row.name }), t('admin.modelConfig.destroyTitle'), {
      type: 'warning',
      confirmButtonText: t('admin.modelConfig.destroy'),
      cancelButtonText: t('common.cancel'),
    })
  } catch {
    return
  }
  destroyingId.value = row.id
  try {
    await destroyPersonalModel(row.id)
    ElMessage.success(t('admin.modelConfig.destroySuccess'))
    await loadAll()
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, t('admin.modelConfig.destroyFailed')))
  } finally {
    destroyingId.value = null
  }
}

function openApplyDialog(row: TeamModelConfig) {
  if (!row.baseUrl || !row.model) {
    ElMessage.warning(t('admin.modelConfig.missingBaseUrlOrModel'))
    return
  }
  if (!row.apiKeyHash) {
    ElMessage.warning(t('admin.modelConfig.missingApiKey'))
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
    throw new Error(res.error || t('admin.modelConfig.configFailed'))
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
      ElMessage.success(t('admin.modelConfig.appliedToSystem', { name: row.name }))
    } else {
      await applyToClaudeCode(row)
      ElMessage.success(t('admin.modelConfig.appliedToClaude', { name: row.name }))
    }
    configDialogVisible.value = false
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, t('admin.modelConfig.configFailed')))
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
        <h3>{{ t('admin.modelConfig.title') }}</h3>
        <p>{{ t('admin.modelConfig.subtitle') }}</p>
      </div>
      <el-button size="small" :loading="loading || recordsLoading" @click="loadAll">{{ t('common.refresh') }}</el-button>
    </div>

    <div v-if="offline" class="offline-banner">
      <span class="offline-icon">
        <svg width="18" height="18" viewBox="0 0 18 18" fill="none"><path d="M9 1.5C4.86 1.5 1.5 4.86 1.5 9s3.36 7.5 7.5 7.5 7.5-3.36 7.5-7.5S13.14 1.5 9 1.5zM9 6v4M9 12h.01" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>
      </span>
      {{ t('admin.modelConfig.offline') }}
      <el-button size="small" @click="loadAll">{{ t('common.retry') }}</el-button>
    </div>

    <template v-if="!store.isAuthenticated">
      <div class="login-prompt">
        <div class="login-card">
          <h3>{{ t('admin.modelConfig.title') }}</h3>
          <p>{{ t('admin.modelConfig.loginPrompt') }}</p>
          <el-button type="primary" @click="loginDialog?.open()">{{ t('app.actions.login') }}</el-button>
        </div>
      </div>
    </template>

    <template v-else>
      <div class="toolbar">
        <el-input
          v-model="searchName"
          clearable
          :placeholder="t('admin.modelConfig.searchPlaceholder')"
          @keyup.enter="loadConfigs"
          @clear="loadConfigs"
        />
        <el-button type="primary" :loading="loading" @click="loadConfigs">{{ t('common.search') }}</el-button>
      </div>

      <el-table :data="configs" v-loading="loading" size="small" style="width: 100%">
        <el-table-column prop="name" :label="t('admin.common.name')" width="150" />
        <el-table-column :label="t('admin.common.source')" width="110">
          <template #default="{ row }">
            <el-tag :type="row.sourceType === 'personal' ? 'warning' : 'success'" size="small">
              {{ row.sourceType === 'personal' ? t('admin.modelConfig.personal') : t('admin.modelConfig.department') }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="provider" :label="t('admin.common.provider')" width="120" />
        <el-table-column prop="model" :label="t('admin.common.model')" min-width="180" />
        <el-table-column prop="baseUrl" label="Base URL" min-width="220">
          <template #default="{ row }">
            <span class="mono">{{ row.baseUrl || '-' }}</span>
          </template>
        </el-table-column>
        <el-table-column :label="t('admin.modelConfig.colDeptName')" width="140">
          <template #default="{ row }">
            {{ row.deptName || (row.sourceType === 'personal' ? t('admin.modelConfig.personal') : '-') }}
          </template>
        </el-table-column>
        <el-table-column :label="t('admin.modelConfig.colGifter')" width="120">
          <template #default="{ row }">{{ row.createdByName || '-' }}</template>
        </el-table-column>
        <el-table-column :label="t('admin.modelConfig.colRecipient')" width="120">
          <template #default="{ row }">{{ row.recipientName || '-' }}</template>
        </el-table-column>
        <el-table-column :label="t('admin.common.action')" width="120" fixed="right">
          <template #default="{ row }">
            <el-button size="small" type="primary" :loading="applyingId === row.id" @click="openApplyDialog(row)">
              {{ t('admin.modelConfig.oneClick') }}
            </el-button>
          </template>
        </el-table-column>
      </el-table>

      <div v-if="!loading && configs.length === 0" class="empty-hint">
        {{ t('admin.modelConfig.emptyHint') }}
      </div>

      <div class="share-card">
        <div class="share-head">
          <h3>{{ t('admin.modelConfig.giftTitle') }}</h3>
          <p>{{ t('admin.modelConfig.giftSubtitle') }}</p>
        </div>
        <el-form label-width="90px" class="share-form">
          <el-row :gutter="12">
            <el-col :xs="24">
              <el-form-item :label="t('admin.modelConfig.recipientUser')">
                <div class="user-search">
                  <el-input
                    v-model="userKeyword"
                    :placeholder="t('admin.modelConfig.userSearchPlaceholder')"
                    clearable
                    @keyup.enter="queryUsers"
                    @clear="selectedRecipient = null"
                  />
                  <el-button :loading="userSearching" @click="queryUsers">{{ t('admin.modelConfig.searchUsers') }}</el-button>
                </div>
                <div v-if="userResults.length > 0" class="user-results">
                  <div class="user-search-hint">
                    {{ t('admin.modelConfig.userSearchHint') }}
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
                      <strong>{{ t('admin.modelConfig.phone', { phone: user.userName }) }}</strong>
                      <span>{{ t('admin.modelConfig.nickname', { name: user.nickName || t('admin.modelConfig.noNickname') }) }}</span>
                    </div>
                    <small>{{ user.deptName || t('admin.modelConfig.noDept') }}</small>
                  </button>
                </div>
                <div v-if="selectedRecipient" class="selected-user">
                  {{ t('admin.modelConfig.selected', { name: selectedRecipient.userName, dept: selectedRecipient.deptName || t('admin.modelConfig.noDept') }) }}
                </div>
              </el-form-item>
            </el-col>
            <el-col :xs="24" :md="12">
              <el-form-item :label="t('admin.common.name')">
                <el-input v-model="shareForm.name" :placeholder="t('admin.modelConfig.namePlaceholder')" maxlength="100" />
              </el-form-item>
            </el-col>
            <el-col :xs="24" :md="12">
              <el-form-item :label="t('admin.common.provider')">
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
                    :label="t(provider.labelKey)"
                    :value="provider.key"
                  />
                </el-select>
              </el-form-item>
            </el-col>
            <el-col :xs="24" :md="12">
              <el-form-item :label="t('admin.common.apiFormat')">
                <el-select v-model="shareApiFormat" style="width: 100%" @change="onShareApiFormatChange">
                  <el-option :label="t('admin.modelConfig.apiFormatOpenai')" value="openai" />
                  <el-option label="Anthropic" value="anthropic" />
                </el-select>
              </el-form-item>
            </el-col>
            <el-col :xs="24" :md="12">
              <el-form-item label="Base URL">
                <el-select
                  v-model="shareForm.baseUrl"
                  :placeholder="t('admin.modelConfig.baseUrlPlaceholder')"
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
              <el-form-item :label="t('admin.common.model')">
                <el-select
                  v-model="shareForm.model"
                  :placeholder="t('admin.common.modelPlaceholder')"
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
                <el-input v-model="shareForm.apiKey" type="password" show-password :placeholder="t('admin.modelConfig.apiKeyPlaceholder')" />
              </el-form-item>
            </el-col>
          </el-row>
          <div class="share-actions">
            <el-button @click="resetShareForm">{{ t('admin.modelConfig.clear') }}</el-button>
            <el-button type="primary" :loading="sharing" :disabled="!selectedRecipient" @click="submitPersonalModel">{{ t('admin.modelConfig.confirmGift') }}</el-button>
          </div>
        </el-form>

        <div class="record-section">
          <div class="record-head">
            <h4>{{ t('admin.modelConfig.recordsTitle') }}</h4>
            <el-button size="small" text :loading="recordsLoading" @click="loadPersonalRecords">{{ t('admin.modelConfig.refreshRecords') }}</el-button>
          </div>
          <el-table :data="personalRecords" v-loading="recordsLoading" size="small" class="record-table">
            <el-table-column prop="name" :label="t('admin.common.name')" min-width="140" />
            <el-table-column prop="provider" :label="t('admin.common.provider')" width="110" />
            <el-table-column prop="model" :label="t('admin.common.model')" min-width="150" />
            <el-table-column prop="baseUrl" label="Base URL" min-width="220">
              <template #default="{ row }">
                <span class="mono">{{ row.baseUrl || '-' }}</span>
              </template>
            </el-table-column>
            <el-table-column :label="t('admin.modelConfig.colRecipient')" width="140">
              <template #default="{ row }">{{ row.recipientName || '-' }}</template>
            </el-table-column>
            <el-table-column :label="t('admin.common.statusLabel')" width="80">
              <template #default="{ row }">
                <el-tag :type="row.isActive ? 'success' : 'info'" size="small">
                  {{ row.isActive ? t('admin.modelConfig.active') : t('admin.modelConfig.inactive') }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column :label="t('admin.common.action')" width="100" fixed="right">
              <template #default="{ row }">
                <el-button size="small" type="danger" text :loading="destroyingId === row.id" @click="destroyRecord(row)">
                  {{ t('admin.modelConfig.destroy') }}
                </el-button>
              </template>
            </el-table-column>
          </el-table>
          <div v-if="!recordsLoading && personalRecords.length === 0" class="record-empty">
            {{ t('admin.modelConfig.noRecords') }}
          </div>
        </div>
      </div>
    </template>

    <el-dialog v-model="configDialogVisible" :title="t('admin.modelConfig.selectTarget')" width="520px" top="16vh">
      <div v-if="selectedModel" class="config-target-dialog">
        <div class="config-model-summary">
          <strong>{{ selectedModel.name }}</strong>
          <span>{{ selectedModel.provider || 'custom' }} / {{ selectedModel.model }}</span>
          <small>{{ selectedModel.baseUrl }}</small>
        </div>

        <el-radio-group v-model="configTarget" class="config-targets">
          <el-radio value="system" border>
            <div class="target-option">
              <strong>{{ t('admin.modelConfig.targetSystem') }}</strong>
              <span>{{ t('admin.modelConfig.targetSystemDesc') }}</span>
            </div>
          </el-radio>
          <el-radio value="claude" border>
            <div class="target-option">
              <strong>Claude Code</strong>
              <span>{{ t('admin.modelConfig.targetClaudeDesc') }}</span>
            </div>
          </el-radio>
        </el-radio-group>
      </div>
      <template #footer>
        <el-button @click="configDialogVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="applyingId !== null" @click="confirmApplyModel">{{ t('admin.modelConfig.confirmConfig') }}</el-button>
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
