<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { fetchLlmConfig, fetchScanPaths, fetchSystemInfo, testLlmConnection, updateLlmConfig } from '../../api/admin'
import type { LlmConfigData, ScanPathEntry, ScanPathSource, SystemInfo } from '../../types/admin'
import { describeError } from '../../utils/error'

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
const selectedProvider = ref('openai')
const showApiKey = ref(false)
const systemInfo = ref<SystemInfo | null>(null)
const scanPaths = ref<ScanPathSource[]>([])
const loading = ref(false)
const saving = ref(false)
const testing = ref(false)
const lastTestOk = ref<boolean | null>(null)
const lastTestError = ref('')

const activePreset = computed(() => PROVIDER_PRESETS.find((p) => p.key === selectedProvider.value))

const formatUnsupported = computed(() => {
  const preset = activePreset.value
  if (!preset) return false
  if (form.llm_api_format === 'anthropic' && !preset.anthropicBaseUrl) return true
  return false
})

const llmStatusLabel = computed(() => {
  if (lastTestOk.value === true) return t('admin.config.llmAvailable')
  if (lastTestOk.value === false) return t('admin.config.connectionFailed', { error: lastTestError.value || t('admin.config.unknownError') })
  if (form.llm_enabled && form.api_key_configured) return t('admin.config.configuredUntested')
  return t('admin.config.localFallback')
})

const llmStatusType = computed<'' | 'success' | 'danger' | 'warning' | 'info'>(() => {
  if (lastTestOk.value === true) return 'success'
  if (lastTestOk.value === false) return 'danger'
  if (form.llm_enabled && form.api_key_configured) return 'warning'
  return 'info'
})

const modelOptions = computed(() => {
  const preset = activePreset.value
  if (!preset || preset.key === 'custom') return []
  return preset.models.map((model) => ({ label: model, value: model }))
})

const baseUrlOptions = computed(() => {
  const preset = activePreset.value
  if (!preset) return []
  const options: { label: string; value: string }[] = []
  if (preset.baseUrl) options.push({ label: preset.baseUrl, value: preset.baseUrl })
  if (form.llm_api_format === 'anthropic' && preset.anthropicBaseUrl) {
    options.push({ label: preset.anthropicBaseUrl, value: preset.anthropicBaseUrl })
  }
  return options
})

const form = reactive({
  llm_enabled: false,
  llm_base_url: 'https://api.openai.com/v1',
  llm_api_key: '',
  llm_model: 'gpt-5.5',
  llm_api_format: 'openai',
  api_key_configured: false,
})

onMounted(load)

function onProviderChange(key: string) {
  const preset = PROVIDER_PRESETS.find((p) => p.key === key)
  if (!preset) return
  form.llm_api_format = preset.apiFormat || 'openai'
  form.llm_base_url = form.llm_api_format === 'anthropic' && preset.anthropicBaseUrl
    ? preset.anthropicBaseUrl
    : preset.baseUrl
  form.llm_model = preset.defaultModel
  lastTestOk.value = null
  lastTestError.value = ''
}

function onApiFormatChange(format: string) {
  const preset = activePreset.value
  if (!preset) return
  if (format === 'anthropic' && preset.anthropicBaseUrl) {
    form.llm_base_url = preset.anthropicBaseUrl
  } else {
    form.llm_base_url = preset.baseUrl
  }
}

async function load() {
  loading.value = true
  try {
    const [sysRes, llmRes, pathsRes] = await Promise.all([
      fetchSystemInfo(),
      fetchLlmConfig(),
      fetchScanPaths(),
    ])
    if (sysRes.success) systemInfo.value = sysRes.data as SystemInfo
    if (pathsRes.success) scanPaths.value = (pathsRes.data as ScanPathSource[]) || []
    if (llmRes.success) applyLlmConfig(llmRes.data as LlmConfigData)
  } finally {
    loading.value = false
  }
}

function applyLlmConfig(data: LlmConfigData) {
  form.llm_enabled = !!data?.enabled
  const provider = data?.provider || 'openai'
  selectedProvider.value = PROVIDER_PRESETS.some((p) => p.key === provider) ? provider : 'custom'
  form.llm_base_url = data?.base_url || activePreset.value?.baseUrl || 'https://api.openai.com/v1'
  form.llm_api_key = ''
  form.llm_model = data?.model || activePreset.value?.defaultModel || 'gpt-5.2'
  form.llm_api_format = data?.api_format || activePreset.value?.apiFormat || 'openai'
  form.api_key_configured = !!(data?.api_key_configured || data?.has_api_key)
}

function llmPayload() {
  return {
    llm_enabled: form.llm_enabled,
    llm_provider: selectedProvider.value,
    llm_base_url: form.llm_base_url.trim(),
    llm_model: form.llm_model.trim(),
    llm_api_key: form.llm_api_key.trim(),
    llm_api_format: form.llm_api_format,
  }
}

