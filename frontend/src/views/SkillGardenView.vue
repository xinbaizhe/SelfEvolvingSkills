<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import {
  fetchSkillInstallTargets,
  fetchWorkflows,
  installWorkflowSkill,
  type SkillInstallTarget,
  type Workflow,
} from '../api/workflows'

const gardenItems = ref<Workflow[]>([])
const installTargets = ref<SkillInstallTarget[]>([])
const selectedTargets = ref<Record<number, string>>({})
const loading = ref(false)
const installingId = ref<number | null>(null)

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

async function load() {
  loading.value = true
  try {
    const [workflowRes, targetRes] = await Promise.all([
      fetchWorkflows(),
      fetchSkillInstallTargets(),
    ])
    const items = Array.isArray(workflowRes.data) ? workflowRes.data : ((workflowRes.data as any)?.items || [])
    gardenItems.value = items.filter((workflow: Workflow) => workflow.status === 'installed' || workflow.can_generate_skill)
    installTargets.value = targetRes.success && targetRes.data ? targetRes.data : []

    const defaultTarget = installTargets.value.find((target) => target.agent_id === 'claude-code')
      || installTargets.value.find((target) => target.is_enabled)
      || installTargets.value[0]
    for (const workflow of gardenItems.value) {
      if (defaultTarget && !selectedTargets.value[workflow.id]) {
        selectedTargets.value[workflow.id] = defaultTarget.agent_id
      }
    }
  } finally {
    loading.value = false
  }
}

async function handleInstall(workflow: Workflow) {
  const agentId = selectedTargets.value[workflow.id]
  if (!agentId) {
    ElMessage.warning('请先选择要安装到的 Agent')
    return
  }

  installingId.value = workflow.id
  try {
    const res = await installWorkflowSkill(workflow.id, agentId)
    if (res.success && res.data) {
      workflow.status = 'installed'
      ElMessage.success(`已安装到 ${res.data.agent_name}: ${res.data.path}`)
    } else {
      ElMessage.error(res.error || '安装失败')
    }
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : '安装失败')
  } finally {
    installingId.value = null
  }
}

onMounted(load)
</script>

<template>
  <section class="page-view" v-loading="loading">
    <div class="metrics">
      <article class="metric">
        <label>已生成技能 <span class="chip green">本地</span></label>
        <strong>{{ gardenItems.length }}</strong>
        <small>管理已安装或待处理的定制技能。</small>
      </article>
      <article class="metric">
        <label>可安装目标 <span class="chip violet">Agent</span></label>
        <strong>{{ installTargets.length }}</strong>
        <small>只显示配置了 skills_path 的 Agent，例如 Claude Code、Codex、Hermes。</small>
      </article>
      <article class="metric">
        <label>本周新增 <span class="chip">近 7 天</span></label>
        <strong>{{ gardenItems.length }}</strong>
        <small>从扫描结果自动生成的技能。</small>
      </article>
      <article class="metric">
        <label>预计节省 <span class="chip orange">每周</span></label>
        <strong>{{ estimatedSavedTotal }}</strong>
        <small>根据重复任务频率和平均持续时间估算。</small>
      </article>
    </div>

    <el-alert
      v-if="installTargets.length === 0"
      type="warning"
      :closable="false"
      show-icon
      style="margin-bottom: 16px"
      title="没有可安装目标。请到系统配置/本地数据源中为至少一个 Agent 配置 skills_path。"
    />

    <section class="panel">
      <div class="head">
        <div>
          <h2>技能花园</h2>
          <p>选择目标 Agent 后安装。不同 Agent 会写入各自的 Skills 目录。</p>
        </div>
      </div>
      <table v-if="gardenItems.length > 0">
        <thead>
          <tr>
            <th>技能名称</th>
            <th>来源 Agent</th>
            <th>使用情况</th>
            <th>状态</th>
            <th>安装目标</th>
            <th>操作</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="workflow in gardenItems" :key="workflow.id">
            <td>
              <b>{{ workflow.name }}</b>
              <small>{{ workflow.description }}</small>
            </td>
            <td>{{ parseSourceAgents(workflow).join(' / ') || '未检测到' }}</td>
            <td>{{ workflow.frequency }} 次 / {{ workflow.estimated_time_saved }}</td>
            <td>
              <span :class="'chip ' + (workflow.status === 'installed' ? 'green' : workflow.skill_score > 85 ? 'green' : 'orange')">
                {{ workflow.status === 'installed' ? '已安装' : workflow.skill_score > 85 ? '推荐安装' : '待审核' }}
              </span>
            </td>
            <td style="min-width: 220px">
              <el-select
                v-model="selectedTargets[workflow.id]"
                placeholder="选择 Agent"
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
            <td>
              <el-button
                size="small"
                :loading="installingId === workflow.id"
                :disabled="workflow.status === 'installed' || installTargets.length === 0"
                @click="handleInstall(workflow)"
              >
                {{ workflow.status === 'installed' ? '已安装' : '安装' }}
              </el-button>
            </td>
          </tr>
        </tbody>
      </table>
      <div v-else class="body" style="text-align: center; padding: 40px; color: var(--muted)">
        暂无已生成技能。请先运行扫描和聚类分析。
      </div>
    </section>
  </section>
</template>
