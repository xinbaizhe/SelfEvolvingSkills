<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import WorkflowsView from './WorkflowsView.vue'
import SkillDraftsView from './SkillDraftsView.vue'
import SkillGardenView from './SkillGardenView.vue'
import SkillCommunityCompareView from './SkillCommunityCompareView.vue'
import EvolutionPipeline from '../components/evolution/EvolutionPipeline.vue'
import EvolutionHistory from '../components/evolution/EvolutionHistory.vue'

const { t } = useI18n()
const route = useRoute()
const activeTab = ref('pipeline')
const refreshKey = ref(0)
const allowedTabs = new Set(['pipeline', 'recommend', 'drafts', 'compare', 'garden', 'history'])

function syncTabFromRoute() {
  const tab = String(route.query.tab || '')
  if (allowedTabs.has(tab)) activeTab.value = tab
}

async function handlePipelineStarted() {
  refreshKey.value += 1
}

async function handlePipelineCompleted() {
  refreshKey.value += 1
  await nextTick()
  activeTab.value = 'recommend'
}

onMounted(syncTabFromRoute)
watch(() => route.query.tab, syncTabFromRoute)
</script>

<template>
  <section class="page-view workbench-view">
    <div class="page-head">
      <div>
        <h2>{{ t('workbench.skillsWorkbench.title') }}</h2>
        <p>{{ t('workbench.skillsWorkbench.subtitle') }}</p>
      </div>
    </div>

    <el-tabs v-model="activeTab" class="workspace-tabs">
      <el-tab-pane :label="t('workbench.skillsWorkbench.tabPipeline')" name="pipeline">
        <EvolutionPipeline @started="handlePipelineStarted" @completed="handlePipelineCompleted" />
      </el-tab-pane>
      <el-tab-pane :label="t('workbench.skillsWorkbench.tabRecommend')" name="recommend">
        <WorkflowsView :key="`recommend-${refreshKey}`" />
      </el-tab-pane>
      <el-tab-pane :label="t('workbench.skillsWorkbench.tabDrafts')" name="drafts">
        <SkillDraftsView :key="`drafts-${refreshKey}`" />
      </el-tab-pane>
      <el-tab-pane :label="t('workbench.skillsWorkbench.tabCompare')" name="compare">
        <SkillCommunityCompareView :key="`compare-${refreshKey}`" />
      </el-tab-pane>
      <el-tab-pane :label="t('workbench.skillsWorkbench.tabGarden')" name="garden">
        <SkillGardenView :key="`garden-${refreshKey}`" />
      </el-tab-pane>
      <el-tab-pane :label="t('workbench.skillsWorkbench.tabHistory')" name="history">
        <EvolutionHistory :key="`history-${refreshKey}`" />
      </el-tab-pane>
    </el-tabs>
  </section>
</template>

<style scoped>
.workbench-view :deep(.page-view) {
  padding: 0;
}

.page-head {
  margin-bottom: 14px;
}

.page-head h2 {
  margin: 0 0 6px;
}

.page-head p {
  margin: 0;
  color: var(--muted);
  font-size: 13px;
}

.workspace-tabs :deep(.el-tabs__content) {
  overflow: visible;
}
</style>
