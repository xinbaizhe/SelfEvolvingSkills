<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import type { VulnScanJob } from '../../api/vuln'

defineProps<{ result: VulnScanJob }>()

const { t } = useI18n()

const SEVERITY_LABEL_KEYS: Record<string, string> = {
  CRITICAL: 'vuln.common.severity.critical',
  HIGH: 'vuln.common.severity.high',
  MEDIUM: 'vuln.common.severity.medium',
  LOW: 'vuln.common.severity.low',
}

function severityType(s: string): string {
  const map: Record<string, string> = { CRITICAL: 'danger', HIGH: 'warning', MEDIUM: '', LOW: 'info' }
  return map[s] || 'info'
}

function severityLabel(s: string): string {
  const key = SEVERITY_LABEL_KEYS[s]
  return key ? t(key) : s
}

function confidenceClass(c: number): string {
  if (c >= 80) return 'conf-high'
  if (c >= 60) return 'conf-mid'
  return 'conf-low'
}

function scanTypeLabel(type: string): string {
  return t(type === 'url' ? 'vuln.common.scanType.url' : 'vuln.common.scanType.code')
}
</script>

<template>
  <div class="result-section">
    <div class="result-header">
      <div>
        <h3>{{ t('vuln.common.scanDetail') }}</h3>
        <p class="result-target">
          <el-tag size="small" :type="result.scanType === 'url' ? 'primary' : 'success'">{{ scanTypeLabel(result.scanType) }}</el-tag>
          {{ result.target }}
        </p>
      </div>
      <div class="result-summary">
        <div class="finding-counts">
          <div class="finding-badge badge-danger"><span class="badge-count">{{ result.criticalCount }}</span><span class="badge-label">{{ t('vuln.common.severity.critical') }}</span></div>
          <div class="finding-badge badge-warning"><span class="badge-count">{{ result.highCount }}</span><span class="badge-label">{{ t('vuln.common.severity.high') }}</span></div>
          <div class="finding-badge"><span class="badge-count">{{ result.mediumCount }}</span><span class="badge-label">{{ t('vuln.common.severity.medium') }}</span></div>
          <div class="finding-badge badge-info"><span class="badge-count">{{ result.lowCount }}</span><span class="badge-label">{{ t('vuln.common.severity.low') }}</span></div>
        </div>
      </div>
    </div>

    <div v-if="result.findings.length === 0" class="empty-state safe-state">
      <p class="safe-text">{{ t('vuln.common.noVulnerabilities') }}</p>
    </div>
    <div v-else class="findings-table-wrapper">
      <table class="findings-table">
        <thead>
          <tr>
            <th style="width:72px">{{ t('vuln.common.table.severity') }}</th>
            <th style="width:110px">{{ t('vuln.common.table.type') }}</th>
            <th style="width:180px">{{ t('vuln.common.table.location') }}</th>
            <th>{{ t('vuln.common.table.description') }}</th>
            <th>{{ t('vuln.common.table.suggestion') }}</th>
            <th style="width:72px">{{ t('vuln.common.table.confidence') }}</th>
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
