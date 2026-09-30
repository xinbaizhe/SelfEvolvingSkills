<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { AgentCredential } from '../../api/vuln'

const { t } = useI18n()

const props = withDefaults(defineProps<{
  modelValue: string
  scanning?: boolean
  cookie?: string
  authorization?: string
  headers?: string
  customPaths?: string
  portScanEnabled?: boolean
  portSpec?: string
  scanProfile?: 'quick' | 'standard' | 'deep'
  maxDepth?: number
  maxPages?: number
  useAgent?: boolean
  agentCredentials?: AgentCredential[]
}>(), {
  scanning: false,
  cookie: '',
  authorization: '',
  headers: '',
  customPaths: '',
  portScanEnabled: false,
  portSpec: '',
  scanProfile: 'standard',
  maxDepth: 2,
  maxPages: 24,
  useAgent: false,
  agentCredentials: () => [],
})

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
  (e: 'update:cookie', value: string): void
  (e: 'update:authorization', value: string): void
  (e: 'update:headers', value: string): void
  (e: 'update:customPaths', value: string): void
  (e: 'update:portScanEnabled', value: boolean): void
  (e: 'update:portSpec', value: string): void
  (e: 'update:scanProfile', value: string): void
  (e: 'update:maxDepth', value: number): void
  (e: 'update:maxPages', value: number): void
  (e: 'update:useAgent', value: boolean): void
  (e: 'update:agentCredentials', value: AgentCredential[]): void
  (e: 'scan'): void
  (e: 'addCredential'): void
  (e: 'removeCredential', index: number): void
}>()

const scanProfileOptions = [
  { value: 'quick', labelKey: 'vuln.common.profile.quick.label', titleKey: 'vuln.common.profile.quick.title', descriptionKey: 'vuln.common.profile.quick.description' },
  { value: 'standard', labelKey: 'vuln.common.profile.standard.label', titleKey: 'vuln.common.profile.standard.title', descriptionKey: 'vuln.common.profile.standard.description' },
  { value: 'deep', labelKey: 'vuln.common.profile.deep.label', titleKey: 'vuln.common.profile.deep.title', descriptionKey: 'vuln.common.profile.deep.description' },
] as const

const selectedProfile = computed(() =>
  scanProfileOptions.find(item => item.value === props.scanProfile) || scanProfileOptions[1]
)

function updateCredential(idx: number, field: keyof AgentCredential, value: string) {
  const copy = [...props.agentCredentials!]
  copy[idx] = { ...copy[idx], [field]: value }
  emit('update:agentCredentials', copy)
}
</script>

