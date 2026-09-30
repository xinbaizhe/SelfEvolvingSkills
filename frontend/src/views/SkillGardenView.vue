<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  deleteWorkflowSkill,
  fetchSkillInstallTargets,
  fetchWorkflows,
  installWorkflowSkill,
  type SkillInstallTarget,
  type Workflow,
} from '../api/workflows'
import { emitSkillsChanged } from '../composables/useSkillEvents'
import type { PaginatedResult } from '../api/skills'

const { t } = useI18n()
const gardenItems = ref<Workflow[]>([])
const installTargets = ref<SkillInstallTarget[]>([])
const selectedTargets = ref<Record<number, string>>({})
const loading = ref(false)
const installingId = ref<number | null>(null)
const deletingId = ref<number | null>(null)

const estimatedSavedTotal = computed(() => {
  const hours = gardenItems.value.reduce((total, workflow) => {
    const match = workflow.estimated_time_saved?.match(/[\d.]+/)
    return total + (match ? Number.parseFloat(match[0]) : 0)
  }, 0)
  return `${hours.toFixed(1)}h`
})

function parseSourceAgents(workflow: Workflow): string[] {
  const raw = workflow.source_agents as unknown
  if (Array.isArray(raw)) return raw.map(String)
  if (!raw) return []
  try { return JSON.parse(String(raw)) as string[] } catch { return [] }
}

function getTarget(workflowId: number): string {
  return selectedTargets.value[workflowId] || ''
}

function setTarget(workflowId: number, agentId: string) {
  selectedTargets.value = { ...selectedTargets.value, [workflowId]: agentId }
}

async function load() {
  loading.value = true
  try {
    const [workflowRes, targetRes] = await Promise.all([
      fetchWorkflows(),
      fetchSkillInstallTargets(),
    ])
    const items = Array.isArray(workflowRes.data) ? workflowRes.data : ((workflowRes.data as unknown as PaginatedResult<Workflow>)?.items || [])
    gardenItems.value = items.filter((workflow: Workflow) => workflow.status === 'installed' || workflow.can_generate_skill)
    installTargets.value = targetRes.success && targetRes.data ? targetRes.data : []

    const defaultTarget = installTargets.value.find((target) => target.agent_id === 'claude-code')
      || installTargets.value.find((target) => target.is_enabled)
      || installTargets.value[0]
    if (defaultTarget) {
      const next: Record<number, string> = {}
      for (const workflow of gardenItems.value) {
        // 已安装的 Skill 使用其实际安装目标，未安装的使用默认目标
        if (workflow.installed_agent_id) {
          next[workflow.id] = workflow.installed_agent_id
        } else {
          next[workflow.id] = selectedTargets.value[workflow.id] || defaultTarget.agent_id
        }
      }
      selectedTargets.value = next
    }
  } finally {
    loading.value = false
  }
}

async function handleInstall(workflow: Workflow) {
  const agentId = getTarget(workflow.id)
  if (!agentId) {
    ElMessage.warning(t('workbench.garden.selectTargetFirst'))
    return
  }

  installingId.value = workflow.id
  try {
    const res = await installWorkflowSkill(workflow.id, agentId)
    if (res.success && res.data) {
      workflow.status = 'installed'
      setTarget(workflow.id, res.data.agent_id)
      emitSkillsChanged()
      ElMessage.success(t('workbench.garden.installSuccess', { agent: res.data.agent_name, path: res.data.path }))
    } else {
      ElMessage.error(res.error || t('workbench.garden.installFailed'))
    }
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : t('workbench.garden.installFailed'))
  } finally {
    installingId.value = null
  }
}

async function handleDelete(workflow: Workflow) {
  const isInstalled = workflow.status === 'installed'
  const message = isInstalled
    ? t('workbench.garden.deleteConfirmInstalled', { name: workflow.name })
    : t('workbench.garden.deleteConfirm', { name: workflow.name })

  try {
    await ElMessageBox.confirm(message, t('workbench.common.deleteConfirmTitle'), {
      confirmButtonText: t('common.delete'),
      cancelButtonText: t('common.cancel'),
      type: 'warning',
    })
  } catch {
    return
  }

  deletingId.value = workflow.id
  try {
    const res = await deleteWorkflowSkill(workflow.id, getTarget(workflow.id))
    if (res.success) {
      gardenItems.value = gardenItems.value.filter((w) => w.id !== workflow.id)
      emitSkillsChanged()
      ElMessage.success(t('workbench.garden.deletedByName', { name: workflow.name }))
    } else {
      ElMessage.error(res.error || t('workbench.common.deleteFailed'))
    }
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : t('workbench.common.deleteFailed'))
  } finally {
    deletingId.value = null
  }
}

onMounted(load)
</script>

