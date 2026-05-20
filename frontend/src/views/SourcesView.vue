<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { invoke } from '@tauri-apps/api/core'
import { deleteSource, detectSources, fetchSources, updateSource, type SourceConfig } from '../api/scan'

const sources = ref<SourceConfig[]>([])
const loading = ref(false)

const pathTypes = [
  { key: 'skills_path', label: 'Skills 路径' },
  { key: 'extra_skills_path', label: '额外 Skills 路径' },
  { key: 'agents_path', label: 'Agent 路径' },
  { key: 'extra_agents_path', label: '额外 Agent 路径' },
  { key: 'sessions_path', label: '会话路径' },
  { key: 'projects_path', label: '项目路径' },
  { key: 'plugins_path', label: '插件 Skills 路径' },
  { key: 'memory_path', label: '记忆路径' },
]

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
    ElMessage.success('来源检测完成')
  } finally {
    loading.value = false
  }
}

async function handleToggle(source: SourceConfig) {
  const newState = !source.is_enabled
  await updateSource(source.agent_id, { is_enabled: newState })
  source.is_enabled = newState
  ElMessage.success(`${source.agent_name} 已${newState ? '启用' : '停用'}`)
}

async function choosePath(source: SourceConfig, key: string) {
  const api = (window as any).electronAPI
  let selected: string | null = null
  if ((window as any).__TAURI_INTERNALS__) {
    selected = await invoke<string | null>('select_directory')
  } else if (api?.selectDirectory) {
    selected = await api.selectDirectory()
  } else {
    ElMessage.warning('当前环境不支持原生目录选择，请在桌面应用中使用')
    return
  }
  if (!selected) return

  const nextPaths = { ...(source.paths || {}), [key]: selected }
  const res = await updateSource(source.agent_id, { paths: nextPaths })
  if (res.success && res.data) {
    source.paths = res.data.paths
    source.detected_path = res.data.detected_path
    ElMessage.success(`${source.agent_name} 路径已更新`)
  }
}

async function clearPath(source: SourceConfig, key: string) {
  const nextPaths = { ...(source.paths || {}), [key]: null }
  const res = await updateSource(source.agent_id, { paths: nextPaths })
  if (res.success && res.data) {
    source.paths = res.data.paths
    ElMessage.success('路径已清空，将使用默认检测路径')
  }
}

async function resetSource(source: SourceConfig) {
  try {
    await ElMessageBox.confirm(
      `确定重置 ${source.agent_name} 的自定义路径吗？重置后会回到默认检测路径。`,
      '确认重置',
      { confirmButtonText: '重置', cancelButtonText: '取消', type: 'warning' },
    )
    const res = await deleteSource(source.agent_id)
    if (res.success) {
      await load()
      ElMessage.success(`${source.agent_name} 自定义路径已重置`)
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
  if (!source.is_available) return '未检测到'
  if (!source.is_enabled) return '已停用'
  return '启用'
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
          <h2>本机 Agent 来源</h2>
          <p>开启或关闭 Agent 来源，并为每类扫描数据配置真实本机路径。</p>
        </div>
        <el-button @click="handleDetect" :loading="loading">重新检测</el-button>
      </div>

      <div class="body">
        <article v-for="source in sources" :key="source.agent_id" class="source-panel">
          <div class="source-head">
            <div>
              <h3>{{ source.agent_name }}</h3>
              <p>最近活动 {{ formatDate(source.last_activity) }} · {{ source.record_count }} 条记录</p>
            </div>
            <div class="source-actions">
              <span :class="'chip ' + statusClass(source)">{{ statusText(source) }}</span>
              <el-switch
                :model-value="source.is_enabled"
                :disabled="!source.is_available"
                size="small"
                @change="handleToggle(source)"
              />
              <el-button size="small" @click="resetSource(source)">重置路径</el-button>
            </div>
          </div>

          <el-table :data="pathTypes" size="small" border>
            <el-table-column prop="label" label="类型" width="150" />
            <el-table-column label="路径">
              <template #default="{ row }">
                <small class="path-text">{{ source.paths?.[row.key] || '-' }}</small>
              </template>
            </el-table-column>
            <el-table-column label="操作" width="160">
              <template #default="{ row }">
                <div class="row-actions">
                  <el-button size="small" @click="choosePath(source, row.key)">选择</el-button>
                  <el-button size="small" text @click="clearPath(source, row.key)">清空</el-button>
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
