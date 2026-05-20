<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { getInstalledSkills, installCommunitySkill, searchCommunitySkills, type CommunitySkill } from '../api/community'
import { fetchSkillInstallTargets, type SkillInstallTarget } from '../api/workflows'

const query = ref('')
const skills = ref<CommunitySkill[]>([])
const installTargets = ref<SkillInstallTarget[]>([])
const selectedAgentId = ref('')
const total = ref(0)
const page = ref(1)
const perPage = 20
const loading = ref(false)
const loadingTargets = ref(false)
const activeTab = ref('all')
const installing = ref<Set<number>>(new Set())

const filteredSkills = computed(() => {
  if (activeTab.value === 'installed') return skills.value.filter((skill) => skill.installed)
  if (activeTab.value === 'available') return skills.value.filter((skill) => !skill.installed)
  return skills.value
})

const availableTargets = computed(() => installTargets.value.filter((target) => target.skills_path))
const selectedTarget = computed(() => availableTargets.value.find((target) => target.agent_id === selectedAgentId.value) || null)

async function loadTargets() {
  loadingTargets.value = true
  try {
    const res = await fetchSkillInstallTargets()
    if (res.success) {
      installTargets.value = res.data || []
      if (!selectedAgentId.value && availableTargets.value.length > 0) {
        selectedAgentId.value = availableTargets.value[0].agent_id
      }
    }
  } catch (error: any) {
    ElMessage.error(error.message || '读取 Agent 安装目标失败')
  } finally {
    loadingTargets.value = false
  }
}

async function search() {
  loading.value = true
  try {
    const res = activeTab.value === 'installed'
      ? await getInstalledSkills()
      : await searchCommunitySkills(query.value || 'skill', page.value, perPage)
    if (res.success) {
      skills.value = (res.data as any).items ?? []
      total.value = (res.data as any).total ?? 0
    } else {
      ElMessage.error((res as any).error || '社区 Skills 搜索失败')
    }
  } catch (error: any) {
    ElMessage.error(error.message ? `社区 Skills 搜索失败：${error.message}` : '社区 Skills 搜索失败，请检查网络连接后重试')
  } finally {
    loading.value = false
  }
}

async function handleInstall(skill: CommunitySkill) {
  if (!selectedAgentId.value) {
    ElMessage.warning('请先选择要安装到的 Agent')
    return
  }

  installing.value.add(skill.id)
  try {
    const res = await installCommunitySkill(skill.id, selectedAgentId.value)
    if (res.success) {
      skill.installed = true
      skill.status = '已安装'
      ElMessage.success(`已安装 ${skill.name} 到 ${res.data?.agent_name || selectedTarget.value?.agent_name || '目标 Agent'}`)
    } else {
      ElMessage.error((res as any).error || '安装失败')
    }
  } catch (error: any) {
    ElMessage.error(error.message || '安装失败，请确认网络可访问 GitHub 且目标 Agent 目录可写')
  } finally {
    installing.value.delete(skill.id)
  }
}

function handleTabChange() {
  page.value = 1
  search()
}

function formatStars(value: number) {
  if (value >= 1000) return `${(value / 1000).toFixed(1)}k`
  return value.toString()
}

onMounted(() => {
  loadTargets()
  search()
})
</script>

