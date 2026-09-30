<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { searchCommunitySkills, fetchCommunitySkillDetail, compareCommunitySkill, type CommunitySkill, type CompareResult } from '../api/community'
import { fetchWorkflows, updateWorkflowDraft, type Workflow } from '../api/workflows'
import type { PaginatedResult } from '../api/skills'
import { useBackendText } from '../composables/useBackendText'

const { t } = useI18n()
const backendText = useBackendText()
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
const communityContentAvailable = computed(() => {
  const s = communitySkill.value
  if (!s) return false
  return !!(s.skill_md_content || s.readme_excerpt || s.description)
})

onMounted(loadDrafts)

async function loadDrafts() {
  loadingDrafts.value = true
  try {
    const res = await fetchWorkflows()
    const items = Array.isArray(res.data) ? res.data : ((res.data as unknown as PaginatedResult<Workflow>)?.items || [])
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
      ElMessage.warning(t('workbench.compare.noSimilar'))
    }
  } catch {
    ElMessage.error(t('workbench.compare.searchFailed'))
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
    ElMessage.error(t('workbench.compare.detailFailed'))
  } finally {
    loadingCommunity.value = false
  }
}

async function runCompare() {
  const draft = selectedDraft.value
  const community = communitySkill.value
  const communityContent = community?.skill_md_content
    || community?.readme_excerpt
    || (community?.description ? backendText(community.description) : '')
    || ''
  if (!draft?.draft_body || !community || !communityContent) {
    ElMessage.warning(t('workbench.compare.bothSidesRequired'))
    return
  }
  comparing.value = true
  compareResult.value = null
  try {
    const res = await compareCommunitySkill({
      draft_body: draft.draft_body,
      draft_name: draft.name,
      community_name: backendText(community.name),
      community_content: communityContent,
    })
    if (res.success && res.data) {
      compareResult.value = res.data
    } else {
      ElMessage.error(res.error || t('workbench.compare.compareFailed'))
    }
  } catch {
    ElMessage.error(t('workbench.compare.compareRequestFailed'))
  } finally {
    comparing.value = false
  }
}

async function adoptSuggestions() {
  const draft = selectedDraft.value
  if (!draft || !compareResult.value?.suggestions.length) return

  const prefix = `\n\n## 社区参考改进 (来自社区对比)\n\n${compareResult.value.suggestions.map((s) => `- [ ] ${backendText(s)}`).join('\n')}\n` // i18n-exempt: appended to persisted draft_body (written into the user's Skill content), not UI text
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
      ElMessage.success(t('workbench.compare.adoptSuccess'))
    } else {
      ElMessage.error(res.error || t('workbench.common.updateFailed'))
    }
  } catch {
    ElMessage.error(t('workbench.compare.updateDraftFailed'))
  }
}

