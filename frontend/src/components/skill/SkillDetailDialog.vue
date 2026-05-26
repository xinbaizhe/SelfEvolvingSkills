<script setup lang="ts">
import { computed } from 'vue'
import type { SkillDetail } from '../../api/skills'

const props = defineProps<{
  visible: boolean
  loading: boolean
  detail: SkillDetail | null
}>()

const emit = defineEmits<{
  (e: 'update:visible', value: boolean): void
}>()

const dialogVisible = computed({
  get: () => props.visible,
  set: (value) => emit('update:visible', value),
})

function formatSize(bytes: number) {
  if (bytes < 1024) return bytes + ' B'
  return (bytes / 1024).toFixed(1) + ' KB'
}
</script>

<template>
  <el-dialog v-model="dialogVisible" :title="detail?.name" width="800px" top="5vh">
    <div v-if="loading" class="empty-state">加载中...</div>
    <div v-else-if="detail">
      <el-descriptions :column="2" border size="small">
        <el-descriptions-item label="名称">{{ detail.name }}</el-descriptions-item>
        <el-descriptions-item label="分类">{{ detail.category }}</el-descriptions-item>
        <el-descriptions-item label="来源">{{ detail.source_type }}</el-descriptions-item>
        <el-descriptions-item label="原始来源">{{ detail.origin || '-' }}</el-descriptions-item>
        <el-descriptions-item label="插件">{{ detail.plugin_name || '-' }}</el-descriptions-item>
        <el-descriptions-item label="使用次数">{{ detail.usage_count }}</el-descriptions-item>
        <el-descriptions-item label="文件大小">{{ formatSize(detail.file_size) }}</el-descriptions-item>
        <el-descriptions-item label="行数">{{ detail.line_count }}</el-descriptions-item>
        <el-descriptions-item label="文件路径" :span="2">{{ detail.file_path }}</el-descriptions-item>
      </el-descriptions>
      <div class="detail-section">
        <h4>Markdown 内容预览</h4>
        <div class="code-preview">{{ detail.body_text || '无内容' }}</div>
      </div>
    </div>
  </el-dialog>
</template>

<style scoped>
.empty-state { text-align: center; padding: 40px; color: var(--muted); }
.detail-section { margin-top: 16px; }
.code-preview {
  max-height: 400px;
  overflow-y: auto;
  background: #f8f8f8;
  padding: 12px;
  border-radius: 4px;
  font-size: 13px;
  white-space: pre-wrap;
  font-family: Consolas, monospace;
}
</style>
