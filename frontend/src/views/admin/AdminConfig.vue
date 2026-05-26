<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { fetchLlmConfig, fetchScanPaths, fetchSystemInfo, testLlmConnection, updateLlmConfig } from '../../api/admin'
import type { LlmConfigData, ScanPathEntry, ScanPathSource, SystemInfo } from '../../types/admin'

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
  if (lastTestOk.value === true) return '大模型可用'
  if (lastTestOk.value === false) return `连接失败：${lastTestError.value || '未知错误'}`
  if (form.llm_enabled && form.api_key_configured) return '已配置，未测试连接'
  return '使用本地回退'
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
    ElMessage.warning('请填写基础 URL')
    return false
  }
  if (!form.llm_model.trim()) {
    ElMessage.warning('请选择或填写模型')
    return false
  }
  if (!form.llm_api_key.trim() && !form.api_key_configured) {
    ElMessage.warning('请填写 API Key')
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
      ElMessage.success('大模型配置已保存')
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
      ElMessage.success(res.data?.message || '连接测试成功')
    } else {
      lastTestOk.value = false
      lastTestError.value = res.error || '连接测试失败'
      ElMessage.error(res.error || '连接测试失败')
    }
  } catch (e: unknown) {
    lastTestOk.value = false
    lastTestError.value = e instanceof Error ? e.message : '连接测试异常'
    ElMessage.error(e instanceof Error ? e.message : '连接测试异常')
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
    <h2 style="margin-bottom: 16px">系统配置</h2>

    <el-card header="大模型推荐配置">
      <el-alert type="info" :closable="false" show-icon style="margin-bottom: 16px">
        <template #title>兼容 OpenAI / Anthropic 接口</template>
        基础 URL、API Key 和模型均可自行填写；支持 OpenAI 和 Anthropic Messages 两种 API 格式。
      </el-alert>

      <el-form label-width="120px" style="max-width: 760px">
        <el-form-item label="启用大模型">
          <el-switch v-model="form.llm_enabled" />
        </el-form-item>
        <el-form-item label="服务商">
          <el-select
            v-model="selectedProvider"
            placeholder="选择服务商"
            style="width: 100%"
            @change="onProviderChange"
          >
            <el-option
              v-for="provider in PROVIDER_PRESETS"
              :key="provider.key"
              :label="provider.label"
              :value="provider.key"
            />
          </el-select>
        </el-form-item>
        <el-form-item label="API 格式">
          <el-select v-model="form.llm_api_format" style="width: 100%" @change="onApiFormatChange">
            <el-option label="OpenAI 兼容 (Chat Completions)" value="openai" />
            <el-option label="Anthropic (Messages API)" value="anthropic" />
          </el-select>
        </el-form-item>
        <div v-if="formatUnsupported" style="max-width: 760px; margin: 0 0 18px 120px">
          <el-alert type="warning" :closable="false" show-icon>
            <template #title>{{ activePreset?.label }} 暂不支持 Anthropic Messages API，建议切换为 OpenAI 兼容格式</template>
          </el-alert>
        </div>
        <el-form-item label="基础 URL">
          <el-select
            v-model="form.llm_base_url"
            placeholder="选择或输入基础 URL"
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
            :placeholder="form.api_key_configured ? '已配置，留空则保持不变' : '请输入 API Key'"
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
        <el-form-item label="模型">
          <el-select
            v-model="form.llm_model"
            placeholder="选择或输入模型"
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
          <el-button type="primary" :loading="saving" @click="save">保存配置</el-button>
          <el-button :loading="testing" @click="testConnection">测试连接</el-button>
          <el-tag :type="llmStatusType" size="small">{{ llmStatusLabel }}</el-tag>
        </el-form-item>
      </el-form>
    </el-card>

    <el-card header="扫描路径配置" style="margin-top: 20px">
      <el-alert type="info" :closable="false" show-icon style="margin-bottom: 16px">
        <template #title>路径来源</template>
        显示已启用本地 Agent 数据源的实际扫描路径。可在“本地数据源”页面启用或禁用 Agent。
      </el-alert>

      <el-empty v-if="scanPaths.length === 0" description="未启用任何 Agent 数据源" />
      <div v-else class="path-list">
        <section v-for="source in scanPaths" :key="source.agent_id" class="path-source">
          <div class="path-source__head">
            <strong>{{ source.agent_name }}</strong>
            <span :class="'chip ' + (source.is_available ? 'green' : 'orange')">
              {{ source.is_available ? '可用' : '未检测到' }}
            </span>
            <span class="hint">{{ source.record_count }} 条记录</span>
          </div>
          <el-table :data="getSourcePaths(source)" size="small" border empty-text="无路径">
            <el-table-column prop="type" label="类型" width="130" />
            <el-table-column prop="path" label="路径" />
          </el-table>
        </section>
      </div>
    </el-card>

    <el-card header="关于" style="margin-top: 20px" v-if="systemInfo">
      <div class="about">
        <p class="about-name">Self Evolving Skills</p>
        <p class="about-desc">
          本地优先的 AI Coding Agent Skills 进化引擎。扫描本机已安装的 AI 编程工具（Claude Code、Codex、Cursor、VSCode 等），
          收集已有的 Skills、Agents 与会话历史，通过 7 步进化管道自动发现高频重复工作流，生成可安装的 Skill 草稿。
          所有数据默认留存在本机，不上传任何内容。
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
