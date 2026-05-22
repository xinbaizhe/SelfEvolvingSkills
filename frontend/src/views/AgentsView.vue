<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { fetchAgents, fetchAgentDetail } from '../api/agents'

const agents = ref<any[]>([])
const total = ref(0)
const loading = ref(false)
const search = ref('')
const selectedSource = ref<string | null>(null)
const sourceCounts = ref<Record<string, number>>({})

const detailVisible = ref(false)
const currentDetail = ref<any>(null)
const detailLoading = ref(false)

const sources = [
  { id: 'hermes', name: 'Hermes', color: '#6d5bd0', icon: 'H' },
  { id: 'openclaw', name: 'OpenClaw', color: '#7c3aed', icon: 'OC' },
  { id: 'claude-code', name: 'Claude Code', color: '#1473e6', icon: 'CC' },
  { id: 'codex', name: 'Codex', color: '#0f9f7a', icon: 'CX' },
  { id: 'vscode', name: 'VSCode', color: '#d98612', icon: 'CL' },
  { id: 'cursor', name: 'Cursor', color: '#d64f4f', icon: 'CU' },
  { id: 'codebuddy', name: 'CodeBuddy', color: '#6d5bd0', icon: 'CB' },
  { id: 'trae', name: 'TRAE', color: '#1473e6', icon: 'TR' },
  { id: 'zeelinclaw', name: 'ZeeLinClaw', color: '#0f9f7a', icon: 'ZC' },
]

const selectedSourceName = computed(() => {
  return sources.find((source) => source.id === selectedSource.value)?.name || ''
})

async function loadSourceCounts() {
  const entries = await Promise.all(
    sources.map(async (source) => {
      const res = await fetchAgents({ size: 1, agent_source: source.id })
      return [source.id, Number(res.data?.total || 0)] as const
    })
  )
  sourceCounts.value = Object.fromEntries(entries)
}

async function loadAgents(agentSource?: string) {
  loading.value = true
  try {
    const params: Record<string, any> = { size: 100 }
    if (agentSource) params.agent_source = agentSource
    if (search.value) params.search = search.value
    const res = await fetchAgents(params)
    if (res.success && res.data) {
      agents.value = res.data.items
      total.value = res.data.total
    }
  } finally {
    loading.value = false
  }
}

function selectSource(sourceId: string) {
  selectedSource.value = sourceId
  search.value = ''
  loadAgents(sourceId)
}

function clearSource() {
  selectedSource.value = null
  search.value = ''
  agents.value = []
  total.value = 0
  loadSourceCounts()
}

async function showDetail(name: string) {
  detailVisible.value = true
  detailLoading.value = true
  currentDetail.value = null
  try {
    const res = await fetchAgentDetail(name)
    if (res.success) currentDetail.value = res.data
  } finally {
    detailLoading.value = false
  }
}

function onSearch() {
  if (selectedSource.value) loadAgents(selectedSource.value)
}

onMounted(loadSourceCounts)
</script>

