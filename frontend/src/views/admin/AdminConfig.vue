<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { fetchLlmConfig, fetchScanPaths, fetchSystemInfo, testLlmConnection, updateLlmConfig } from '../../api/admin'

interface ProviderPreset {
  key: string
  label: string
  baseUrl: string
  models: string[]
  defaultModel: string
}

const PROVIDER_PRESETS: ProviderPreset[] = [
  { key: 'openai', label: 'OpenAI', baseUrl: 'https://api.openai.com/v1', models: ['gpt-5.2', 'gpt-5.2-chat-latest', 'gpt-5.2-pro', 'gpt-5.1', 'gpt-4.1', 'o3'], defaultModel: 'gpt-5.2' },
  { key: 'deepseek', label: 'DeepSeek', baseUrl: 'https://api/deepseek.com/v1', models: ['deepseek-v4-pro', 'deepseek-v4-flash', 'deepseek-chat', 'deepseek-reasoner'], defaultModel: 'deepseek-v4-pro' },
  { key: 'siliconflow', label: '硅基流动 (SiliconFlow)', baseUrl: 'https://api.siliconflow.cn/v1', models: ['deepseek-ai/DeepSeek-V3', 'deepseek-ai/DeepSeek-R1', 'Qwen/Qwen3-235B-A22B-Instruct-2507'], defaultModel: 'deepseek-ai/DeepSeek-V3' },
  { key: 'bailian', label: '阿里百炼', baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1', models: ['qwen-plus', 'qwen-max', 'qwen-turbo', 'qwen3-235b-a22b'], defaultModel: 'qwen-plus' },
  { key: 'zhipu', label: '智谱 AI (GLM)', baseUrl: 'https://open.bigmodel.cn/api/paas/v4', models: ['glm-4.6', 'glm-4.5', 'glm-4-plus', 'glm-4-flash'], defaultModel: 'glm-4.6' },
  { key: 'moonshot', label: '月之暗面 (Moonshot)', baseUrl: 'https://api.moonshot.cn/v1', models: ['kimi-k2-0905-preview', 'moonshot-v1-8k', 'moonshot-v1-32k', 'moonshot-v1-128k'], defaultModel: 'kimi-k2-0905-preview' },
  { key: 'custom', label: '自定义兼容接口', baseUrl: '', models: [], defaultModel: '' },
]

const selectedProvider = ref('openai')
const showApiKey = ref(false)
const systemInfo = ref<any>(null)
const scanPaths = ref<any[]>([])
const loading = ref(false)
const saving = ref(false)
const testing = ref(false)

const activePreset = computed(() => PROVIDER_PRESETS.find((p) => p.key === selectedProvider.value))

const modelOptions = computed(() => {
  const preset = activePreset.value
  if (!preset || preset.key === 'custom') return []
  return preset.models.map((model) => ({ label: model, value: model }))
})

const baseUrlOptions = computed(() => {
  const preset = activePreset.value
  if (!preset?.baseUrl) return []
  return [{ label: preset.baseUrl, value: preset.baseUrl }]
})

const form = reactive({
  llm_enabled: false,
  llm_base_url: 'https://api.openai.com/v1',
  llm_api_key: '',
  llm_model: 'gpt-5.2',
  api_key_configured: false,
})

onMounted(load)

function onProviderChange(key: string) {
  const preset = PROVIDER_PRESETS.find((p) => p.key === key)
  if (!preset) return
  form.llm_base_url = preset.baseUrl
  form.llm_model = preset.defaultModel
}

async function load() {
  loading.value = true
  try {
    const [sysRes, llmRes, pathsRes] = await Promise.all([
      fetchSystemInfo(),
      fetchLlmConfig(),
      fetchScanPaths(),
    ])
    if (sysRes.success) systemInfo.value = sysRes.data
    if (pathsRes.success) scanPaths.value = (pathsRes.data as any[]) || []
    if (llmRes.success) applyLlmConfig(llmRes.data)
  } finally {
    loading.value = false
  }
}

function applyLlmConfig(data: any) {
  form.llm_enabled = !!data?.enabled
  const provider = data?.provider || 'openai'
  selectedProvider.value = PROVIDER_PRESETS.some((p) => p.key === provider) ? provider : 'custom'
  form.llm_base_url = data?.base_url || activePreset.value?.baseUrl || 'https://api.openai.com/v1'
  form.llm_api_key = ''
  form.llm_model = data?.model || activePreset.value?.defaultModel || 'gpt-5.2'
  form.api_key_configured = !!(data?.api_key_configured || data?.has_api_key)
}

function llmPayload() {
  return {
    llm_enabled: form.llm_enabled,
    llm_provider: selectedProvider.value,
    llm_base_url: form.llm_base_url.trim(),
    llm_model: form.llm_model.trim(),
    llm_api_key: form.llm_api_key.trim(),
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
      applyLlmConfig(res.data || { ...payload, enabled: payload.llm_enabled, provider: payload.llm_provider, base_url: payload.llm_base_url, model: payload.llm_model, has_api_key: true })
      const sysRes = await fetchSystemInfo()
      if (sysRes.success) systemInfo.value = sysRes.data
      ElMessage.success('大模型配置已保存')
    }
  } finally {
    saving.value = false
  }
}

async function testConnection() {
  if (!validateLlmForm()) return
  testing.value = true
  try {
    const res = await testLlmConnection(llmPayload())
    if (res.success) {
      ElMessage.success(res.data?.message || '连接测试成功')
    } else {
      ElMessage.error(res.error || '连接测试失败')
    }
  } finally {
    testing.value = false
  }
}

function getSourcePaths(source: any): any[] {
  if (!source.paths) return []
  return Object.entries(source.paths)
    .filter(([, value]) => value)
    .map(([type, path]) => ({ type, path }))
}
</script>

<template>
  <div v-loading="loading">
    <h2 style="margin-bottom: 16px">系统配置</h2>

    <el-card header="大模型推荐配置">
      <el-alert type="info" :closable="false" show-icon style="margin-bottom: 16px">
        <template #title>兼容 OpenAI 接口</template>
        基础 URL、API Key 和模型均可自行填写；基础 URL 和模型列表提供快捷选项。
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
          <span class="hint">
            状态：{{ form.llm_enabled && form.api_key_configured ? '大模型可用' : '使用本地回退' }}
          </span>
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
      <p><strong>Self Evolving Skills</strong></p>
      <p>扫描本地 AI Coding Agent 数据，发现重复工作流，生成可审查的 Skill 草稿。</p>
      <p>运行时：{{ systemInfo.runtime }}</p>
      <p>数据库：{{ systemInfo.database }}</p>
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
</style>
