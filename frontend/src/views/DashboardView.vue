<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { fetchSummary, fetchTopSkills, type SummaryStats } from '../api/stats'
import { fetchSources, type SourceConfig } from '../api/scan'
import { fetchWorkflows, type Workflow } from '../api/workflows'
import type { PaginatedResult } from '../api/skills'

interface LogEntry {
  time: string
  titleKey: string
  bodyKey?: string
  bodyParams?: Record<string, unknown>
  bodyText?: string
}

interface TopSkill {
  name: string
  usage_count: number
  description?: string
  category?: string
}

const { t } = useI18n()
const summary = ref<SummaryStats | null>(null)
const sources = ref<SourceConfig[]>([])
const workflows = ref<Workflow[]>([])
const topSkills = ref<TopSkill[]>([])
const logs = ref<LogEntry[]>([])
const loading = ref(false)
const loadError = ref('')

const availableAgentCount = computed(() => sources.value.filter((source) => source.is_available).length)
const enabledAgentCount = computed(() => sources.value.filter((source) => source.is_enabled).length)
const candidateWorkflows = computed(() => workflows.value.filter((workflow) => workflow.can_generate_skill))
const estimatedSaved = computed(() => {
  const hours = workflows.value.reduce((total, w) => {
    const match = w.estimated_time_saved?.match(/[\d.]+/)
    return total + (match ? Number.parseFloat(match[0]) : 0)
  }, 0)
  return `${hours.toFixed(1)}h`
})

function addLog(titleKey: string, body: { key: string; params?: Record<string, unknown> } | { text: string }) {
  const now = new Date()
  const stamp = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')} ${String(now.getHours()).padStart(2, '0')}:${String(now.getMinutes()).padStart(2, '0')}:${String(now.getSeconds()).padStart(2, '0')}`
  const entry: LogEntry = { time: stamp, titleKey }
  if ('key' in body) {
    entry.bodyKey = body.key
    entry.bodyParams = body.params
  } else {
    entry.bodyText = body.text
  }
  logs.value.unshift(entry)
  if (logs.value.length > 20) logs.value.pop()
}

async function load() {
  loading.value = true
  loadError.value = ''
  try {
    const [summaryRes, sourceRes, workflowRes, skillsRes] = await Promise.all([
      fetchSummary(),
      fetchSources(),
      fetchWorkflows(),
      fetchTopSkills(20),
    ])

    if (summaryRes.success) summary.value = summaryRes.data
    if (sourceRes.success && sourceRes.data) sources.value = sourceRes.data
    if (workflowRes.success && workflowRes.data) {
      workflows.value = Array.isArray(workflowRes.data) ? workflowRes.data : ((workflowRes.data as PaginatedResult<Workflow>)?.items || [])
    }
    if (skillsRes.success && skillsRes.data) topSkills.value = skillsRes.data as TopSkill[]

    addLog('admin.overview.logRefreshDone', { key: 'admin.overview.logRefreshBody', params: { agents: availableAgentCount.value, workflows: candidateWorkflows.value.length } })
  } catch (error) {
    loadError.value = error instanceof Error ? error.message : String(error)
    addLog('admin.overview.logLoadFailed', { text: loadError.value })
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  addLog('admin.overview.logStartup', { key: 'admin.overview.logStartupBody' })
  load()
})
</script>