function verdictTag(verdict: string) {
  const map: Record<string, { type: string; text: string }> = {
    local_better: { type: 'success', text: t('workbench.compare.verdictLocalBetter') },
    community_better: { type: 'warning', text: t('workbench.compare.verdictCommunityBetter') },
    complementary: { type: 'primary', text: t('workbench.compare.verdictComplementary') },
    neutral: { type: 'info', text: t('workbench.compare.verdictNeutral') },
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
        :placeholder="t('workbench.compare.selectDraftPlaceholder')"
        filterable
        style="min-width: 300px"
        @change="searchSkills"
      >
        <el-option v-for="draft in drafts" :key="draft.id" :label="t('workbench.compare.draftOption', { name: draft.name, score: draft.skill_score })" :value="draft.id" />
      </el-select>
      <span v-if="communitySkills.length" class="hint">{{ t('workbench.compare.matched', { n: communitySkills.length }) }}</span>
    </div>

    <el-empty v-if="!loadingDrafts && drafts.length === 0" :description="t('workbench.compare.emptyDrafts')" />

    <template v-else>
      <!-- Community Skill Picker -->
      <div v-if="communitySkills.length > 1" class="community-selector">
        <el-radio-group v-model="selectedCommunityId" @change="onCommunityChange" size="small">
          <el-radio-button v-for="s in communitySkills" :key="s.id" :value="s.id">
            {{ backendText(s.name) }} · {{ formatStars(s.stars) }} ★
          </el-radio-button>
        </el-radio-group>
      </div>

      <!-- Content Panels -->
      <div class="compare-layout">
        <section class="panel">
          <div class="head">
            <h3>{{ t('workbench.compare.localDraft') }}</h3>
            <span v-if="selectedDraft" class="chip green">{{ t('workbench.compare.score', { score: selectedDraft.skill_score }) }}</span>
          </div>
          <div class="body">
            <pre class="codebox">{{ selectedDraft?.draft_body || t('workbench.compare.noContent') }}</pre>
          </div>
        </section>

        <section class="panel" v-loading="loadingCommunity">
          <div class="head">
            <h3>{{ t('workbench.compare.communityRef') }}</h3>
            <template v-if="communitySkill">
              <span class="chip blue">{{ formatStars(communitySkill.stars) }} ★</span>
              <a :href="communitySkill.repo_url" target="_blank" rel="noreferrer" style="font-size: 12px;">{{ t('workbench.compare.viewRepo') }}</a>
            </template>
          </div>
          <div class="body">
            <pre v-if="communitySkill?.skill_md_content" class="codebox">{{ communitySkill.skill_md_content }}</pre>
            <div v-else-if="communitySkill" class="empty-hint">
              <p>{{ communitySkill.description ? backendText(communitySkill.description) : t('workbench.common.noDescription') }}</p>
              <p class="muted">{{ communitySkill.repo }}</p>
              <p class="muted">{{ t('workbench.compare.noContentHint') }}</p>
            </div>
            <el-empty v-else :description="t('workbench.compare.selectToShow')" :image-size="60" />
          </div>
        </section>
      </div>

      <!-- Compare Action -->
      <div v-if="selectedDraft && communitySkill" class="action-bar">
        <el-button type="primary" :loading="comparing" :disabled="!communityContentAvailable" @click="runCompare">
          {{ compareResult ? t('workbench.compare.recompare') : t('workbench.compare.compare') }}
        </el-button>
        <span class="hint">{{ t('workbench.compare.actionHint') }}</span>
      </div>

      <!-- Results -->
      <section v-if="compareResult" class="results">
        <div v-if="compareResult.summary" class="summary-banner">
          <strong>{{ compareResult.source === 'llm' ? t('workbench.compare.aiAnalysis') : t('workbench.compare.structuralAnalysis') }}：</strong>{{ backendText(compareResult.summary) }}
        </div>

        <table v-if="compareResult.dimensions.length" class="compare-table">
          <thead>
            <tr>
              <th style="width: 120px">{{ t('workbench.compare.colDimension') }}</th>
              <th>{{ t('workbench.compare.localDraft') }}</th>
              <th>{{ t('workbench.compare.communitySkill') }}</th>
              <th style="width: 100px">{{ t('workbench.compare.colVerdict') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(dim, index) in compareResult.dimensions" :key="index">
              <td><b>{{ backendText(dim.label) }}</b></td>
              <td>{{ backendText(dim.local) }}</td>
              <td>{{ backendText(dim.community) }}</td>
              <td>
                <el-tag :type="verdictTag(dim.verdict).type as any" size="small">
                  {{ verdictTag(dim.verdict).text }}
                </el-tag>
              </td>
            </tr>
          </tbody>
        </table>

        <div v-if="compareResult.suggestions.length" class="suggestions-card">
          <h4>{{ t('workbench.compare.suggestions') }}</h4>
          <ul>
            <li v-for="(s, i) in compareResult.suggestions" :key="i">{{ backendText(s) }}</li>
          </ul>
          <el-button type="success" size="small" @click="adoptSuggestions" style="margin-top: 10px;">
            {{ t('workbench.compare.adopt') }}
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
