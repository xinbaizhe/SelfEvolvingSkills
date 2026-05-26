<script setup lang="ts">
import { computed } from 'vue'
import type { SkillReviewFeedback, Workflow } from '../../api/workflows'

const props = defineProps<{
  draft: Workflow
}>()

const reviewFeedback = computed<SkillReviewFeedback | null>(() => props.draft.review_feedback || null)

const reviewSections = computed(() => {
  const feedback = reviewFeedback.value || {}
  return [
    { key: 'safety', title: '安全', items: feedback.safety || [] },
    { key: 'performance', title: '性能', items: feedback.performance || [] },
    { key: 'functionality', title: '功能', items: feedback.functionality || [] },
    { key: 'writing', title: '写法', items: feedback.writing || [] },
    { key: 'improvements', title: '怎么改进', items: feedback.improvements || [] },
  ]
})

function reviewVerdictLabel(verdict?: string) {
  const map: Record<string, string> = {
    install: '建议安装',
    revise: '建议修改',
    merge: '建议合并',
    discard: '建议丢弃',
  }
  return map[verdict || ''] || verdict || '等待评审'
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
      <h3>改进意见</h3>
      <span>Skill Review Agent</span>
    </div>
    <div v-if="draft.review_score != null || reviewFeedback" class="review-summary">
      <el-tag :type="reviewScoreType(draft.review_score)">
        {{ draft.review_score ?? '-' }} 分
      </el-tag>
      <el-tag type="info">{{ reviewVerdictLabel(reviewFeedback?.verdict) }}</el-tag>
      <p>{{ draft.review_summary || '暂无总结' }}</p>
    </div>
    <div v-if="reviewFeedback" class="review-sections">
      <section v-for="section in reviewSections" :key="section.key" class="review-section">
        <h4>{{ section.title }}</h4>
        <ul v-if="section.items.length">
          <li v-for="item in section.items" :key="item">{{ item }}</li>
        </ul>
        <p v-else>暂无明显问题。</p>
      </section>
    </div>
    <el-empty v-else description="暂无评审意见。运行进化管道并启用大模型后会自动生成。" />
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