function validateLlmForm() {
  if (!form.llm_base_url.trim()) {
    ElMessage.warning(t('admin.config.fillBaseUrl'))
    return false
  }
  if (!form.llm_model.trim()) {
    ElMessage.warning(t('admin.config.fillModel'))
    return false
  }
  if (!form.llm_api_key.trim() && !form.api_key_configured) {
    ElMessage.warning(t('admin.config.fillApiKey'))
    return false
  }
  return true
}

async function save() {
  if (!validateLlmForm()) return
  saving.value = true
  try {
    const payload = llmPayload()
    const res = await updateLlmConfig(payload)
    if (res.success) {
      applyLlmConfig(res.data || { ...payload, enabled: payload.llm_enabled, provider: payload.llm_provider, base_url: payload.llm_base_url, model: payload.llm_model, api_format: payload.llm_api_format, has_api_key: true })
      const sysRes = await fetchSystemInfo()
      if (sysRes.success) systemInfo.value = sysRes.data as SystemInfo
      ElMessage.success(t('admin.config.saveSuccess'))
    }
  } finally {
    saving.value = false
  }
}

async function testConnection() {
  if (!validateLlmForm()) return
  testing.value = true
  lastTestOk.value = null
  lastTestError.value = ''
  try {
    const res = await testLlmConnection(llmPayload())
    if (res.success) {
      lastTestOk.value = true
      ElMessage.success(describeError(res.data?.message, t('admin.config.testSuccess')))
    } else {
      lastTestOk.value = false
      lastTestError.value = describeError(res.error, t('admin.config.testFailed'))
      ElMessage.error(describeError(res.error, t('admin.config.testFailed')))
    }
  } catch (e: unknown) {
    lastTestOk.value = false
    lastTestError.value = describeError(e, t('admin.config.testError'))
    ElMessage.error(describeError(e, t('admin.config.testError')))
  } finally {
    testing.value = false
  }
}

function getSourcePaths(source: ScanPathSource): ScanPathEntry[] {
  if (!source.paths) return []
  return Object.entries(source.paths)
    .filter(([, value]) => value !== null)
    .map(([type, path]) => ({ type, path: path as string }))
}
</script>

