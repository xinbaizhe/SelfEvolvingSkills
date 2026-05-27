<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useTeamStore } from '../../stores/useTeamStore'
import { fetchModelConfigs, createModelConfig, updateModelConfig, deleteModelConfigs, type TeamModelConfig } from '../../api/team'
import { getErrorMessage } from '../../utils/error'
import LoginDialog from '../../components/team/LoginDialog.vue'

const store = useTeamStore()
const loginDialog = ref<InstanceType<typeof LoginDialog> | null>(null)

const loading = ref(false)
const saving = ref(false)
const configs = ref<TeamModelConfig[]>([])
const total = ref(0)
const dialogVisible = ref(false)
const isEditing = ref(false)

const form = reactive<Partial<TeamModelConfig>>({
  name: '',
  provider: '',
  baseUrl: '',
  model: '',
  deptId: undefined,
  isActive: 1,
})

const providers = ['openai', 'anthropic', 'azure', 'local', 'custom']

async function loadConfigs() {
  loading.value = true
  try {
    const res = await fetchModelConfigs({ pageSize: '50' })
    configs.value = res.items
    total.value = res.total
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '加载模型配置失败'))
  } finally {
    loading.value = false
  }
}

function openAdd() {
  isEditing.value = false
  form.name = ''
  form.provider = 'anthropic'
  form.baseUrl = ''
  form.model = ''
  form.deptId = undefined
  form.isActive = 1
  dialogVisible.value = true
}

function openEdit(config: TeamModelConfig) {
  isEditing.value = true
  form.id = config.id
  form.name = config.name
  form.provider = config.provider
  form.baseUrl = config.baseUrl || ''
  form.model = config.model
  form.deptId = config.deptId
  form.isActive = config.isActive
  dialogVisible.value = true
}

async function handleSave() {
  if (!form.name || !form.provider || !form.model) {
    ElMessage.warning('请填写名称、提供商和模型')
    return
  }
  saving.value = true
  try {
    if (isEditing.value && form.id) {
      await updateModelConfig(form)
      ElMessage.success('模型配置已更新')
    } else {
      await createModelConfig(form)
      ElMessage.success('模型配置已创建')
    }
    dialogVisible.value = false
    await loadConfigs()
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '保存失败'))
  } finally {
    saving.value = false
  }
}

async function handleDelete(config: TeamModelConfig) {
  try {
    await ElMessageBox.confirm(`确定删除模型配置 "${config.name}"？`, '确认删除', { type: 'warning' })
    await deleteModelConfigs([config.id])
    ElMessage.success('已删除')
    await loadConfigs()
  } catch { /* cancelled */ }
}

onMounted(() => {
  if (store.isAuthenticated) loadConfigs()
})
</script>

<template>
  <div class="model-config-panel">
    <div class="panel-header">
      <h3>团队模型配置</h3>
      <el-button type="primary" size="small" @click="openAdd">新增模型</el-button>
    </div>

    <template v-if="!store.isAuthenticated">
      <div class="login-prompt">
        <div class="login-card">
          <h3>团队模型配置</h3>
          <p>登录团队版后可管理共享的模型配置</p>
          <el-button type="primary" @click="loginDialog?.open()">登录团队版</el-button>
        </div>
      </div>
    </template>
    <template v-else>
      <el-table :data="configs" v-loading="loading" size="small" style="width: 100%">
        <el-table-column prop="name" label="名称" width="140" />
        <el-table-column prop="provider" label="提供商" width="100" />
        <el-table-column prop="model" label="模型" min-width="160" />
        <el-table-column prop="baseUrl" label="Base URL" min-width="180">
          <template #default="{ row }">
            <span class="mono">{{ row.baseUrl || '-' }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="deptId" label="部门ID" width="80" />
        <el-table-column prop="isActive" label="状态" width="70">
          <template #default="{ row }">
            <el-tag :type="row.isActive ? 'success' : 'info'" size="small">
              {{ row.isActive ? '启用' : '停用' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="140" fixed="right">
          <template #default="{ row }">
            <el-button size="small" text @click="openEdit(row)">编辑</el-button>
            <el-button size="small" text type="danger" @click="handleDelete(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>

      <div v-if="!loading && configs.length === 0" class="empty-hint">
        暂无模型配置，点击"新增模型"添加
      </div>

      <el-dialog v-model="dialogVisible" :title="isEditing ? '编辑模型配置' : '新增模型配置'" width="480px" top="12vh">
        <el-form label-position="top" @submit.prevent="handleSave">
          <el-form-item label="名称">
            <el-input v-model="form.name" placeholder="如 生产环境 Claude" />
          </el-form-item>
          <el-form-item label="提供商">
            <el-select v-model="form.provider" style="width: 100%">
              <el-option v-for="p in providers" :key="p" :label="p" :value="p" />
            </el-select>
          </el-form-item>
          <el-form-item label="模型">
            <el-input v-model="form.model" placeholder="如 claude-sonnet-4-6" />
          </el-form-item>
          <el-form-item label="Base URL (可选)">
            <el-input v-model="form.baseUrl" placeholder="如 https://api.anthropic.com" />
          </el-form-item>
          <el-form-item label="部门 ID (可选)">
            <el-input-number v-model="form.deptId" :min="1" style="width: 100%" />
          </el-form-item>
          <el-form-item label="启用">
            <el-switch v-model="form.isActive" :active-value="1" :inactive-value="0" />
          </el-form-item>
        </el-form>
        <template #footer>
          <el-button @click="dialogVisible = false">取消</el-button>
          <el-button type="primary" :loading="saving" @click="handleSave">保存</el-button>
        </template>
      </el-dialog>
    </template>

    <LoginDialog ref="loginDialog" @logged-in="loadConfigs" />
  </div>
</template>

<style scoped>
.model-config-panel {
  padding: 0;
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 14px;
}

.panel-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}

.mono {
  font-family: Consolas, monospace;
  font-size: 12px;
  color: var(--muted);
}

.empty-hint {
  padding: 32px 0;
  text-align: center;
  color: var(--muted);
  font-size: 13px;
}

.login-prompt {
  display: grid;
  place-items: center;
  min-height: 200px;
}

.login-card {
  text-align: center;
  padding: 36px;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  background: var(--panel);
}

.login-card h3 {
  margin: 0 0 8px;
  font-size: 16px;
  color: var(--ink);
}

.login-card p {
  margin: 0 0 18px;
  color: var(--muted);
  font-size: 13px;
}
</style>