<template>
  <section class="page-view" v-loading="loading">
    <el-alert
      v-if="loadError"
      type="error"
      :closable="false"
      show-icon
      style="margin-bottom: 16px"
      :title="loadError"
    />

    <div class="metrics">
      <article class="metric">
        <label>{{ t('admin.overview.discoveredAgents') }} <span class="chip green">{{ t('admin.overview.local') }}</span></label>
        <strong>{{ availableAgentCount }}</strong>
        <small>{{ t('admin.overview.enabledSourcesHint', { n: enabledAgentCount }) }}</small>
      </article>
      <article class="metric">
        <label>{{ t('admin.overview.historyTasks') }} <span class="chip">{{ t('admin.overview.thisMachine') }}</span></label>
        <strong>{{ summary?.total_sessions ?? 0 }}</strong>
        <small>{{ t('admin.overview.historyTasksHint') }}</small>
      </article>
      <article class="metric">
        <label>{{ t('admin.overview.repeatedWorkflows') }} <span class="chip violet">{{ t('admin.overview.clustered') }}</span></label>
        <strong>{{ workflows.length }}</strong>
        <small>{{ t('admin.overview.candidatesHint', { n: candidateWorkflows.length }) }}</small>
      </article>
      <article class="metric">
        <label>{{ t('admin.overview.estimatedSaved') }} <span class="chip orange">{{ t('admin.overview.weekly') }}</span></label>
        <strong>{{ estimatedSaved }}</strong>
        <small>{{ t('admin.overview.estimatedSavedHint') }}</small>
      </article>
    </div>

    <div class="grid">
      <section class="panel">
        <div class="head">
          <div>
            <h2>{{ t('admin.overview.recommendedSkills') }}</h2>
            <p>{{ t('admin.overview.recommendedSkillsHint') }}</p>
          </div>
          <span v-if="candidateWorkflows.length > 0" class="chip green">{{ t('admin.overview.reviewable') }}</span>
        </div>
        <div class="body">
          <article
            v-for="(workflow, index) in candidateWorkflows.slice(0, 5)"
            :key="workflow.id"
            :class="'item' + (index === 0 ? ' active' : '')"
          >
            <div :class="'rank ' + (workflow.skill_score >= 88 ? 'high' : workflow.skill_score >= 78 ? 'med' : 'low')">
              {{ index + 1 }}
            </div>
            <div>
              <h3>{{ workflow.name }}</h3>
              <p>{{ workflow.description || t('admin.common.noDescription') }}</p>
            </div>
            <div class="score">
              {{ t('admin.overview.score', { score: workflow.skill_score }) }}
              <div class="bar"><span :style="{ width: workflow.skill_score + '%' }"></span></div>
            </div>
            <div class="meta">{{ t('admin.overview.times', { n: workflow.frequency }) }}<br>{{ workflow.estimated_time_saved || '-' }}</div>
          </article>
          <div v-if="candidateWorkflows.length === 0" class="empty-block">
            {{ t('admin.overview.noCandidates') }}
          </div>
        </div>
      </section>

      <div class="stack">
        <section class="panel">
          <div class="head">
            <div>
              <h2>{{ t('admin.overview.localScanFlow') }}</h2>
              <p>{{ t('admin.overview.localScanFlowHint') }}</p>
            </div>
          </div>
          <div class="body">
            <article class="node">
              <div class="node-icon"></div>
              <div><b>{{ t('admin.overview.discoverPaths') }}</b><span>{{ t('admin.overview.discoverPathsHint') }}</span></div>
              <span class="chip green">{{ t('admin.overview.ready') }}</span>
            </article>
            <article class="node">
              <div class="node-icon"></div>
              <div><b>{{ t('admin.overview.parseHistory') }}</b><span>{{ t('admin.overview.parseHistoryHint') }}</span></div>
              <span class="chip green">{{ t('admin.overview.ready') }}</span>
            </article>
            <article class="node">
              <div class="node-icon"></div>
              <div><b>{{ t('admin.overview.localRedaction') }}</b><span>{{ t('admin.overview.localRedactionHint') }}</span></div>
              <span class="chip green">{{ t('admin.overview.default') }}</span>
            </article>
            <article class="node">
              <div class="node-icon"></div>
              <div><b>{{ t('admin.overview.workflowClustering') }}</b><span>{{ t('admin.overview.workflowClusteringHint') }}</span></div>
              <span :class="'chip ' + (workflows.length > 0 ? 'green' : 'orange')">
                {{ workflows.length > 0 ? t('admin.common.status.completed') : t('admin.common.status.pending') }}
              </span>
            </article>
          </div>
        </section>

        <section class="panel">
          <div class="head">
            <div>
              <h2>{{ t('admin.overview.scanLog') }}</h2>
              <p>{{ t('admin.overview.scanLogHint') }}</p>
            </div>
          </div>
          <div class="body log">
            <div v-if="logs.length === 0" class="empty-block">{{ t('admin.overview.noLogs') }}</div>
            <div v-for="(log, index) in logs" :key="index" class="log-item">
              <time>{{ log.time }}</time>
              <div><b>{{ t(log.titleKey) }}</b>{{ log.bodyKey ? t(log.bodyKey, log.bodyParams || {}) : log.bodyText }}</div>
            </div>
          </div>
        </section>
      </div>
    </div>

    <div class="two">
      <section class="panel">
        <div class="head">
          <div>
            <h2>{{ t('admin.overview.autoSources') }}</h2>
            <p>{{ t('admin.overview.autoSourcesHint') }}</p>
          </div>
        </div>
        <table>
          <thead>
            <tr><th>{{ t('admin.overview.thSource') }}</th><th>{{ t('admin.common.path') }}</th><th>{{ t('admin.common.records') }}</th><th>{{ t('admin.common.statusLabel') }}</th></tr>
          </thead>
          <tbody>
            <tr v-for="source in sources" :key="source.agent_id">
              <td><b>{{ source.agent_name }}</b></td>
              <td><small>{{ source.detected_path || '-' }}</small></td>
              <td>{{ source.record_count }}</td>
              <td>
                <span :class="'chip ' + (source.is_enabled ? 'green' : 'orange')">
                  {{ source.is_enabled ? t('admin.common.enabled') : source.is_available ? t('admin.overview.sourceDisabled') : t('admin.common.unavailable') }}
                </span>
              </td>
            </tr>
          </tbody>
        </table>
        <div v-if="sources.length === 0" class="empty-block">{{ t('admin.overview.noSources') }}</div>
      </section>

      <section class="panel">
        <div class="head">
          <div>
            <h2>{{ t('admin.overview.hotSkills') }}</h2>
            <p>{{ t('admin.overview.hotSkillsHint') }}</p>
          </div>
        </div>
        <div class="body">
          <article v-for="skill in topSkills.slice(0, 5)" :key="skill.name" class="node">
            <div class="node-icon"></div>
            <div><b>{{ skill.name }}</b><span>{{ skill.description || skill.category || t('admin.common.noDescription') }}</span></div>
            <span class="chip">{{ skill.usage_count ?? 0 }}</span>
          </article>
          <div v-if="topSkills.length === 0" class="empty-block">{{ t('admin.overview.noSkills') }}</div>
        </div>
      </section>
    </div>
  </section>
</template>

<style scoped>
.empty-block {
  color: var(--muted);
  padding: 20px;
  text-align: center;
}
</style>
