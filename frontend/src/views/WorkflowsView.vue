<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { startEvolution } from '../api/evolution'
import { fetchWorkflows, type Workflow } from '../api/workflows'

const workflows = ref<Workflow[]>([])
const loading = ref(false)
const starting = ref(false)

const llmCount = computed(() => workflows.value.filter((workflow) => workflow.recommendation_source === 'llm').length)
const fallbackCount = computed(() => workflows.value.length - llmCount.value)
const installableCount = computed(() => workflows.value.filter((workflow) => workflow.can_generate_skill).length)

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
    workflows.value = Array.isArray(res.data) ? res.data : ((res.data as any)?.items || [])
  } finally {
    loading.value = false
  }
}

async function handleStartEvolution() {
  starting.value = true
  try {
    const res = await startEvolution()
    if (res.success) {
      ElMessage.success('已启动进化管道，将自动扫描本机历史记录并生成推荐。')
      window.setTimeout(load, 1500)
    } else {
      ElMessage.error(res.error || '启动进化管道失败')
    }
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : '启动进化管道失败')
  } finally {
    starting.value = false
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

onMounted(load)
</script>

<template>
  <section class="page-view">
    <section class="panel">
      <div class="head">
        <div>
          <h2>推荐技能</h2>
          <p>启动进化后会自动完成扫描、聚类、生成草稿和社区对比，不需要先去系统管理手动扫描。</p>
        </div>
        <el-button @click="handleStartEvolution" :loading="starting" type="primary" size="default">
          {{ starting ? '启动中...' : '启动进化' }}
        </el-button>
      </div>

      <div v-if="workflows.length > 0" class="body" v-loading="loading">
        <div class="workflow-summary">
          <span class="chip green">大模型 {{ llmCount }}</span>
          <span class="chip orange">本地分析 {{ fallbackCount }}</span>
          <span class="chip">合计 {{ workflows.length }}</span>
          <span class="chip violet">可生成 {{ installableCount }}</span>
        </div>

        <article v-for="(workflow, index) in workflows" :key="workflow.id" class="item workflow-item">
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

      <div v-else class="body empty-state" v-loading="loading">
        <p>暂无推荐。</p>
        <p>点击“启动进化”后，系统会自动扫描已启用 Agent 的历史记录，提取用户请求和压缩摘要，再生成工作流推荐。</p>
        <p>如果进化完成后仍为 0，请到“数据源”确认目标 Agent 已检测到历史会话路径。</p>
      </div>
    </section>

    <section class="two" style="margin-top: 20px" v-if="workflows.some((workflow) => workflow.can_generate_skill)">
      <section class="panel" v-for="workflow in workflows.filter((item) => item.can_generate_skill).slice(0, 4)" :key="'draft-' + workflow.id">
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
.workflow-summary,
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
