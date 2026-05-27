<script setup lang="ts">
import { ref, reactive, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useTeamStore } from '../../stores/useTeamStore'
import { shareSkillToTeam } from '../../api/team'
import { fetchSkills, fetchSkillDetail, type SkillItem } from '../../api/skills'
import { getErrorMessage } from '../../utils/error'

const store = useTeamStore()
const visible = ref(false)
const loading = ref(false)
const sharing = ref(false)

const form = reactive({
  name: '',
  description: '',
  category: '',
  bodyMd: '',
  originAgent: '',
  compatibleModels: '',
  compatibleAgents: '',
})

const localSkills = ref<SkillItem[]>([])
const selectedSkill = ref('')

const emit = defineEmits<{
  (e: 'shared'): void
}>()

async function loadLocalSkills() {
  try {
    const res = await fetchSkills({})
    if (res.success && res.data) {
      localSkills.value = (res.data as { items: SkillItem[] }).items ?? []
    }
  } catch { /* ignore */ }
}

watch(selectedSkill, async (name) => {
  if (!name) return
  loading.value = true
  try {
    const res = await fetchSkillDetail(name)
    if (res.success && res.data) {
      const detail = res.data
      form.name = detail.name
      form.description = detail.description || ''
      form.category = detail.category || 'general'
      form.bodyMd = detail.body_text || ''
      form.originAgent = ''
    }
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '加载 Skill 详情失败'))
  } finally {
    loading.value = false
  }
})

function open() {
  if (!store.isAuthenticated) {
    ElMessage.warning('请先登录团队版')
    return
  }
  visible.value = true
  form.name = ''
  form.description = ''
  form.category = ''
  form.bodyMd = ''
  form.originAgent = ''
  form.compatibleModels = ''
  form.compatibleAgents = ''
  selectedSkill.value = ''
  loadLocalSkills()
}

async function doShare() {
  if (!form.name || !form.bodyMd) {
    ElMessage.warning('请选择 Skill 并确认内容不为空')
    return
  }
  sharing.value = true
  try {
    await shareSkillToTeam({
      name: form.name,
      description: form.description,
      category: form.category || 'general',
      bodyMd: form.bodyMd,
      originAgent: form.originAgent || undefined,
      compatibleModels: form.compatibleModels || undefined,
      compatibleAgents: form.compatibleAgents || undefined,
    })
    ElMessage.success('Skill 已分享到团队')
    visible.value = false
    emit('shared')
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '分享失败'))
  } finally {
    sharing.value = false
  }
}

defineExpose({ open })
</script>

<template>
  <el-dialog v-model="visible" title="分享 Skill 到团队" width="560px" top="10vh">
    <el-form label-position="top" @submit.prevent="doShare">
      <el-form-item label="选择本地 Skill">
        <el-select
          v-model="selectedSkill"
          placeholder="选择要分享的 Skill..."
          filterable
          style="width: 100%"
          :loading="loading"
        >
          <el-option
            v-for="skill in localSkills"
            :key="skill.name"
            :label="skill.name"
            :value="skill.name"
          >
            <span>{{ skill.name }}</span>
            <span style="float: right; color: var(--muted); font-size: 12px">{{ skill.category || 'general' }}</span>
          </el-option>
        </el-select>
      </el-form-item>

      <el-form-item label="名称">
        <el-input v-model="form.name" placeholder="Skill 名称" />
      </el-form-item>

      <el-form-item label="描述">
        <el-input v-model="form.description" type="textarea" :rows="2" placeholder="简短的 Skill 描述" />
      </el-form-item>

      <el-form-item label="分类">
        <el-select v-model="form.category" style="width: 100%">
          <el-option label="通用" value="general" />
          <el-option label="编程" value="coding" />
          <el-option label="DevOps" value="devops" />
          <el-option label="数据" value="data" />
          <el-option label="写作" value="writing" />
          <el-option label="设计" value="design" />
          <el-option label="自动化" value="automation" />
        </el-select>
      </el-form-item>

      <el-form-item label="来源 Agent">
        <el-input v-model="form.originAgent" placeholder="如 claude-code, cursor" />
      </el-form-item>

      <el-form-item label="兼容模型 (逗号分隔)">
        <el-input v-model="form.compatibleModels" placeholder='如 claude-sonnet-4-6, *' />
      </el-form-item>

      <el-form-item label="兼容 Agent (逗号分隔)">
        <el-input v-model="form.compatibleAgents" placeholder='如 claude-code, cursor' />
      </el-form-item>
    </el-form>

    <template #footer>
      <el-button @click="visible = false">取消</el-button>
      <el-button type="primary" :loading="sharing" @click="doShare">分享</el-button>
    </template>
  </el-dialog>
</template>