<template>
  <div class="community-skills">
    <div class="page-header">
      <div>
        <h2>社区 Skills</h2>
        <p class="subtitle">来源为 GitHub Search API，搜索结果会缓存到本地 community_skills 表。</p>
      </div>
    </div>

    <div class="toolbar">
      <el-input
        v-model="query"
        class="search-input"
        placeholder="搜索 GitHub 社区 Skills..."
        clearable
        @keyup.enter="search"
      />
      <el-button type="primary" :loading="loading" @click="search">搜索</el-button>
      <el-select
        v-model="selectedAgentId"
        class="agent-select"
        :loading="loadingTargets"
        placeholder="选择安装目标 Agent"
      >
        <el-option
          v-for="target in availableTargets"
          :key="target.agent_id"
          :label="`${target.agent_name} - ${target.skills_path}`"
          :value="target.agent_id"
        />
      </el-select>
    </div>

    <p v-if="selectedTarget" class="target-hint">
      当前安装目标：{{ selectedTarget.agent_name }}，目录：{{ selectedTarget.skills_path }}
    </p>
    <el-alert
      v-else
      type="warning"
      show-icon
      :closable="false"
      title="未检测到可安装的 Agent Skills 目录，请先到数据源配置 Agent 路径。"
    />

    <el-tabs v-model="activeTab" class="tabs" @tab-change="handleTabChange">
      <el-tab-pane label="全部" name="all" />
      <el-tab-pane label="可安装" name="available" />
      <el-tab-pane label="已安装" name="installed" />
    </el-tabs>

    <div v-loading="loading" class="skills-grid">
      <article
        v-for="skill in filteredSkills"
        :key="skill.id"
        class="skill-card"
        :class="{ installed: skill.installed }"
      >
        <div class="card-header">
          <h4 class="skill-name">{{ skill.name }}</h4>
          <el-tag :type="skill.installed ? 'success' : 'info'" size="small">{{ skill.status }}</el-tag>
        </div>

        <p class="skill-desc">{{ skill.description || '暂无描述' }}</p>

        <div class="card-meta">
          <span class="repo">{{ skill.repo }}</span>
          <span class="stars">★ {{ formatStars(skill.stars) }}</span>
        </div>

        <div class="source-line">
          来源：{{ skill.source || 'GitHub' }}
          <span v-if="skill.fetched_at"> · 获取时间：{{ skill.fetched_at }}</span>
        </div>

        <div class="card-actions">
          <a :href="skill.repo_url" target="_blank" class="repo-link">
            <el-button size="small" text>查看仓库</el-button>
          </a>
          <el-button
            v-if="!skill.installed"
            type="primary"
            size="small"
            :disabled="!selectedAgentId"
            :loading="installing.has(skill.id)"
            @click="handleInstall(skill)"
          >
            安装到 Agent
          </el-button>
          <el-tag v-else type="success" size="small" effect="plain">已安装</el-tag>
        </div>
      </article>

      <div v-if="!loading && filteredSkills.length === 0" class="empty-state">
        <p>暂无社区 Skills</p>
        <p class="hint">请输入关键词搜索 GitHub；网络失败时不会生成模拟数据。</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.community-skills {
  max-width: 1200px;
  margin: 0 auto;
  padding: 24px;
}

.page-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  margin-bottom: 20px;
}

.page-header h2 {
  margin: 0;
  font-size: 22px;
  font-weight: 700;
  color: var(--el-text-color-primary);
}

.subtitle,
.target-hint {
  margin: 6px 0 0;
  font-size: 13px;
  color: var(--el-text-color-secondary);
}

.toolbar {
  display: flex;
  gap: 12px;
  align-items: center;
  margin-bottom: 10px;
}

.search-input {
  max-width: 420px;
}

.agent-select {
  min-width: 360px;
  flex: 1;
}

.tabs {
  margin: 18px 0;
}

.skills-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 16px;
  min-height: 200px;
}

.skill-card {
  padding: 18px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  background: var(--el-bg-color);
}

.skill-card.installed {
  border-color: var(--el-color-success-light-5);
  background: var(--el-color-success-light-9);
}

.card-header,
.card-meta,
.card-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.skill-name {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  overflow-wrap: anywhere;
}

.skill-desc {
  min-height: 40px;
  margin: 12px 0;
  font-size: 13px;
  line-height: 1.5;
  color: var(--el-text-color-secondary);
}

.card-meta,
.source-line {
  margin-bottom: 12px;
  font-size: 12px;
  color: var(--el-text-color-placeholder);
}

.repo {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.stars {
  color: #b7791f;
  font-weight: 600;
}

.repo-link {
  text-decoration: none;
}

.empty-state {
  grid-column: 1 / -1;
  padding: 56px 20px;
  text-align: center;
  color: var(--el-text-color-secondary);
}

.empty-state p {
  margin: 0;
  font-size: 16px;
}

.empty-state .hint {
  margin-top: 8px;
  font-size: 13px;
  color: var(--el-text-color-placeholder);
}
</style>
