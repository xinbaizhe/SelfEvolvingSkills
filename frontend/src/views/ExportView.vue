<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { exportAgents as exportAgentsData, exportSkills } from '../api/export'

const { t } = useI18n()
const exporting = ref(false)

function downloadBlob(content: string, filename: string, type: string) {
  const blob = new Blob([content], { type })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

async function exportSkillsJson() {
  exporting.value = true
  try {
    const data = await exportSkills('json')
    downloadBlob(data.content, data.filename, data.content_type)
    ElMessage.success(t('workbench.export.skillsJsonSuccess'))
  } catch {
    ElMessage.error(t('workbench.export.failed'))
  } finally {
    exporting.value = false
  }
}

async function exportSkillsCsv() {
  exporting.value = true
  try {
    const data = await exportSkills('csv')
    downloadBlob(data.content, data.filename, data.content_type)
    ElMessage.success(t('workbench.export.skillsCsvSuccess'))
  } catch {
    ElMessage.error(t('workbench.export.failed'))
  } finally {
    exporting.value = false
  }
}

async function exportAgents() {
  exporting.value = true
  try {
    const data = await exportAgentsData()
    downloadBlob(data.content, data.filename, data.content_type)
    ElMessage.success(t('workbench.export.agentsJsonSuccess'))
  } catch {
    ElMessage.error(t('workbench.export.failed'))
  } finally {
    exporting.value = false
  }
}
</script>

<template>
  <div>
    <h2 style="margin-bottom: 16px">{{ t('workbench.export.title') }}</h2>

    <el-row :gutter="20">
      <el-col :span="8">
        <el-card>
          <template #header>{{ t('workbench.export.skillsJsonCard') }}</template>
          <p style="color: #909399; font-size: 13px; min-height: 40px">{{ t('workbench.export.skillsJsonDesc') }}</p>
          <el-button type="primary" :loading="exporting" @click="exportSkillsJson">{{ t('workbench.export.exportJson') }}</el-button>
        </el-card>
      </el-col>
      <el-col :span="8">
        <el-card>
          <template #header>{{ t('workbench.export.skillsCsvCard') }}</template>
          <p style="color: #909399; font-size: 13px; min-height: 40px">{{ t('workbench.export.skillsCsvDesc') }}</p>
          <el-button type="success" :loading="exporting" @click="exportSkillsCsv">{{ t('workbench.export.exportCsv') }}</el-button>
        </el-card>
      </el-col>
      <el-col :span="8">
        <el-card>
          <template #header>{{ t('workbench.export.agentsJsonCard') }}</template>
          <p style="color: #909399; font-size: 13px; min-height: 40px">{{ t('workbench.export.agentsJsonDesc') }}</p>
          <el-button type="warning" :loading="exporting" @click="exportAgents">{{ t('workbench.export.exportJson') }}</el-button>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>