<template>
  <section class="page-view">
    <div class="page-headline">
      <div>
        <h2>Agent 列表</h2>
        <p>选择一个 Agent 来源查看 Agent 定义。</p>
      </div>
      <span v-if="selectedSource" class="count-badge">{{ total }} 个 Agent</span>
    </div>

    <div v-if="!selectedSource" class="cards">
      <article
        v-for="src in sources"
        :key="src.id"
        class="card source-card"
        @click="selectSource(src.id)"
      >
        <span class="source-count">{{ sourceCounts[src.id] ?? 0 }}</span>
        <div class="source-icon" :style="{ background: src.color }">{{ src.icon }}</div>
        <h3>{{ src.name }}</h3>
        <p>查看来自 {{ src.name }} 的所有 Agent</p>
      </article>
    </div>

    <template v-else>
      <div class="toolbar">
        <el-button size="small" @click="clearSource">返回来源列表</el-button>
        <span class="toolbar-title">{{ selectedSourceName }} 的 Agent</span>

        <el-input
          v-model="search"
          placeholder="搜索 Agent 名称"
          clearable
          class="search-input"
          @clear="onSearch"
          @keyup.enter="onSearch"
        />
        <el-button type="primary" size="small" @click="onSearch">搜索</el-button>
      </div>

      <div v-if="loading" class="empty-state">加载中...</div>

      <div v-else-if="agents.length === 0" class="empty-state">暂无该来源的 Agent</div>

      <div v-else class="result-grid">
        <div v-for="agent in agents" :key="agent.id" class="agent-card" @click="showDetail(agent.name)">
          <div class="agent-title">
            <span>{{ agent.name }}</span>
            <el-tag v-if="agent.model" size="small">{{ agent.model }}</el-tag>
          </div>
          <div class="item-desc">{{ agent.description || '暂无描述' }}</div>
          <div v-if="agent.tools" class="tag-row">
            <el-tag
              v-for="tool in (Array.isArray(agent.tools) ? agent.tools : [])"
              :key="tool"
              size="small"
              type="info"
            >
              {{ tool }}
            </el-tag>
          </div>
        </div>
      </div>
    </template>

    <el-dialog v-model="detailVisible" :title="currentDetail?.name" width="700px" top="5vh">
      <div v-if="detailLoading" class="empty-state">加载中...</div>
      <div v-else-if="currentDetail">
        <el-descriptions :column="2" border size="small">
          <el-descriptions-item label="名称">{{ currentDetail.name }}</el-descriptions-item>
          <el-descriptions-item label="模型">{{ currentDetail.model || '-' }}</el-descriptions-item>
          <el-descriptions-item label="文件路径" :span="2">{{ currentDetail.file_path }}</el-descriptions-item>
          <el-descriptions-item label="描述" :span="2">{{ currentDetail.description || '-' }}</el-descriptions-item>
        </el-descriptions>
        <div class="detail-section">
          <h4>定义内容</h4>
          <div class="code-preview">{{ currentDetail.body_text || '无内容' }}</div>
        </div>
      </div>
    </el-dialog>
  </section>
</template>

<style scoped>
.page-headline {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 20px;
}
.page-headline h2 { margin: 0 0 8px; }
.page-headline p { margin: 0; color: var(--muted); font-size: 13px; }
.count-badge, .source-count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 999px;
  background: #e9f8f3;
  color: #0c8265;
  font-size: 12px;
  font-weight: 700;
}
.count-badge { min-width: 92px; padding: 6px 10px; }
.source-count {
  position: absolute;
  top: 12px;
  right: 12px;
  min-width: 30px;
  height: 24px;
  padding: 0 8px;
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}
.toolbar-title { font-weight: 600; }
.search-input { width: 220px; margin-left: auto; }
.result-grid { display: flex; flex-wrap: wrap; gap: 16px; }
.empty-state { text-align: center; padding: 40px; color: var(--muted); }
.agent-card {
  width: 280px;
  background: #fff;
  border-radius: 8px;
  padding: 16px;
  cursor: pointer;
  box-shadow: 0 1px 4px rgba(0,0,0,0.08);
  transition: box-shadow 0.2s;
}
.agent-card:hover { box-shadow: 0 4px 12px rgba(0,0,0,0.15); }
.source-card {
  position: relative;
  cursor: pointer;
  transition: all 0.2s;
  border: 2px solid transparent;
}
.source-card:hover {
  border-color: var(--blue);
  transform: translateY(-2px);
  box-shadow: 0 10px 28px rgba(20,115,230,.12);
}
.source-icon {
  width: 44px;
  height: 44px;
  border-radius: 10px;
  color: #fff;
  display: grid;
  place-items: center;
  font-weight: 700;
  font-size: 15px;
  margin-bottom: 8px;
}
.agent-title {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
  font-weight: bold;
  font-size: 15px;
}
.item-desc {
  color: #909399;
  font-size: 13px;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.tag-row { display: flex; gap: 4px; flex-wrap: wrap; margin-top: 8px; }
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