<template>
  <section class="page-view" v-loading="loading">
    <div class="metrics">
      <article class="metric">
        <label>{{ t('workbench.garden.metricGenerated') }} <span class="chip green">{{ t('workbench.garden.chipLocal') }}</span></label>
        <strong>{{ gardenItems.length }}</strong>
        <small>{{ t('workbench.garden.metricGeneratedHint') }}</small>
      </article>
      <article class="metric">
        <label>{{ t('workbench.garden.metricTargets') }} <span class="chip violet">Agent</span></label>
        <strong>{{ installTargets.length }}</strong>
        <small>{{ t('workbench.garden.metricTargetsHint') }}</small>
      </article>
      <article class="metric">
        <label>{{ t('workbench.garden.metricTotal') }} <span class="chip">{{ t('common.all') }}</span></label>
        <strong>{{ gardenItems.length }}</strong>
        <small>{{ t('workbench.garden.metricTotalHint') }}</small>
      </article>
      <article class="metric">
        <label>{{ t('workbench.garden.metricSaved') }} <span class="chip orange">{{ t('workbench.garden.chipWeekly') }}</span></label>
        <strong>{{ estimatedSavedTotal }}</strong>
        <small>{{ t('workbench.garden.metricSavedHint') }}</small>
      </article>
    </div>

    <el-alert
      v-if="installTargets.length === 0"
      type="warning"
      :closable="false"
      show-icon
      style="margin-bottom: 16px"
      :title="t('workbench.garden.noTargets')"
    />

    <section class="panel">
      <div class="head">
        <div>
          <h2>{{ t('workbench.garden.title') }}</h2>
          <p>{{ t('workbench.garden.subtitle') }}</p>
        </div>
      </div>
      <table v-if="gardenItems.length > 0">
        <thead>
          <tr>
            <th>{{ t('workbench.garden.colName') }}</th>
            <th>{{ t('workbench.garden.colSource') }}</th>
            <th>{{ t('workbench.garden.colUsage') }}</th>
            <th>{{ t('workbench.garden.colStatus') }}</th>
            <th>{{ t('workbench.garden.colTarget') }}</th>
            <th>{{ t('workbench.common.actions') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="workflow in gardenItems" :key="workflow.id">
            <td>
              <b>{{ workflow.name }}</b>
              <span v-if="workflow.evolves_skill" class="chip violet" style="margin-left: 6px; font-size: 11px;">
                {{ t('workbench.garden.upgrade', { n: workflow.iteration_num || '?' }) }}
              </span>
              <small>{{ workflow.description }}</small>
            </td>
            <td>{{ parseSourceAgents(workflow).join(' / ') || t('workbench.common.notDetected') }}</td>
            <td>{{ t('workbench.garden.usage', { n: workflow.frequency, time: workflow.estimated_time_saved }) }}</td>
            <td>
              <span v-if="workflow.evolves_skill" class="chip blue">{{ t('workbench.garden.upgradeCandidate') }}</span>
              <span v-else :class="'chip ' + (workflow.status === 'installed' ? 'green' : workflow.skill_score > 85 ? 'green' : 'orange')">
                {{ workflow.status === 'installed' ? t('workbench.common.statusInstalled') : workflow.skill_score > 85 ? t('workbench.common.statusRecommended') : t('workbench.common.statusPendingReview') }}
              </span>
            </td>
            <td style="min-width: 220px">
              <el-select
                :model-value="getTarget(workflow.id)"
                @update:model-value="(val: string) => setTarget(workflow.id, val)"
                :placeholder="t('workbench.garden.selectAgentPlaceholder')"
                size="small"
                :disabled="workflow.status === 'installed'"
              >
                <el-option
                  v-for="target in installTargets"
                  :key="target.agent_id"
                  :label="`${target.agent_name} · ${target.skills_path}`"
                  :value="target.agent_id"
                />
              </el-select>
            </td>
            <td class="action-cell">
              <el-button
                size="small"
                :loading="installingId === workflow.id"
                :disabled="workflow.status === 'installed' || installTargets.length === 0"
                @click="handleInstall(workflow)"
              >
                {{ workflow.status === 'installed' ? t('workbench.common.statusInstalled') : t('workbench.garden.install') }}
              </el-button>
              <el-button
                size="small"
                type="danger"
                :loading="deletingId === workflow.id"
                @click="handleDelete(workflow)"
              >
                {{ t('common.delete') }}
              </el-button>
            </td>
          </tr>
        </tbody>
      </table>
      <div v-else class="body" style="text-align: center; padding: 40px; color: var(--muted)">
        {{ t('workbench.garden.empty') }}
      </div>
    </section>
  </section>
</template>

<style scoped>
.action-cell {
  display: flex;
  gap: 6px;
  align-items: center;
}
</style>
