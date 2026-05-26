<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { fetchWorkflows, type Workflow } from '../api/workflows'
import type { PaginatedResult } from '../api/skills'

const workflows = ref<Workflow[]>([])
const loading = ref(false)
const activeFilter = ref<string | null>(null)

const manualCount = computed(() => workflows.value.filter((w) => w.recommendation_source === 'manual-existing-skill' || w.status === 'manual-draft').length)
const pipelineCount = computed(() => workflows.value.filter((w) => w.recommendation_source !== 'manual-existing-skill' && w.recommendation_source !== 'llm').length)
const llmCount = computed(() => workflows.value.filter((w) => w.recommendation_source === 'llm').length)

const filteredWorkflows = computed(() => {
  if (activeFilter.value === 'manual') return workflows.value.filter((w) => w.recommendation_source === 'manual-existing-skill' || w.status === 'manual-draft')
  if (activeFilter.value === 'pipeline') return workflows.value.filter((w) => w.recommendation_source !== 'manual-existing-skill' && w.recommendation_source !== 'llm')
  if (activeFilter.value === 'llm') return workflows.value.filter((w) => w.recommendation_source === 'llm')
  return workflows.value
})

function setFilter(key: string | null) {
  activeFilter.value = activeFilter.value === key ? null : key
}

function parseSourceAgents(workflow: Workflow): string[] {
  const raw = workflow.source_agents as unknown
  if (Array.isArray(raw)) return raw.map(String)
  if (!raw) return []
  try { return JSON.parse(String(raw)) as string[] } catch { return [] }
}

function parseSampleTasks(workflow: Workflow): string[] {
  const raw = workflow.sample_tasks as unknown
  if (Array.isArray(raw)) return raw.map(String)
  if (!raw) return []
  try { return JSON.parse(String(raw)) as string[] } catch { return [] }
}

async function load() {
  loading.value = true
  try {
    const res = await fetchWorkflows()
    workflows.value = Array.isArray(res.data) ? res.data : ((res.data as unknown as PaginatedResult<Workflow>)?.items || [])
  } finally {
    loading.value = false
  }
}

function rankClass(score: number): string {
  if (score >= 88) return 'high'
  if (score >= 78) return 'med'
  return 'low'
}

function sourceLabel(source?: string): string {
  if (source === 'llm') return '大模型判断'
  if (source === 'local-frequency') return '本地频率分析'
  if (source === 'local-prompt-frequency') return '本地提示词分析'
  if (source === 'local-compressed-workflow') return '本地压缩摘要分析'
  return '历史数据分析'
}

const canGenerateWorkflows = computed(() => filteredWorkflows.value.filter((w) => w.can_generate_skill))
const topDrafts = computed(() => canGenerateWorkflows.value.slice(0, 4))

onMounted(load)
</script>