<template>
  <div v-loading="loading">
    <h2 style="margin-bottom: 16px">{{ t('admin.config.title') }}</h2>

    <el-card :header="t('admin.config.llmCard')">
      <el-alert type="info" :closable="false" show-icon style="margin-bottom: 16px">
        <template #title>{{ t('admin.config.llmAlertTitle') }}</template>
        {{ t('admin.config.llmAlertBody') }}
      </el-alert>

      <el-form label-width="120px" style="max-width: 760px">
        <el-form-item :label="t('admin.config.enableLlm')">
          <el-switch v-model="form.llm_enabled" />
        </el-form-item>
        <el-form-item :label="t('admin.config.provider')">
          <el-select
            v-model="selectedProvider"
            :placeholder="t('admin.config.selectProvider')"
            style="width: 100%"
            @change="onProviderChange"
          >
            <el-option
              v-for="provider in PROVIDER_PRESETS"
              :key="provider.key"
              :label="t(provider.labelKey)"
              :value="provider.key"
            />
          </el-select>
        </el-form-item>
        <el-form-item :label="t('admin.common.apiFormat')">
          <el-select v-model="form.llm_api_format" style="width: 100%" @change="onApiFormatChange">
            <el-option :label="t('admin.config.apiFormatOpenai')" value="openai" />
            <el-option :label="t('admin.config.apiFormatAnthropic')" value="anthropic" />
          </el-select>
        </el-form-item>
        <div v-if="formatUnsupported" style="max-width: 760px; margin: 0 0 18px 120px">
          <el-alert type="warning" :closable="false" show-icon>
            <template #title>{{ t('admin.config.formatUnsupported', { provider: activePreset ? t(activePreset.labelKey) : '' }) }}</template>
          </el-alert>
        </div>
        <el-form-item :label="t('admin.config.baseUrl')">
          <el-select
            v-model="form.llm_base_url"
            :placeholder="t('admin.config.baseUrlPlaceholder')"
            style="width: 100%"
            filterable
            allow-create
            default-first-option
          >
            <el-option
              v-for="item in baseUrlOptions"
              :key="item.value"
              :label="item.label"
              :value="item.value"
            />
          </el-select>
        </el-form-item>
        <el-form-item label="API Key">
          <el-input
            v-model="form.llm_api_key"
            :type="showApiKey ? 'text' : 'password'"
            :placeholder="form.api_key_configured ? t('admin.config.apiKeyConfiguredPlaceholder') : t('admin.config.apiKeyPlaceholder')"
          >
            <template #suffix>
              <el-icon
                class="api-key-eye"
                :class="{ visible: showApiKey }"
                @click="showApiKey = !showApiKey"
              >
                <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                  <template v-if="showApiKey">
                    <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"/>
                    <line x1="1" y1="1" x2="23" y2="23"/>
                  </template>
                  <template v-else>
                    <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
                    <circle cx="12" cy="12" r="3"/>
                  </template>
                </svg>
              </el-icon>
            </template>
          </el-input>
        </el-form-item>
        <el-form-item :label="t('admin.common.model')">
          <el-select
            v-model="form.llm_model"
            :placeholder="t('admin.common.modelPlaceholder')"
            style="width: 100%"
            filterable
            allow-create
            default-first-option
          >
            <el-option
              v-for="model in modelOptions"
              :key="model.value"
              :label="model.label"
              :value="model.value"
            />
          </el-select>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" :loading="saving" @click="save">{{ t('admin.config.saveConfig') }}</el-button>
          <el-button :loading="testing" @click="testConnection">{{ t('admin.config.testConnection') }}</el-button>
          <el-tag :type="llmStatusType" size="small">{{ llmStatusLabel }}</el-tag>
        </el-form-item>
      </el-form>
    </el-card>

    <el-card :header="t('admin.config.scanPathsCard')" style="margin-top: 20px">
      <el-alert type="info" :closable="false" show-icon style="margin-bottom: 16px">
        <template #title>{{ t('admin.config.pathSourceTitle') }}</template>
        {{ t('admin.config.pathSourceBody') }}
      </el-alert>

      <el-empty v-if="scanPaths.length === 0" :description="t('admin.config.noSources')" />
      <div v-else class="path-list">
        <section v-for="source in scanPaths" :key="source.agent_id" class="path-source">
          <div class="path-source__head">
            <strong>{{ source.agent_name }}</strong>
            <span :class="'chip ' + (source.is_available ? 'green' : 'orange')">
              {{ source.is_available ? t('admin.common.available') : t('admin.common.unavailable') }}
            </span>
            <span class="hint">{{ t('admin.config.recordCount', { n: source.record_count }) }}</span>
          </div>
          <el-table :data="getSourcePaths(source)" size="small" border :empty-text="t('admin.config.noPaths')">
            <el-table-column prop="type" :label="t('admin.common.type')" width="130" />
            <el-table-column prop="path" :label="t('admin.common.path')" />
          </el-table>
        </section>
      </div>
    </el-card>

    <el-card :header="t('admin.config.about')" style="margin-top: 20px" v-if="systemInfo">
      <div class="about">
        <p class="about-name">Self Evolving Skills</p>
        <p class="about-desc">
          {{ t('admin.config.aboutDesc') }}
        </p>
        <div class="about-stats">
          <div class="stat-item">
            <span class="stat-num">{{ systemInfo.total_skills ?? 0 }}</span>
            <span class="stat-label">Skills</span>
          </div>
          <div class="stat-item">
            <span class="stat-num">{{ systemInfo.total_agents ?? 0 }}</span>
            <span class="stat-label">Agents</span>
          </div>
          <div class="stat-item">
            <span class="stat-num">{{ systemInfo.total_sessions ?? 0 }}</span>
            <span class="stat-label">Sessions</span>
          </div>
        </div>
        <div class="about-tech">
          <el-tag size="small" type="info">Tauri 2</el-tag>
          <el-tag size="small" type="info">Vue 3 + Element Plus</el-tag>
          <el-tag size="small" type="info">Rust Edition 2021</el-tag>
          <el-tag size="small" type="info">SQLite WAL</el-tag>
        </div>
        <p class="about-license">MIT License &mdash; <a href="https://github.com/xinbaizhe/SelfEvolvingSkills" target="_blank">GitHub</a></p>
      </div>
    </el-card>
  </div>
</template>

<style scoped>
.hint {
  color: var(--muted);
  font-size: 13px;
  margin-left: 10px;
}

.path-list {
  display: grid;
  gap: 16px;
}

.path-source {
  border: 1px solid var(--line);
  border-radius: 8px;
  overflow: hidden;
}

.path-source__head {
  padding: 12px 14px;
  display: flex;
  align-items: center;
  gap: 10px;
  background: #fafbfe;
  border-bottom: 1px solid var(--line);
}

.api-key-eye {
  cursor: pointer;
  color: #94a3b8;
  transition: color .15s;
  display: flex;
  align-items: center;
}
.api-key-eye:hover {
  color: #475569;
}
.api-key-eye.visible {
  color: #2563eb;
}

.about-name {
  font-size: 18px;
  font-weight: 700;
  margin: 0 0 10px;
}
.about-desc {
  color: #555;
  font-size: 13px;
  line-height: 1.8;
  margin: 0 0 18px;
}
.about-stats {
  display: flex;
  gap: 32px;
  margin-bottom: 18px;
}
.stat-item {
  display: flex;
  flex-direction: column;
  align-items: center;
}
.stat-num {
  font-size: 24px;
  font-weight: 700;
  color: #0c8265;
}
.stat-label {
  font-size: 12px;
  color: #909399;
  margin-top: 2px;
}
.about-tech {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 14px;
}
.about-license {
  font-size: 12px;
  color: #909399;
  margin: 0;
}
.about-license a {
  color: #1473e6;
  text-decoration: none;
}
</style>
