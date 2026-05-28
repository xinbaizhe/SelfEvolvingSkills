<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { useTeamStore } from '../stores/useTeamStore'
import { useVulnStore } from '../stores/useVulnStore'
import { getErrorMessage } from '../utils/error'
import LoginDialog from '../components/team/LoginDialog.vue'
import type { VulnScanJob, VulnFinding } from '../api/vuln'

const store = useTeamStore()
const vulnStore = useVulnStore()
const loginDialog = ref<InstanceType<typeof LoginDialog> | null>(null)

const activeTab = ref('url')
const urlInput = ref('')
const dirInput = ref('')
const showHistory = ref(false)

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
  const result = await vulnStore.runUrlScan(urlInput.value.trim())
  if (result) {
    ElMessage.success(`扫描完成，发现 ${result.totalFindings} 个漏洞`)
  } else if (vulnStore.error) {
    ElMessage.error(vulnStore.error)
  }
}

async function handleCodeScan() {
  if (!dirInput.value.trim()) {
    ElMessage.warning('请选择或输入目录路径')
    return
  }
  const result = await vulnStore.runCodeScan(dirInput.value.trim())
  if (result) {
    ElMessage.success(`扫描完成，发现 ${result.totalFindings} 个漏洞`)
  } else if (vulnStore.error) {
    ElMessage.error(vulnStore.error)
  }
}

async function toggleHistory() {
  showHistory.value = !showHistory.value
  if (showHistory.value) {
    await vulnStore.loadHistory()
  }
}

function formatTime(dateStr: string): string {
  if (!dateStr) return '-'
  return dateStr.slice(0, 16).replace('T', ' ')
}

onMounted(() => {
  if (store.isAuthenticated) {
    vulnStore.loadHistory()
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
        <el-button v-if="vulnStore.currentResult" @click="vulnStore.clearResult()">
          新建扫描
        </el-button>
      </div>
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
        <div v-if="vulnStore.history.length === 0" class="empty-state">
          <p>暂无扫描记录</p>
        </div>
        <div v-else class="history-list">
          <article
            v-for="job in vulnStore.history"
            :key="job.id"
            class="history-card"
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
          </article>
        </div>
      </section>
    </template>

    <template v-else>
      <div class="scan-container">
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
            <p class="scan-hint">AI 会自动检测目标网站的 SQL注入、XSS、CSRF、信息泄露、安全响应头缺失等常见漏洞</p>
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
            <p class="scan-hint">AI 会扫描目录下的源代码文件，检测硬编码密钥、SQL注入、XSS、命令注入、路径遍历、不安全加密等漏洞</p>
          </el-tab-pane>
        </el-tabs>
      </div>

      <div v-if="vulnStore.error" class="error-banner">
        <span class="error-icon">!</span>
        {{ vulnStore.error }}
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

.error-banner {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  margin-bottom: 16px;
  background: rgba(214, 79, 79, .08);
  border: 1px solid rgba(214, 79, 79, .28);
  border-radius: 8px;
  color: #d64f4f;
  font-size: 13px;
}

.error-icon {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: #d64f4f;
  color: #fff;
  display: grid;
  place-items: center;
  font-size: 12px;
  font-weight: 700;
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
}

.history-card:hover {
  box-shadow: var(--shadow-hover);
  border-color: rgba(13, 148, 136, .18);
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
