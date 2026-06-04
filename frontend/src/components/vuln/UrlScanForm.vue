<script setup lang="ts">
import { ref, computed } from 'vue'
import type { AgentCredential } from '../../api/vuln'

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
  { value: 'quick', label: '快速', title: '快速扫描', description: '少量页面、常见端口、基础安全头和敏感路径检查，适合先判断目标是否有明显问题。' },
  { value: 'standard', label: '标准', title: '标准扫描', description: '默认模式，爬取更多入口，执行 SQL/XSS/CSRF/SSRF/NoSQL/SSTI/LFI、系统漏洞检测（HTTP方法/CRLF/Host头/默认凭据/源码泄露），6角色 AI 并行复核。' },
  { value: 'deep', label: '深度', title: '深度扫描', description: '更多页面、更全端口集合、更完整敏感路径和全部 payload 变种 + 6角色 AI 并行发现与复核，适合正式排查但耗时更长。' },
] as const

const selectedProfile = computed(() =>
  scanProfileOptions.find(item => item.value === props.scanProfile) || scanProfileOptions[1]
)
</script>

<template>
  <div class="scan-input-row">
    <el-input
      :model-value="modelValue"
      @update:model-value="emit('update:modelValue', $event)"
      placeholder="输入网址，例如 https://example.com"
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
      {{ scanning ? '扫描中...' : '开始扫描' }}
    </el-button>
  </div>
  <p class="scan-hint">系统会爬取同源页面并检测 SQL 注入、XSS、CSRF、信息泄露、安全响应头缺失等常见漏洞。</p>

  <el-checkbox :model-value="useAgent" @update:model-value="emit('update:useAgent', $event)" class="agent-toggle" style="margin-top:12px">
    启用 AI 自主渗透测试 Agent（需配置LLM模型，扫描耗时 30-60 分钟）
  </el-checkbox>

  <div v-if="useAgent" class="credential-section" style="margin-top:12px">
    <div class="cred-header" style="display:flex;justify-content:space-between;align-items:center;margin-bottom:8px">
      <span style="font-size:14px;font-weight:500">多角色凭据（Agent 多角色越权测试用）</span>
      <el-button size="small" @click="emit('addCredential')">+ 添加凭据</el-button>
    </div>
    <div v-for="(cred, idx) in agentCredentials" :key="idx" class="cred-row" style="display:flex;gap:8px;margin-bottom:6px;align-items:center">
      <el-input :model-value="cred.username" @update:model-value="const copy = [...agentCredentials]; copy[idx] = { ...copy[idx], username: $event }; emit('update:agentCredentials', copy)" placeholder="用户名" size="small" style="width:100px" />
      <el-input :model-value="cred.role" @update:model-value="const copy = [...agentCredentials]; copy[idx] = { ...copy[idx], role: $event }; emit('update:agentCredentials', copy)" placeholder="角色(admin/user)" size="small" style="width:120px" />
      <el-input :model-value="cred.cookie" @update:model-value="const copy = [...agentCredentials]; copy[idx] = { ...copy[idx], cookie: $event }; emit('update:agentCredentials', copy)" placeholder="Cookie" size="small" style="width:160px" />
      <el-input :model-value="cred.authorization" @update:model-value="const copy = [...agentCredentials]; copy[idx] = { ...copy[idx], authorization: $event }; emit('update:agentCredentials', copy)" placeholder="Authorization" size="small" style="width:160px" />
      <el-button @click="emit('removeCredential', idx)" size="small" type="danger" circle>×</el-button>
    </div>
  </div>

  <div class="url-scan-options">
    <div class="scan-mode-row">
      <div class="scan-mode-select">
        <span class="option-label">扫描模式</span>
        <el-select :model-value="scanProfile" @update:model-value="emit('update:scanProfile', $event)" style="width: 100%">
          <el-option v-for="option in scanProfileOptions" :key="option.value" :label="option.title" :value="option.value">
            <div class="scan-mode-option"><strong>{{ option.title }}</strong><span>{{ option.description }}</span></div>
          </el-option>
        </el-select>
        <p class="scan-mode-desc">{{ selectedProfile.description }}</p>
      </div>
      <label class="number-field">
        <span>最大深度</span>
        <el-input-number :model-value="maxDepth" @update:model-value="emit('update:maxDepth', $event)" :min="0" :max="4" size="small" controls-position="right" />
      </label>
      <label class="number-field">
        <span>最多页面</span>
        <el-input-number :model-value="maxPages" @update:model-value="emit('update:maxPages', $event)" :min="1" :max="80" size="small" controls-position="right" />
      </label>
    </div>
    <div class="auth-grid">
      <el-input :model-value="cookie" @update:model-value="emit('update:cookie', $event)" type="textarea" :rows="2" placeholder="登录态 Cookie，可选，例如 JSESSIONID=...; token=..." />
      <el-input :model-value="authorization" @update:model-value="emit('update:authorization', $event)" placeholder="Authorization，可选，例如 Bearer eyJ..." clearable />
    </div>
    <div class="auth-grid">
      <el-input :model-value="headers" @update:model-value="emit('update:headers', $event)" type="textarea" :rows="2" placeholder="自定义请求头，每行一个：X-Token: xxx" />
      <el-input :model-value="customPaths" @update:model-value="emit('update:customPaths', $event)" type="textarea" :rows="2" placeholder="自定义敏感路径，每行一个：/actuator/heapdump" />
    </div>
    <div class="port-scan-row">
      <el-checkbox :model-value="portScanEnabled" @update:model-value="emit('update:portScanEnabled', $event)">如果目标是公网 IP，同时扫描服务器开放端口</el-checkbox>
      <el-input :model-value="portSpec" @update:model-value="emit('update:portSpec', $event)" :disabled="!portScanEnabled" placeholder="端口范围，可选：22,80,443,3306 或 1-1024，最多 80 个" clearable />
    </div>
  </div>
  <p class="scan-hint">系统会携带登录态爬取同源页面，执行 SQL/XSS/SSRF/NoSQL/SSTI/LFI 注入检测、HTTP 方法/CRLF/Host头/默认凭据/源码泄露等系统漏洞检测、TLS 证书检查、IP 端口扫描，并调用 AI 大模型 6 角色并行发现与复核。</p>
</template>
