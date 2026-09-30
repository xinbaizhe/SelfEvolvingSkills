<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { invoke } from '@tauri-apps/api/core'
import { deleteSource, detectSources, fetchSources, updateSource, type SourceConfig } from '../api/scan'

const { t } = useI18n()
const sources = ref<SourceConfig[]>([])
const loading = ref(false)

const pathTypes = computed(() => [
  { key: 'skills_path', label: t('workbench.sources.pathSkills') },
  { key: 'extra_skills_path', label: t('workbench.sources.pathExtraSkills') },
  { key: 'agents_path', label: t('workbench.sources.pathAgents') },
  { key: 'extra_agents_path', label: t('workbench.sources.pathExtraAgents') },
  { key: 'sessions_path', label: t('workbench.sources.pathSessions') },
  { key: 'projects_path', label: t('workbench.sources.pathProjects') },
  { key: 'plugins_path', label: t('workbench.sources.pathPlugins') },
  { key: 'memory_path', label: t('workbench.sources.pathMemory') },
])

async function load() {
  loading.value = true
  try {
    const res = await fetchSources()
    sources.value = res.data || []
  } finally {
    loading.value = false
  }
}

async function handleDetect() {
  loading.value = true
  try {
    const res = await detectSources()
    sources.value = res.data || []
    ElMessage.success(t('workbench.sources.detected'))
  } finally {
    loading.value = false
  }
}

async function handleToggle(source: SourceConfig) {
  const newState = !source.is_enabled
  await updateSource(source.agent_id, { is_enabled: newState })
  source.is_enabled = newState
  ElMessage.success(t(newState ? 'workbench.sources.enabled' : 'workbench.sources.disabled', { agent: source.agent_name }))
}

async function choosePath(source: SourceConfig, key: string) {
  const api = (window as any).electronAPI
  let selected: string | null = null
  if ((window as any).__TAURI_INTERNALS__) {
    selected = await invoke<string | null>('select_directory')
  } else if (api?.selectDirectory) {
    selected = await api.selectDirectory()
  } else {
    ElMessage.warning(t('workbench.sources.noNativePicker'))
    return
  }
  if (!selected) return

  const nextPaths = { ...(source.paths || {}), [key]: selected }
  const res = await updateSource(source.agent_id, { paths: nextPaths })
  if (res.success && res.data) {
    source.paths = res.data.paths
    source.detected_path = res.data.detected_path
    ElMessage.success(t('workbench.sources.pathUpdated', { agent: source.agent_name }))
  }
}

async function clearPath(source: SourceConfig, key: string) {
  const nextPaths = { ...(source.paths || {}), [key]: null }
  const res = await updateSource(source.agent_id, { paths: nextPaths })
  if (res.success && res.data) {
    source.paths = res.data.paths
    ElMessage.success(t('workbench.sources.pathCleared'))
  }
}

async function resetSource(source: SourceConfig) {
  try {
    await ElMessageBox.confirm(
      t('workbench.sources.resetConfirm', { agent: source.agent_name }),
      t('workbench.sources.resetTitle'),
      { confirmButtonText: t('workbench.sources.reset'), cancelButtonText: t('common.cancel'), type: 'warning' },
    )
    const res = await deleteSource(source.agent_id)
    if (res.success) {
      await load()
      ElMessage.success(t('workbench.sources.resetDone', { agent: source.agent_name }))
    }
  } catch {
    // cancelled
  }
}

function formatDate(dateStr: string | null): string {
  if (!dateStr) return '-'
  const m = dateStr.match(/^(\d{4}-\d{2}-\d{2})/)
  return m ? m[1] : dateStr
}

function statusText(source: SourceConfig): string {
  if (!source.is_available) return t('workbench.common.notDetected')
  if (!source.is_enabled) return t('workbench.common.statusDisabled')
  return t('workbench.common.statusEnabled')
}

function statusClass(source: SourceConfig): string {
  if (!source.is_available) return 'red'
  if (!source.is_enabled) return 'orange'
  return 'green'
}

onMounted(load)
</script>

<template>
  <section class="page-view">
    <section class="panel">
      <div class="head">
        <div>
          <h2>{{ t('workbench.sources.title') }}</h2>
          <p>{{ t('workbench.sources.subtitle') }}</p>
        </div>
        <el-button @click="handleDetect" :loading="loading">{{ t('workbench.sources.redetect') }}</el-button>
      </div>

      <div class="body">
        <article v-for="source in sources" :key="source.agent_id" class="source-panel">
          <div class="source-head">
            <div>
              <h3>{{ source.agent_name }}</h3>
              <p>{{ t('workbench.sources.activity', { date: formatDate(source.last_activity), n: source.record_count }) }}</p>
            </div>
            <div class="source-actions">
              <span :class="'chip ' + statusClass(source)">{{ statusText(source) }}</span>
              <el-switch
                :model-value="source.is_enabled"
                :disabled="!source.is_available"
                size="small"
                @change="handleToggle(source)"
              />
              <el-button size="small" @click="resetSource(source)">{{ t('workbench.sources.resetPaths') }}</el-button>
            </div>
          </div>

          <el-table :data="pathTypes" size="small" border>
            <el-table-column prop="label" :label="t('workbench.common.type')" width="150" />
            <el-table-column :label="t('workbench.sources.path')">
              <template #default="{ row }">
                <small class="path-text">{{ source.paths?.[row.key] || '-' }}</small>
              </template>
            </el-table-column>
            <el-table-column :label="t('workbench.common.actions')" width="160">
              <template #default="{ row }">
                <div class="row-actions">
                  <el-button size="small" @click="choosePath(source, row.key)">{{ t('workbench.sources.choose') }}</el-button>
                  <el-button size="small" text @click="clearPath(source, row.key)">{{ t('workbench.sources.clear') }}</el-button>
                </div>
              </template>
            </el-table-column>
          </el-table>
        </article>
      </div>
    </section>
  </section>
</template>

<style scoped>
.source-panel {
  border: 1px solid var(--line);
  border-radius: 8px;
  overflow: hidden;
  background: #fff;
}

.source-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  padding: 14px;
  background: #fafbfe;
  border-bottom: 1px solid var(--line);
}

.source-head h3 {
  margin: 0 0 4px;
  font-size: 15px;
}

.source-head p {
  margin: 0;
  color: var(--muted);
  font-size: 12px;
}

.source-actions,
.row-actions {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
}

.path-text {
  color: var(--muted);
  word-break: break-all;
}
</style>
