<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { searchCommunitySkills, fetchCommunitySkillDetail, compareCommunitySkill, type CommunitySkill, type CompareResult } from '../api/community'
import { fetchWorkflows, updateWorkflowDraft, type Workflow } from '../api/workflows'

const drafts = ref<Workflow[]>([])
const selectedDraftId = ref<number | null>(null)
const communitySkills = ref<CommunitySkill[]>([])
const selectedCommunityId = ref<number | null>(null)
const communitySkill = ref<CommunitySkill | null>(null)
const loadingDrafts = ref(false)
const loadingCommunity = ref(false)
const searching = ref(false)
const comparing = ref(false)
const compareResult = ref<CompareResult | null>(null)

const selectedDraft = computed(() => drafts.value.find((d) => d.id === selectedDraftId.value) || null)

onMounted(loadDrafts)

async function loadDrafts() {
  loadingDrafts.value = true
  try {
    const res = await fetchWorkflows()
    const items = Array.isArray(res.data) ? res.data : ((res.data as any)?.items || [])
    drafts.value = items.filter((w: Workflow) => w.can_generate_skill)
    selectedDraftId.value = drafts.value[0]?.id ?? null
  } finally {
    loadingDrafts.value = false
  }
}

async function searchSkills() {
  if (!selectedDraft.value) return
  searching.value = true
  communitySkills.value = []
  selectedCommunityId.value = null
  communitySkill.value = null
  compareResult.value = null
  try {
    const keyword = [selectedDraft.value.name, selectedDraft.value.description]
      .filter(Boolean)
      .join(' ')
      .split(/\s+/)
      .slice(0, 6)
      .join(' ')
    const res = await searchCommunitySkills(keyword || selectedDraft.value.name, 1, 8)
    if (res.success && res.data) {
      communitySkills.value = res.data.items
      selectedCommunityId.value = communitySkills.value[0]?.id ?? null
      if (communitySkills.value[0]) await loadCommunityDetail(communitySkills.value[0].id)
    } else {
      ElMessage.warning('未找到相似社区 Skill')
    }
  } catch {
    ElMessage.error('社区搜索失败')
  } finally {
    searching.value = false
  }
}

async function onCommunityChange(id: number) {
  await loadCommunityDetail(id)
  compareResult.value = null
}

async function loadCommunityDetail(id: number) {
  loadingCommunity.value = true
  communitySkill.value = null
  try {
    const res = await fetchCommunitySkillDetail(id)
    if (res.success && res.data) {
      communitySkill.value = res.data
    }
  } catch {
    ElMessage.error('获取社区 Skill 详情失败')
  } finally {
    loadingCommunity.value = false
  }
}

async function runCompare() {
  const draft = selectedDraft.value
  const community = communitySkill.value
  if (!draft?.draft_body || !community?.skill_md_content) {
    ElMessage.warning('请确保两端都有内容')
    return
  }
  comparing.value = true
  compareResult.value = null
  try {
    const res = await compareCommunitySkill({
      draft_body: draft.draft_body,
      draft_name: draft.name,
      community_name: community.name,
      community_content: community.skill_md_content,
    })
    if (res.success && res.data) {
      compareResult.value = res.data
    } else {
      ElMessage.error(res.error || '对比分析失败')
    }
  } catch {
    ElMessage.error('对比分析请求失败')
  } finally {
    comparing.value = false
  }
}

async function adoptSuggestions() {
  const draft = selectedDraft.value
  if (!draft || !compareResult.value?.suggestions.length) return

  const prefix = `\n\n## 社区参考改进 (来自社区对比)\n\n${compareResult.value.suggestions.map((s) => `- [ ] ${s}`).join('\n')}\n`
  try {
    const res = await updateWorkflowDraft(draft.id, {
      draft_body: (draft.draft_body || '') + prefix,
    })
    if (res.success) {
      // Update local draft cache
      const found = drafts.value.find((d) => d.id === draft.id)
      if (found) {
        found.draft_body = (draft.draft_body || '') + prefix
      }
      ElMessage.success('建议已追加到草稿末尾，请在工作台人工审核')
    } else {
      ElMessage.error(res.error || '更新失败')
    }
  } catch {
    ElMessage.error('更新草稿失败')
  }
}

function verdictTag(verdict: string) {
  const map: Record<string, { type: string; text: string }> = {
    local_better: { type: 'success', text: '本地更优' },
    community_better: { type: 'warning', text: '社区更优' },
    complementary: { type: 'primary', text: '互补' },
    neutral: { type: 'info', text: '持平' },
  }
  return map[verdict] || { type: 'info', text: verdict }
}

function formatStars(stars: number) {
  return stars >= 1000 ? `${(stars / 1000).toFixed(1)}k` : String(stars)
}
</script>

