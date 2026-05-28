<script setup lang="ts">
import AdminModelConfigPanel from './AdminModelConfigPanel.vue'
</script>

<template>
  <div class="team-model-page">
    <div class="page-header">
      <div>
        <h2>团队模型管理</h2>
        <p>查看当前团队登录后可用的模型配置。</p>
      </div>
    </div>

    <section class="section">
      <AdminModelConfigPanel />
    </section>
  </div>
</template>

<style scoped>
.team-model-page {
  min-height: 100%;
  padding: 26px 28px 38px;
  background: linear-gradient(180deg, rgba(240, 247, 250, 0.92), rgba(246, 248, 251, 0.45) 320px);
}

.page-header {
  margin-bottom: 18px;
  padding: 24px;
  color: #f8fafc;
  background: linear-gradient(135deg, rgba(8, 35, 46, 0.96), rgba(12, 74, 82, 0.94)), #0f172a;
  border: 1px solid rgba(20, 184, 166, 0.22);
  border-radius: 12px;
  box-shadow: 0 18px 48px rgba(15, 23, 42, 0.18);
}

.page-header h2 {
  margin: 0;
  font-size: 24px;
}

.page-header p {
  margin: 8px 0 0;
  color: #b7c7d8;
  font-size: 13px;
}

.section {
  padding: 18px;
  background: rgba(255, 255, 255, 0.86);
  border: 1px solid rgba(226, 232, 240, 0.86);
  border-radius: 12px;
  box-shadow: 0 8px 28px rgba(15, 23, 42, 0.06);
}

@media (max-width: 900px) {
  .team-model-page {
    padding: 18px;
  }
}
</style>
