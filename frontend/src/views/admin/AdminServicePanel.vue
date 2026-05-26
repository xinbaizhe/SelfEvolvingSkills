<script setup lang="ts">
import { ref } from 'vue'
import { ElMessage } from 'element-plus'
import { searchCommunitySkills } from '../../api/community'
import { testLlmConnection } from '../../api/admin'

interface LlmConfig {
  enabled?: boolean
  provider?: string
  base_url?: string
  model?: string
  has_api_key?: boolean
  api_key_configured?: boolean
}

const props = defineProps<{
  llmConfig: LlmConfig | null
}>()

const emit = defineEmits<{
  (e: 'config'): void
}>()

const llmTesting = ref(false)
const githubTesting = ref(false)

async function testLlmService() {
  if (!props.llmConfig?.base_url || !props.llmConfig?.model) {
    ElMessage.warning('请先配置 LLM Base URL 和模型')
    emit('config')
    return
  }
  if (!props.llmConfig.has_api_key && !props.llmConfig.api_key_configured) {
    ElMessage.warning('请先配置 LLM API Key')
    emit('config')
    return
  }
  llmTesting.value = true
  try {
    const res = await testLlmConnection({
      llm_enabled: !!props.llmConfig.enabled,
      llm_provider: props.llmConfig.provider || 'custom',
      llm_base_url: props.llmConfig.base_url,
      llm_model: props.llmConfig.model,
      llm_api_key: '',
    })
    if (res.success) {
      ElMessage.success(res.data?.message || 'LLM 连接正常')
    } else {
      ElMessage.error(res.error || 'LLM 连接失败')
    }
  } finally {
    llmTesting.value = false
  }
}

async function testGithubService() {
  githubTesting.value = true
  try {
    const res = await searchCommunitySkills('skill', 1, 1)
    if (res.success) {
      ElMessage.success('GitHub 社区检索可访问')
    } else {
      ElMessage.error(res.error || 'GitHub 社区检索失败')
    }
  } finally {
    githubTesting.value = false
  }
}
</script>

<template>
  <section class="section">
    <div class="section-head">
      <h3>服务连通性</h3>
      <span>检测社区检索和大模型优化能力</span>
    </div>
    <el-row :gutter="16">
      <el-col :xs="24" :md="12">
        <div class="service-card">
          <div>
            <h4>GitHub 社区检索</h4>
            <p>用于搜索社区 Skill 参考，网络失败时本地扫描和聚类仍可运行。</p>
          </div>
          <el-button :loading="githubTesting" @click="testGithubService">测试 GitHub</el-button>
        </div>
      </el-col>
      <el-col :xs="24" :md="12">
        <div class="service-card">
          <div>
            <h4>LLM 模型服务</h4>
            <p>{{ llmConfig?.enabled ? '已启用' : '未启用' }} · {{ llmConfig?.provider || '未配置' }} · {{ llmConfig?.model || '未选择模型' }}</p>
          </div>
          <div class="service-actions">
            <el-button :loading="llmTesting" @click="testLlmService">测试 LLM</el-button>
            <el-button link type="primary" @click="emit('config')">配置</el-button>
          </div>
        </div>
      </el-col>
    </el-row>
  </section>
</template>

<style scoped>
.section {
  margin-bottom: 18px;
  padding: 18px;
  background: rgba(255, 255, 255, 0.82);
  border: 1px solid rgba(226, 232, 240, 0.86);
  border-radius: 12px;
  box-shadow: 0 8px 28px rgba(15, 23, 42, 0.06);
}

.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid #eef2f7;
}

.section-head h3 {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 0;
  color: #0f172a;
  font-size: 17px;
  font-weight: 700;
}

.section-head h3::before {
  content: "";
  width: 8px;
  height: 18px;
  border-radius: 999px;
  background: linear-gradient(180deg, #14b8a6, #0891b2);
}

.section-head span {
  color: #64748b;
  font-size: 12px;
}

.service-card {
  position: relative;
  min-height: 100%;
  background: linear-gradient(180deg, #ffffff, #fbfdff);
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  padding: 18px;
  box-shadow: 0 2px 12px rgba(15, 23, 42, 0.04);
  overflow: hidden;
  transition: transform 0.18s ease, box-shadow 0.18s ease, border-color 0.18s ease;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  min-height: 122px;
}

.service-card::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 3px;
  background: linear-gradient(180deg, #14b8a6, #0891b2);
}

.service-card:hover {
  transform: translateY(-2px);
  border-color: rgba(13, 148, 136, 0.24);
  box-shadow: 0 14px 34px rgba(15, 23, 42, 0.08);
}

.service-card h4 {
  margin: 0;
  color: #0f172a;
  font-size: 15px;
  font-weight: 800;
}

.service-card p {
  margin: 8px 0 0;
  color: #64748b;
  font-size: 13px;
  line-height: 1.55;
}

.service-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

@media (max-width: 900px) {
  .section-head,
  .service-card {
    align-items: flex-start;
    flex-direction: column;
  }
}
</style>
