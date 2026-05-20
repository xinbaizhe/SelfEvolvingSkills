<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { searchCommunitySkills, type CommunitySkill } from '../api/community'
import { fetchWorkflows, type Workflow } from '../api/workflows'

const drafts = ref<Workflow[]>([])
const selectedDraftId = ref<number | null>(null)
const communitySkills = ref<CommunitySkill[]>([])
const selectedCommunityId = ref<number | null>(null)
const loadingDrafts = ref(false)
const searching = ref(false)
const suggestion = ref('')

const selectedDraft = computed(() => drafts.value.find((draft) => draft.id === selectedDraftId.value) || null)
const selectedCommunitySkill = computed(() => communitySkills.value.find((skill) => skill.id === selectedCommunityId.value) || null)

const searchKeywords = computed(() => {
  const draft = selectedDraft.value
  if (!draft) return ''
  return [draft.name, draft.description, draft.source_agents]
    .filter(Boolean)
    .join(' ')
    .split(/\s+/)
    .slice(0, 8)
    .join(' ')
})

const draftSections = computed(() => splitSections(selectedDraft.value?.draft_body || ''))
const comparisonRows = computed(() => {
  const draft = selectedDraft.value
  const community = selectedCommunitySkill.value
  if (!draft || !community) return []

  return [
    {
      label: '定位',
      local: draft.description || '本地草稿暂无描述',
      community: community.description || '社区 Skill 暂无描述',
    },
    {
      label: '来源',
      local: draft.source_agents || '来自本地工作流聚类',
      community: `${community.repo}，${formatStars(community.stars)} stars`,
    },
    {
      label: '结构',
      local: draftSections.value.length > 0 ? draftSections.value.join(' / ') : '未检测到 Markdown 标题结构',
      community: 'GitHub 搜索 API 当前只返回仓库摘要，详细结构需要打开仓库查看',
    },
    {
      label: '示例任务',
      local: parseSampleTasks(draft.sample_tasks).join('；') || '暂无示例任务',
      community: '可从社区仓库 README 或 SKILL.md 中人工核对',
    },
  ]
})

onMounted(loadDrafts)

async function loadDrafts() {
  loadingDrafts.value = true
  try {
    const res = await fetchWorkflows()
    const items = Array.isArray(res.data) ? res.data : ((res.data as any)?.items || [])
    drafts.value = items.filter((workflow: Workflow) => workflow.can_generate_skill)
    selectedDraftId.value = drafts.value[0]?.id ?? null
    if (selectedDraftId.value) await searchSimilarSkills()
  } finally {
    loadingDrafts.value = false
  }
}

async function searchSimilarSkills() {
  if (!selectedDraft.value) return
  searching.value = true
  suggestion.value = ''
  try {
    const keyword = searchKeywords.value || selectedDraft.value.name
    const res = await searchCommunitySkills(keyword, 1, 8)
    if (res.success && res.data) {
      communitySkills.value = res.data.items
      selectedCommunityId.value = communitySkills.value[0]?.id ?? null
      if (communitySkills.value.length === 0) {
        ElMessage.info('没有找到相似社区 Skill。请检查网络连接，或换一个草稿重试。')
      }
    } else {
      ElMessage.error(res.error || '社区 Skill 搜索失败')
    }
  } catch (error) {
    ElMessage.error(error instanceof Error ? `社区 Skill 搜索失败：${error.message}` : '社区 Skill 搜索失败，请检查网络连接')
  } finally {
    searching.value = false
  }
}

function onDraftChange() {
  communitySkills.value = []
  selectedCommunityId.value = null
  suggestion.value = ''
  searchSimilarSkills()
}

function generateSuggestion() {
  const draft = selectedDraft.value
  const community = selectedCommunitySkill.value
  if (!draft || !community) return

  const tips = [
    `参考 ${community.name} 的定位，把草稿开头改成更明确的适用场景和非适用场景。`,
    '补充输入条件、前置假设和验收标准，避免 Skill 只描述流程而缺少判断边界。',
    '打开社区仓库核对 README/SKILL.md，把可复用结构吸收到本地草稿中。',
    '保留本地草稿里来自真实工作流的步骤，社区内容只作为结构和表达参考。',
  ]

  if (draftSections.value.length === 0) {
    tips.unshift('本地草稿缺少清晰标题结构，建议增加 Purpose、When to use、Steps、Examples、Validation 等章节。')
  }

  suggestion.value = tips.join('\n')
}

function parseSampleTasks(raw: unknown) {
  if (!raw) return []
  if (Array.isArray(raw)) return raw.map(String)
  try {
    const parsed = JSON.parse(String(raw))
    return Array.isArray(parsed) ? parsed.map(String) : []
  } catch {
    return []
  }
}