<template>
  <section class="page-view">
    <section class="panel">
      <div class="head">
        <div>
          <h2>推荐技能</h2>
          <p>进化管道运行后，推荐结果会展示在这里。请前往「Skills 工作台」→「进化管道」启动。</p>
        </div>
      </div>

      <div v-if="workflows.length > 0" class="body" v-loading="loading">
        <div class="workflow-summary">
          <span
            :class="'chip green filter-chip' + (activeFilter === 'manual' ? ' active' : '')"
            @click="setFilter('manual')"
          >手动进化 {{ manualCount }}</span>
          <span
            :class="'chip orange filter-chip' + (activeFilter === 'pipeline' ? ' active' : '')"
            @click="setFilter('pipeline')"
          >进化管道推荐 {{ pipelineCount }}</span>
          <span
            :class="'chip violet filter-chip' + (activeFilter === 'llm' ? ' active' : '')"
            @click="setFilter('llm')"
          >大模型复核 {{ llmCount }}</span>
          <span
            v-if="activeFilter"
            class="chip filter-chip"
            @click="setFilter(null)"
          >显示全部 {{ workflows.length }}</span>
        </div>

        <article v-for="(workflow, index) in filteredWorkflows" :key="workflow.id" class="item workflow-item">
          <div :class="'rank ' + rankClass(workflow.skill_score)">{{ index + 1 }}</div>
          <div class="workflow-main">
            <div class="workflow-title">
              <h3>{{ workflow.name }}</h3>
              <span class="chip">{{ sourceLabel(workflow.recommendation_source) }}</span>
            </div>
            <p>{{ workflow.description }}</p>
            <p v-if="workflow.reasoning" class="reasoning">{{ workflow.reasoning }}</p>
            <div class="tag-row" v-if="workflow.source_skills?.length">
              <span v-for="skill in workflow.source_skills.slice(0, 5)" :key="skill" class="chip green">{{ skill }}</span>
            </div>
            <div class="similar" v-if="workflow.similar_skills?.length">
              <b>相似技能</b>
              <a
                v-for="skill in workflow.similar_skills.slice(0, 3)"
                :key="skill.name + skill.source"
                :href="skill.url || undefined"
                target="_blank"
                rel="noreferrer"
              >
                {{ skill.name }}
              </a>
            </div>
          </div>
          <div class="score">
            评分 {{ workflow.skill_score }}
            <span v-if="workflow.confidence != null">置信度 {{ workflow.confidence }}</span>
            <div class="bar"><span :style="{ width: workflow.skill_score + '%' }"></span></div>
          </div>
          <div class="meta">
            {{ workflow.frequency }} 次<br>
            {{ workflow.estimated_time_saved }}
          </div>
        </article>
      </div>

      <div v-else-if="activeFilter" class="body empty-state" v-loading="loading">
        <p>该筛选条件下暂无工作流。</p>
        <el-button link type="primary" @click="setFilter(null)">显示全部</el-button>
      </div>

      <div v-else class="body empty-state" v-loading="loading">
        <p>暂无推荐。</p>
        <p>暂无推荐，请前往「进化管道」启动进化。</p>
        <p>如果进化完成后仍为 0，请到"数据源"确认目标 Agent 已检测到历史会话路径。</p>
      </div>
    </section>

    <section class="two" style="margin-top: 20px" v-if="canGenerateWorkflows.length > 0">
      <section class="panel" v-for="workflow in topDrafts" :key="'draft-' + workflow.id">
        <div class="head">
          <div>
            <h2>{{ workflow.name }}</h2>
            <p>出现 {{ workflow.frequency }} 次 · 预计节省 {{ workflow.estimated_time_saved }}</p>
          </div>
          <span :class="'chip ' + (workflow.skill_score > 85 ? 'green' : 'orange')">
            {{ workflow.skill_score > 85 ? '推荐安装' : '待审核' }}
          </span>
        </div>
        <div class="body">
          <p style="margin: 0; color: var(--muted); font-size: 13px; line-height: 1.6;">
            支持 Agent：{{ parseSourceAgents(workflow).join(' / ') || '未检测到' }}
          </p>
          <div v-if="parseSampleTasks(workflow).length">
            <p style="margin: 8px 0 4px; font-size: 13px; font-weight: 600;">历史证据</p>
            <ul style="margin: 0; padding-left: 18px; color: var(--muted); font-size: 12px;">
              <li v-for="task in parseSampleTasks(workflow)" :key="task">{{ task }}</li>
            </ul>
          </div>
        </div>
      </section>
    </section>
  </section>
</template>

<style scoped>
.workflow-summary {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 12px;
}
.filter-chip {
  cursor: pointer;
  user-select: none;
  padding: 4px 12px;
  transition: transform 0.15s, box-shadow 0.15s;
}
.filter-chip:hover {
  transform: translateY(-1px);
  box-shadow: 0 2px 8px rgba(0,0,0,0.12);
}
.filter-chip.active {
  transform: translateY(-1px);
  box-shadow: 0 0 0 2px currentColor, 0 2px 8px rgba(0,0,0,0.15);
}
.tag-row,
.similar { margin-bottom: 8px; }
.tag-row { display: flex; gap: 6px; flex-wrap: wrap; }
.reasoning { font-size: 12px; color: var(--muted); font-style: italic; }
.similar a { margin-left: 8px; color: var(--blue); font-size: 12px; }
.workflow-title { display: flex; align-items: center; gap: 8px; margin-bottom: 4px; }
.workflow-title h3 { margin: 0; }
.workflow-main { flex: 1; min-width: 0; }
.workflow-item { display: flex; gap: 16px; align-items: flex-start; padding: 16px; border-bottom: 1px solid #f0f0f0; }
.rank {
  width: 32px; height: 32px; border-radius: 8px; display: grid; place-items: center;
  font-weight: 700; font-size: 14px; color: #fff; flex-shrink: 0;
}
.rank.high { background: #0c8265; }
.rank.med { background: #d98612; }
.rank.low { background: #909399; }
.score { text-align: center; font-size: 12px; color: var(--muted); flex-shrink: 0; min-width: 80px; }
.score .bar { height: 4px; background: #f0f0f0; border-radius: 2px; margin-top: 4px; }
.score .bar span { display: block; height: 100%; border-radius: 2px; background: var(--blue); }
.meta { font-size: 12px; color: var(--muted); text-align: right; flex-shrink: 0; }
.empty-state {
  text-align: center;
  padding: 40px;
  color: var(--muted);
  line-height: 1.7;
}
.empty-state p {
  margin: 4px 0;
}
</style>
