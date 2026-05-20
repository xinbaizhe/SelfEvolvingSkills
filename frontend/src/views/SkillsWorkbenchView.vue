<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import WorkflowsView from './WorkflowsView.vue'
import SkillDraftsView from './SkillDraftsView.vue'
import SkillGardenView from './SkillGardenView.vue'
import SkillCommunityCompareView from './SkillCommunityCompareView.vue'
import EvolutionPipeline from '../components/evolution/EvolutionPipeline.vue'
import EvolutionHistory from '../components/evolution/EvolutionHistory.vue'

const route = useRoute()
const activeTab = ref('pipeline')
const refreshKey = ref(0)
const allowedTabs = new Set(['pipeline', 'recommend', 'drafts', 'compare', 'garden', 'history'])

function syncTabFromRoute() {
  const tab = String(route.query.tab || '')
  if (allowedTabs.has(tab)) activeTab.value = tab
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
        <h2>Skills 工作台</h2>
        <p>从真实本机会话中发现重复工作流，生成 Skill 草稿，对比社区参考，再审核安装到目标 Agent。</p>
      </div>
    </div>

    <el-tabs v-model="activeTab" class="workspace-tabs">
      <el-tab-pane label="进化管道" name="pipeline">
        <EvolutionPipeline @completed="handlePipelineCompleted" />
      </el-tab-pane>
      <el-tab-pane label="推荐" name="recommend">
        <WorkflowsView :key="`recommend-${refreshKey}`" />
      </el-tab-pane>
      <el-tab-pane label="草稿" name="drafts">
        <SkillDraftsView :key="`drafts-${refreshKey}`" />
      </el-tab-pane>
      <el-tab-pane label="社区对比" name="compare">
        <SkillCommunityCompareView :key="`compare-${refreshKey}`" />
      </el-tab-pane>
      <el-tab-pane label="已生成" name="garden">
        <SkillGardenView :key="`garden-${refreshKey}`" />
      </el-tab-pane>
      <el-tab-pane label="操作记录" name="history">
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