function splitSections(markdown: string) {
  return markdown
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.startsWith('#'))
    .map((line) => line.replace(/^#+\s*/, ''))
}

function formatStars(stars: number) {
  if (stars >= 1000) return `${(stars / 1000).toFixed(1)}k`
  return String(stars)
}
</script>

<template>
  <section class="community-compare" v-loading="loadingDrafts">
    <div class="compare-toolbar">
      <el-select
        v-model="selectedDraftId"
        placeholder="选择本地 Skill 草稿"
        filterable
        style="min-width: 320px"
        @change="onDraftChange"
      >
        <el-option
          v-for="draft in drafts"
          :key="draft.id"
          :label="draft.name"
          :value="draft.id"
        />
      </el-select>
      <el-button type="primary" :loading="searching" :disabled="!selectedDraft" @click="searchSimilarSkills">
        搜索真实社区 Skill
      </el-button>
      <span class="hint">基于草稿名称、描述和来源调用 GitHub Search API，不使用模拟数据。</span>
    </div>

    <el-empty v-if="!loadingDrafts && drafts.length === 0" description="暂无本地 Skill 草稿。请先运行进化管道生成真实草稿。" />

    <div v-else class="compare-layout">
      <section class="panel">
        <div class="head">
          <div>
            <h2>本地草稿</h2>
            <p>{{ selectedDraft?.description || '选择一个草稿开始对比' }}</p>
          </div>
          <span v-if="selectedDraft" class="chip green">评分 {{ selectedDraft.skill_score }}</span>
        </div>
        <div class="body">
          <pre class="codebox">{{ selectedDraft?.draft_body || '暂无草稿内容' }}</pre>
        </div>
      </section>

      <section class="panel">
        <div class="head">
          <div>
            <h2>社区参考</h2>
            <p>选择一个真实 GitHub 搜索结果进行基础差异对比。</p>
          </div>
        </div>
        <div class="body">
          <el-select
            v-model="selectedCommunityId"
            placeholder="选择社区 Skill"
            filterable
            style="width: 100%"
          >
            <el-option
              v-for="skill in communitySkills"
              :key="skill.id"
              :label="`${skill.name} · ${formatStars(skill.stars)} stars`"
              :value="skill.id"
            />
          </el-select>

          <article v-if="selectedCommunitySkill" class="community-card">
            <h3>{{ selectedCommunitySkill.name }}</h3>
            <p>{{ selectedCommunitySkill.description || '暂无描述' }}</p>
            <div class="card-meta">
              <span>{{ selectedCommunitySkill.repo }}</span>
              <span>{{ formatStars(selectedCommunitySkill.stars) }} stars</span>
            </div>
            <a :href="selectedCommunitySkill.repo_url" target="_blank" rel="noreferrer">
              <el-button size="small">查看仓库</el-button>
            </a>
          </article>

          <el-empty v-if="!searching && communitySkills.length === 0" description="暂无真实社区搜索结果" />
        </div>
      </section>
    </div>

    <section v-if="comparisonRows.length > 0" class="panel compare-table">
      <div class="head">
        <div>
          <h2>差异对比</h2>
          <p>先做基础结构对比，详细内容需要打开社区仓库人工核对。</p>
        </div>
        <el-button @click="generateSuggestion">生成改进建议</el-button>
      </div>
      <table>
        <thead>
          <tr>
            <th>维度</th>
            <th>本地草稿</th>
            <th>社区 Skill</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in comparisonRows" :key="row.label">
            <td><b>{{ row.label }}</b></td>
            <td>{{ row.local }}</td>
            <td>{{ row.community }}</td>
          </tr>
        </tbody>
      </table>
    </section>

    <section v-if="suggestion" class="panel">
      <div class="head">
        <div>
          <h2>改进建议</h2>
          <p>用于人工审核，不会自动修改本地草稿。</p>
        </div>
      </div>
      <div class="body">
        <pre class="suggestion">{{ suggestion }}</pre>
      </div>
    </section>
  </section>
</template>

<style scoped>
.community-compare {
  display: grid;
  gap: 18px;
}

.compare-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.hint {
  color: var(--muted);
  font-size: 13px;
}

.compare-layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(360px, .8fr);
  gap: 18px;
  align-items: start;
}

.codebox {
  max-height: 520px;
}

.community-card {
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 14px;
  background: #fff;
}

.community-card h3 {
  margin: 0 0 8px;
  font-size: 15px;
}

.community-card p {
  color: var(--muted);
  font-size: 13px;
  line-height: 1.5;
}

.card-meta {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  color: var(--muted);
  font-size: 12px;
  margin-bottom: 12px;
}

.compare-table {
  overflow: hidden;
}

.suggestion {
  margin: 0;
  white-space: pre-wrap;
  color: #334155;
  font-family: inherit;
  line-height: 1.65;
}

@media (max-width: 1080px) {
  .compare-layout {
    grid-template-columns: 1fr;
  }
}
</style>
