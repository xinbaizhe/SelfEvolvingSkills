<script setup lang="ts">
import { ref, watchEffect } from 'vue'
import { useRoute } from 'vue-router'
import SourcesView from './SourcesView.vue'
import AdminConfig from './admin/AdminConfig.vue'

const route = useRoute()
const activeTab = ref('sources')

watchEffect(() => {
  activeTab.value = route.query.tab === 'model' ? 'model' : 'sources'
})
</script>

<template>
  <section class="page-view resource-view">
    <div class="page-head">
      <div>
        <h2>资源与配置</h2>
        <p>管理本机 Agent 来源、扫描路径和大模型推荐配置。</p>
      </div>
    </div>

    <el-tabs v-model="activeTab" class="resource-tabs">
      <el-tab-pane label="Agent 来源" name="sources">
        <SourcesView />
      </el-tab-pane>
      <el-tab-pane label="模型配置" name="model">
        <AdminConfig />
      </el-tab-pane>
    </el-tabs>
  </section>
</template>

<style scoped>
.resource-view :deep(.page-view) {
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

.resource-tabs :deep(.el-tabs__content) {
  overflow: visible;
}
</style>
