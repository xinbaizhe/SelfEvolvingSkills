<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { fetchSummary, fetchTopSkills, type SummaryStats } from '../api/stats'
import { fetchSources, type SourceConfig } from '../api/scan'
import { fetchWorkflows, type Workflow } from '../api/workflows'
import type { PaginatedResult } from '../api/skills'

interface LogEntry {
  time: string
  title: string
  body: string
}

interface TopSkill {
  name: string
  usage_count: number
}

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

function addLog(title: string, body: string) {
  const now = new Date()
  const stamp = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')} ${String(now.getHours()).padStart(2, '0')}:${String(now.getMinutes()).padStart(2, '0')}:${String(now.getSeconds()).padStart(2, '0')}`
  logs.value.unshift({ time: stamp, title, body })
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

    addLog('刷新完成', `发现 ${availableAgentCount.value} 个本地 Agent，${candidateWorkflows.value.length} 个可生成 Skill 的工作流。`)
  } catch (error) {
    loadError.value = error instanceof Error ? error.message : String(error)
    addLog('加载失败', loadError.value)
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  addLog('启动', 'Self Evolving Skills 已就绪。')
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
        <label>已发现 Agent <span class="chip green">本地</span></label>
        <strong>{{ availableAgentCount }}</strong>
        <small>已启用 {{ enabledAgentCount }} 个数据源，可在系统配置里调整。</small>
      </article>
      <article class="metric">
        <label>历史任务 <span class="chip">本机</span></label>
        <strong>{{ summary?.total_sessions ?? 0 }}</strong>
        <small>扫描本地会话记录，仅保留工作流指纹和摘要信息。</small>
      </article>
      <article class="metric">
        <label>重复工作流 <span class="chip violet">已聚类</span></label>
        <strong>{{ workflows.length }}</strong>
        <small>{{ candidateWorkflows.length }} 个可直接生成 Skill 草稿。</small>
      </article>
      <article class="metric">
        <label>预计节省时间 <span class="chip orange">每周</span></label>
        <strong>{{ estimatedSaved }}</strong>
        <small>根据重复任务频率和平均耗时估算。</small>
      </article>
    </div>

    <div class="grid">
      <section class="panel">
        <div class="head">
          <div>
            <h2>推荐技能</h2>
            <p>点击候选工作流可查看来源任务、草稿、测试样本和导出格式。</p>
          </div>
          <span v-if="candidateWorkflows.length > 0" class="chip green">可审查</span>
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
              <p>{{ workflow.description || '暂无描述' }}</p>
            </div>
            <div class="score">
              评分 {{ workflow.skill_score }}
              <div class="bar"><span :style="{ width: workflow.skill_score + '%' }"></span></div>
            </div>
            <div class="meta">{{ workflow.frequency }} 次<br>{{ workflow.estimated_time_saved || '-' }}</div>
          </article>
          <div v-if="candidateWorkflows.length === 0" class="empty-block">
            暂无候选项。请先在系统管理中运行扫描，再生成工作流聚类。
          </div>
        </div>
      </section>

      <div class="stack">
        <section class="panel">
          <div class="head">
            <div>
              <h2>本地扫描流程</h2>
              <p>自动发现路径、扫描、脱敏、聚类并生成待审查 Skill 草稿。</p>
            </div>
          </div>
          <div class="body">
            <article class="node">
              <div class="node-icon"></div>
              <div><b>发现本地路径</b><span>检测 Hermes、Claude Code、Codex、VSCode、Cursor 等历史目录。</span></div>
              <span class="chip green">就绪</span>
            </article>
            <article class="node">
              <div class="node-icon"></div>
              <div><b>解析历史记录</b><span>提取用户目标、工具调用、输出摘要和成功信号。</span></div>
              <span class="chip green">就绪</span>
            </article>
            <article class="node">
              <div class="node-icon"></div>
              <div><b>本地脱敏处理</b><span>替换 Token、邮箱、客户名称、绝对路径和私有代码片段。</span></div>
              <span class="chip green">默认</span>
            </article>
            <article class="node">
              <div class="node-icon"></div>
              <div><b>工作流聚类</b><span>按目标、输入、输出、工具和重复频率进行聚类。</span></div>
              <span :class="'chip ' + (workflows.length > 0 ? 'green' : 'orange')">
                {{ workflows.length > 0 ? '已完成' : '待处理' }}
              </span>
            </article>
          </div>
        </section>

        <section class="panel">
          <div class="head">
            <div>
              <h2>扫描日志</h2>
              <p>记录本地发现、脱敏和生成操作。</p>
            </div>
          </div>
          <div class="body log">
            <div v-if="logs.length === 0" class="empty-block">暂无日志</div>
            <div v-for="(log, index) in logs" :key="index" class="log-item">
              <time>{{ log.time }}</time>
              <div><b>{{ log.title }}</b>{{ log.body }}</div>
            </div>
          </div>
        </section>
      </div>
    </div>

    <div class="two">
      <section class="panel">
        <div class="head">
          <div>
            <h2>自动发现的本地数据源</h2>
            <p>扫描前需要用户确认。</p>
          </div>
        </div>
        <table>
          <thead>
            <tr><th>数据源</th><th>路径</th><th>记录数</th><th>状态</th></tr>
          </thead>
          <tbody>
            <tr v-for="source in sources" :key="source.agent_id">
              <td><b>{{ source.agent_name }}</b></td>
              <td><small>{{ source.detected_path || '-' }}</small></td>
              <td>{{ source.record_count }}</td>
              <td>
                <span :class="'chip ' + (source.is_enabled ? 'green' : 'orange')">
                  {{ source.is_enabled ? '已启用' : source.is_available ? '已禁用' : '未检测到' }}
                </span>
              </td>
            </tr>
          </tbody>
        </table>
        <div v-if="sources.length === 0" class="empty-block">暂无数据源。请进入系统管理执行扫描。</div>
      </section>

      <section class="panel">
        <div class="head">
          <div>
            <h2>热门 Skills</h2>
            <p>按本地使用频率排序。</p>
          </div>
        </div>
        <div class="body">
          <article v-for="skill in topSkills.slice(0, 5)" :key="skill.name" class="node">
            <div class="node-icon"></div>
            <div><b>{{ skill.name }}</b><span>{{ skill.description || skill.category || '暂无描述' }}</span></div>
            <span class="chip">{{ skill.usage_count ?? 0 }}</span>
          </article>
          <div v-if="topSkills.length === 0" class="empty-block">暂无 Skills。扫描后会在这里显示。</div>
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