<template>
  <section class="community-compare" v-loading="loadingDrafts">
    <div class="toolbar">
      <el-select
        v-model="selectedDraftId"
        placeholder="选择本地 Skill 草稿"
        filterable
        style="min-width: 300px"
        @change="searchSkills"
      >
        <el-option v-for="draft in drafts" :key="draft.id" :label="`${draft.name} (评分 ${draft.skill_score})`" :value="draft.id" />
      </el-select>
      <span v-if="communitySkills.length" class="hint">匹配 {{ communitySkills.length }} 个社区 Skill</span>
    </div>

    <el-empty v-if="!loadingDrafts && drafts.length === 0" description="暂无本地草稿。请先运行进化管道。" />

    <template v-else>
      <!-- Community Skill Picker -->
      <div v-if="communitySkills.length > 1" class="community-selector">
        <el-radio-group v-model="selectedCommunityId" @change="onCommunityChange" size="small">
          <el-radio-button v-for="s in communitySkills" :key="s.id" :value="s.id">
            {{ s.name }} · {{ formatStars(s.stars) }} ★
          </el-radio-button>
        </el-radio-group>
      </div>

      <!-- Content Panels -->
      <div class="compare-layout">
        <section class="panel">
          <div class="head">
            <h3>本地草稿</h3>
            <span v-if="selectedDraft" class="chip green">评分 {{ selectedDraft.skill_score }}</span>
          </div>
          <div class="body">
            <pre class="codebox">{{ selectedDraft?.draft_body || '暂无内容' }}</pre>
          </div>
        </section>

        <section class="panel" v-loading="loadingCommunity">
          <div class="head">
            <h3>社区参考</h3>
            <template v-if="communitySkill">
              <span class="chip blue">{{ formatStars(communitySkill.stars) }} ★</span>
              <a :href="communitySkill.repo_url" target="_blank" rel="noreferrer" style="font-size: 12px;">查看仓库</a>
            </template>
          </div>
          <div class="body">
            <pre v-if="communitySkill?.skill_md_content" class="codebox">{{ communitySkill.skill_md_content }}</pre>
            <div v-else-if="communitySkill" class="empty-hint">
              <p>{{ communitySkill.description || '暂无描述' }}</p>
              <p class="muted">{{ communitySkill.repo }}</p>
              <p class="muted">该社区 Skill 暂无 SKILL.md 内容缓存，请点击"查看仓库"获取原始内容。</p>
            </div>
            <el-empty v-else description="选择社区 Skill 后显示内容" :image-size="60" />
          </div>
        </section>
      </div>

      <!-- Compare Action -->
      <div v-if="selectedDraft && communitySkill" class="action-bar">
        <el-button type="primary" :loading="comparing" :disabled="!communitySkill.skill_md_content" @click="runCompare">
          {{ compareResult ? '重新对比' : '对比分析' }}
        </el-button>
        <span class="hint">调用大模型进行结构化差异分析</span>
      </div>

      <!-- Results -->
      <section v-if="compareResult" class="results">
        <div v-if="compareResult.summary" class="summary-banner">
          <strong>{{ compareResult.source === 'llm' ? 'AI 分析' : '结构分析' }}：</strong>{{ compareResult.summary }}
        </div>

        <table v-if="compareResult.dimensions.length" class="compare-table">
          <thead>
            <tr>
              <th style="width: 120px">维度</th>
              <th>本地草稿</th>
              <th>社区 Skill</th>
              <th style="width: 100px">判定</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="dim in compareResult.dimensions" :key="dim.label">
              <td><b>{{ dim.label }}</b></td>
              <td>{{ dim.local }}</td>
              <td>{{ dim.community }}</td>
              <td>
                <el-tag :type="verdictTag(dim.verdict).type as any" size="small">
                  {{ verdictTag(dim.verdict).text }}
                </el-tag>
              </td>
            </tr>
          </tbody>
        </table>

        <div v-if="compareResult.suggestions.length" class="suggestions-card">
          <h4>改进建议</h4>
          <ul>
            <li v-for="(s, i) in compareResult.suggestions" :key="i">{{ s }}</li>
          </ul>
          <el-button type="success" size="small" @click="adoptSuggestions" style="margin-top: 10px;">
            采纳建议，追加到本地草稿
          </el-button>
        </div>
      </section>
    </template>
  </section>
</template>

<style scoped>
.community-compare {
  display: grid;
  gap: 16px;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.hint { color: var(--muted); font-size: 13px; }

.community-selector {
  padding: 8px 0;
  overflow-x: auto;
}

.compare-layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 16px;
  align-items: start;
}

.panel {
  background: #fff;
  border-radius: 8px;
  box-shadow: 0 1px 4px rgba(0,0,0,0.08);
  overflow: hidden;
}

.head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--line);
}

.head h3 { margin: 0; font-size: 15px; }

.codebox {
  max-height: 560px;
  overflow-y: auto;
  padding: 14px;
  margin: 0;
  background: #f8f8f8;
  font-size: 12px;
  white-space: pre-wrap;
  font-family: Consolas, monospace;
  line-height: 1.55;
}

.empty-hint { padding: 30px 16px; text-align: center; }
.empty-hint p { margin: 4px 0; }
.empty-hint .muted { color: var(--muted); font-size: 13px; }

.action-bar {
  display: flex;
  align-items: center;
  gap: 12px;
}

.results {
  display: grid;
  gap: 14px;
}

.summary-banner {
  padding: 12px 16px;
  background: #eef2ff;
  border-radius: 6px;
  font-size: 14px;
  line-height: 1.6;
}

.compare-table {
  width: 100%;
  border-collapse: collapse;
  background: #fff;
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 1px 4px rgba(0,0,0,0.08);
}

.compare-table th, .compare-table td {
  padding: 10px 14px;
  border-bottom: 1px solid var(--line);
  font-size: 13px;
  text-align: left;
  vertical-align: top;
}

.compare-table th {
  background: #f9fafb;
  font-weight: 600;
}

.suggestions-card {
  padding: 16px;
  background: #fff;
  border-radius: 8px;
  box-shadow: 0 1px 4px rgba(0,0,0,0.08);
}

.suggestions-card h4 { margin: 0 0 10px; }
.suggestions-card ul { margin: 0; padding-left: 20px; }
.suggestions-card li { margin-bottom: 6px; font-size: 13px; line-height: 1.5; }

@media (max-width: 960px) {
  .compare-layout {
    grid-template-columns: 1fr;
  }
}
</style>
