<script setup lang="ts">
import { ref } from 'vue'
import { ElMessage } from 'element-plus'
import { exportAgents as exportAgentsData, exportSkills } from '../api/export'

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
    ElMessage.success('技能 JSON 导出成功')
  } catch {
    ElMessage.error('导出失败')
  } finally {
    exporting.value = false
  }
}

async function exportSkillsCsv() {
  exporting.value = true
  try {
    const data = await exportSkills('csv')
    downloadBlob(data.content, data.filename, data.content_type)
    ElMessage.success('技能 CSV 导出成功')
  } catch {
    ElMessage.error('导出失败')
  } finally {
    exporting.value = false
  }
}

async function exportAgents() {
  exporting.value = true
  try {
    const data = await exportAgentsData()
    downloadBlob(data.content, data.filename, data.content_type)
    ElMessage.success('Agent JSON 导出成功')
  } catch {
    ElMessage.error('导出失败')
  } finally {
    exporting.value = false
  }
}
</script>

<template>
  <div>
    <h2 style="margin-bottom: 16px">数据导出</h2>

    <el-row :gutter="20">
      <el-col :span="8">
        <el-card>
          <template #header>技能数据 - JSON</template>
          <p style="color: #909399; font-size: 13px; min-height: 40px">导出所有技能数据为 JSON 格式，包含完整的元数据和内容。</p>
          <el-button type="primary" :loading="exporting" @click="exportSkillsJson">导出 JSON</el-button>
        </el-card>
      </el-col>
      <el-col :span="8">
        <el-card>
          <template #header>技能数据 - CSV</template>
          <p style="color: #909399; font-size: 13px; min-height: 40px">导出技能列表为 CSV 格式，适合在 Excel 中打开查看。</p>
          <el-button type="success" :loading="exporting" @click="exportSkillsCsv">导出 CSV</el-button>
        </el-card>
      </el-col>
      <el-col :span="8">
        <el-card>
          <template #header>Agent 数据 - JSON</template>
          <p style="color: #909399; font-size: 13px; min-height: 40px">导出所有 Agent 定义数据为 JSON 格式。</p>
          <el-button type="warning" :loading="exporting" @click="exportAgents">导出 JSON</el-button>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>
