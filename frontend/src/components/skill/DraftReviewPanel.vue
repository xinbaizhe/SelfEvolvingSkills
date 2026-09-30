<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { SkillReviewFeedback, Workflow } from '../../api/workflows'

const { t } = useI18n()

const props = defineProps<{
  draft: Workflow
}>()

const reviewFeedback = computed<SkillReviewFeedback | null>(() => props.draft.review_feedback || null)

const reviewSections = computed(() => {
  const feedback = reviewFeedback.value || {}
  return [
    { key: 'safety', titleKey: 'workbench.common.review.safety', items: feedback.safety || [] },
    { key: 'performance', titleKey: 'workbench.common.review.performance', items: feedback.performance || [] },
    { key: 'functionality', titleKey: 'workbench.common.review.functionality', items: feedback.functionality || [] },
    { key: 'writing', titleKey: 'workbench.common.review.writing', items: feedback.writing || [] },
    { key: 'improvements', titleKey: 'workbench.common.review.improvements', items: feedback.improvements || [] },
  ]
})

function reviewVerdictLabel(verdict?: string) {
  const map: Record<string, string> = {
    install: t('workbench.common.review.verdictInstall'),
    revise: t('workbench.common.review.verdictRevise'),
    merge: t('workbench.common.review.verdictMerge'),
    discard: t('workbench.common.review.verdictDiscard'),
  }
  return map[verdict || ''] || verdict || t('workbench.common.review.verdictPending')
}

function reviewScoreType(score?: number | null) {
  if (score == null) return 'info'
  if (score >= 85) return 'success'
  if (score >= 70) return 'warning'
  return 'danger'
}
</script>

<template>
  <aside class="review-pane">
    <div class="pane-title">
      <h3>{{ t('workbench.common.review.title') }}</h3>
      <span>Skill Review Agent</span>
    </div>
    <div v-if="draft.review_score != null || reviewFeedback" class="review-summary">
      <el-tag :type="reviewScoreType(draft.review_score)">
        {{ t('workbench.common.review.score', { score: draft.review_score ?? '-' }) }}
      </el-tag>
      <el-tag type="info">{{ reviewVerdictLabel(reviewFeedback?.verdict) }}</el-tag>
      <p>{{ draft.review_summary || t('workbench.common.review.summaryEmpty') }}</p>
    </div>
    <div v-if="reviewFeedback" class="review-sections">
      <section v-for="section in reviewSections" :key="section.key" class="review-section">
        <h4>{{ t(section.titleKey) }}</h4>
        <ul v-if="section.items.length">
          <li v-for="item in section.items" :key="item">{{ item }}</li>
        </ul>
        <p v-else>{{ t('workbench.common.review.noIssues') }}</p>
      </section>
    </div>
    <el-empty v-else :description="t('workbench.common.review.emptyDescription')" />
  </aside>
</template>

<style scoped>
.review-pane {
  border-left: 1px solid var(--line);
  padding-left: 16px;
}

.pane-title {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
}

.pane-title h3 { margin: 0; font-size: 15px; }
.pane-title span { color: var(--muted); font-size: 12px; }

.review-summary {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 14px;
}

.review-summary p {
  flex-basis: 100%;
  margin: 2px 0 0;
  color: #334155;
  font-size: 13px;
  line-height: 1.6;
}

.review-sections { display: grid; gap: 12px; }

.review-section {
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 10px;
  background: #fff;
}

.review-section h4 { margin: 0 0 8px; font-size: 13px; }
.review-section ul { margin: 0; padding-left: 18px; color: #475569; font-size: 12px; line-height: 1.6; }
.review-section p { margin: 0; color: var(--muted); font-size: 12px; }

@media (max-width: 1180px) {
  .review-pane {
    border-left: 0;
    border-top: 1px solid var(--line);
    padding-left: 0;
    padding-top: 16px;
  }
}
</style>
