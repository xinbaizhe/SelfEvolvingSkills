<script setup lang="ts">
import { ref, watchEffect } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import SourcesView from './SourcesView.vue'
import AdminConfig from './admin/AdminConfig.vue'

const { t } = useI18n()
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
        <h2>{{ t('workbench.resourceConfig.title') }}</h2>
        <p>{{ t('workbench.resourceConfig.subtitle') }}</p>
      </div>
    </div>

    <el-tabs v-model="activeTab" class="resource-tabs">
      <el-tab-pane :label="t('workbench.resourceConfig.tabSources')" name="sources">
        <SourcesView />
      </el-tab-pane>
      <el-tab-pane :label="t('workbench.resourceConfig.tabModel')" name="model">
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
