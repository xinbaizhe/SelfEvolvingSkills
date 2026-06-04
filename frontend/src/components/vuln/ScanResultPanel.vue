<script setup lang="ts">
import type { VulnScanJob } from '../../api/vuln'

defineProps<{ result: VulnScanJob }>()

function severityType(s: string): string {
  const map: Record<string, string> = { CRITICAL: 'danger', HIGH: 'warning', MEDIUM: '', LOW: 'info' }
  return map[s] || 'info'
}

function severityLabel(s: string): string {
  const map: Record<string, string> = { CRITICAL: '严重', HIGH: '高危', MEDIUM: '中危', LOW: '低危' }
  return map[s] || s
}

function confidenceClass(c: number): string {
  if (c >= 80) return 'conf-high'
  if (c >= 60) return 'conf-mid'
  return 'conf-low'
}

function scanTypeLabel(t: string): string {
  return t === 'url' ? '网址扫描' : '代码扫描'
}
</script>

<template>
  <div class="result-section">
    <div class="result-header">
      <div>
        <h3>扫描详情</h3>
        <p class="result-target">
          <el-tag size="small" :type="result.scanType === 'url' ? 'primary' : 'success'">{{ scanTypeLabel(result.scanType) }}</el-tag>
          {{ result.target }}
        </p>
      </div>
      <div class="result-summary">
        <div class="finding-counts">
          <div class="finding-badge badge-danger"><span class="badge-count">{{ result.criticalCount }}</span><span class="badge-label">严重</span></div>
          <div class="finding-badge badge-warning"><span class="badge-count">{{ result.highCount }}</span><span class="badge-label">高危</span></div>
          <div class="finding-badge"><span class="badge-count">{{ result.mediumCount }}</span><span class="badge-label">中危</span></div>
          <div class="finding-badge badge-info"><span class="badge-count">{{ result.lowCount }}</span><span class="badge-label">低危</span></div>
        </div>
      </div>
    </div>

    <div v-if="result.findings.length === 0" class="empty-state safe-state">
      <p class="safe-text">未发现安全漏洞</p>
    </div>
    <div v-else class="findings-table-wrapper">
      <table class="findings-table">
        <thead>
          <tr>
            <th style="width:72px">严重程度</th>
            <th style="width:110px">类型</th>
            <th style="width:180px">位置</th>
            <th>描述</th>
            <th>修复建议</th>
            <th style="width:72px">置信度</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(f, idx) in result.findings" :key="idx">
            <td><el-tag :type="severityType(f.severity)" size="small" effect="dark">{{ severityLabel(f.severity) }}</el-tag></td>
            <td><span class="finding-type">{{ f.type }}</span></td>
            <td><code class="finding-location">{{ f.location }}</code></td>
            <td class="finding-desc">{{ f.description }}</td>
            <td class="finding-suggestion">{{ f.suggestion }}</td>
            <td>
              <span v-if="f.confidence != null" class="confidence-badge" :class="confidenceClass(f.confidence)">{{ f.confidence }}%</span>
              <span v-else class="confidence-na">-</span>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