<template>
  <div class="scan-input-row">
    <el-input
      :model-value="modelValue"
      @update:model-value="emit('update:modelValue', $event)"
      :placeholder="t('vuln.urlForm.urlPlaceholder')"
      size="large"
      clearable
      @keyup.enter="emit('scan')"
    >
      <template #prefix>
        <span class="input-prefix-icon">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none"><circle cx="8" cy="8" r="6" stroke="currentColor" stroke-width="1.2"/><path d="M2 8h12M8 2c1.66 2 1.66 10 0 12M8 2c-1.66 2-1.66 10 0 12" stroke="currentColor" stroke-width="1.2"/></svg>
        </span>
      </template>
    </el-input>
    <el-button type="primary" size="large" :loading="scanning" @click="emit('scan')">
      {{ scanning ? t('vuln.common.scanning') : t('vuln.common.startScan') }}
    </el-button>
  </div>
  <p class="scan-hint">{{ t('vuln.urlForm.hintBasic') }}</p>

  <el-checkbox :model-value="useAgent" @update:model-value="emit('update:useAgent', $event)" class="agent-toggle" style="margin-top:12px">
    {{ t('vuln.urlForm.useAgent') }}
  </el-checkbox>

  <div v-if="useAgent" class="credential-section" style="margin-top:12px">
    <div class="cred-header" style="display:flex;justify-content:space-between;align-items:center;margin-bottom:8px">
      <span style="font-size:14px;font-weight:500">{{ t('vuln.urlForm.credentialsTitle') }}</span>
      <el-button size="small" @click="emit('addCredential')">+ {{ t('vuln.urlForm.addCredential') }}</el-button>
    </div>
    <div v-for="(cred, idx) in agentCredentials" :key="idx" class="cred-row" style="display:flex;gap:8px;margin-bottom:6px;align-items:center">
      <el-input :model-value="cred.username" @update:model-value="updateCredential(idx, 'username', $event)" :placeholder="t('vuln.urlForm.username')" size="small" style="width:100px" />
      <el-input :model-value="cred.role" @update:model-value="updateCredential(idx, 'role', $event)" :placeholder="t('vuln.urlForm.role')" size="small" style="width:120px" />
      <el-input :model-value="cred.cookie" @update:model-value="updateCredential(idx, 'cookie', $event)" placeholder="Cookie" size="small" style="width:160px" />
      <el-input :model-value="cred.authorization" @update:model-value="updateCredential(idx, 'authorization', $event)" placeholder="Authorization" size="small" style="width:160px" />
      <el-button @click="emit('removeCredential', idx)" size="small" type="danger" circle>×</el-button>
    </div>
  </div>

  <div class="url-scan-options">
    <div class="scan-mode-row">
      <div class="scan-mode-select">
        <span class="option-label">{{ t('vuln.urlForm.scanMode') }}</span>
        <el-select :model-value="scanProfile" @update:model-value="emit('update:scanProfile', $event)" style="width: 100%">
          <el-option v-for="option in scanProfileOptions" :key="option.value" :label="t(option.titleKey)" :value="option.value">
            <div class="scan-mode-option"><strong>{{ t(option.titleKey) }}</strong><span>{{ t(option.descriptionKey) }}</span></div>
          </el-option>
        </el-select>
        <p class="scan-mode-desc">{{ t(selectedProfile.descriptionKey) }}</p>
      </div>
      <label class="number-field">
        <span>{{ t('vuln.urlForm.maxDepth') }}</span>
        <el-input-number :model-value="maxDepth" @update:model-value="emit('update:maxDepth', $event)" :min="0" :max="4" size="small" controls-position="right" />
      </label>
      <label class="number-field">
        <span>{{ t('vuln.urlForm.maxPages') }}</span>
        <el-input-number :model-value="maxPages" @update:model-value="emit('update:maxPages', $event)" :min="1" :max="80" size="small" controls-position="right" />
      </label>
    </div>
    <div class="auth-grid">
      <el-input :model-value="cookie" @update:model-value="emit('update:cookie', $event)" type="textarea" :rows="2" :placeholder="t('vuln.urlForm.cookiePlaceholder')" />
      <el-input :model-value="authorization" @update:model-value="emit('update:authorization', $event)" :placeholder="t('vuln.urlForm.authPlaceholder')" clearable />
    </div>
    <div class="auth-grid">
      <el-input :model-value="headers" @update:model-value="emit('update:headers', $event)" type="textarea" :rows="2" :placeholder="t('vuln.urlForm.headersPlaceholder')" />
      <el-input :model-value="customPaths" @update:model-value="emit('update:customPaths', $event)" type="textarea" :rows="2" :placeholder="t('vuln.urlForm.pathsPlaceholder')" />
    </div>
    <div class="port-scan-row">
      <el-checkbox :model-value="portScanEnabled" @update:model-value="emit('update:portScanEnabled', $event)">{{ t('vuln.urlForm.portScan') }}</el-checkbox>
      <el-input :model-value="portSpec" @update:model-value="emit('update:portSpec', $event)" :disabled="!portScanEnabled" :placeholder="t('vuln.urlForm.portPlaceholder')" clearable />
    </div>
  </div>
  <p class="scan-hint">{{ t('vuln.urlForm.hintFull') }}</p>
</template>
