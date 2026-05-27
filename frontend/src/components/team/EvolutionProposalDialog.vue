<script setup lang="ts">
import { ref, reactive } from 'vue'
import { ElMessage } from 'element-plus'
import { useTeamStore } from '../../stores/useTeamStore'
import { fetchTeamSkills, fetchTeamSkillDetail, submitEvolution, type TeamSkill } from '../../api/team'
import { getErrorMessage } from '../../utils/error'

const store = useTeamStore()
const visible = ref(false)
const loading = ref(false)
const submitting = ref(false)
const skills = ref<TeamSkill[]>([])
const currentBody = ref('')

const form = reactive({
  skillId: 0,
  skillName: '',
  currentVersion: 0,
  proposedChange: '',
  reason: '',
})

const emit = defineEmits<{
  (e: 'submitted'): void
}>()

async function loadSkills() {
  loading.value = true
  try {
    const res = await fetchTeamSkills({ pageSize: '100' })
    skills.value = res.items
  } catch { /* ignore */ }
  finally { loading.value = false }
}

async function onSkillSelect(skillId: number) {
  if (!skillId) {
    currentBody.value = ''
    form.currentVersion = 0
    return
  }
  try {
    const skill = await fetchTeamSkillDetail(skillId)
    currentBody.value = skill.bodyMd || ''
    form.currentVersion = skill.version || 1
    form.skillName = skill.name
  } catch { /* ignore */ }
}

function open(skillId?: number) {
  if (!store.isAuthenticated) {
    ElMessage.warning('请先登录团队版')
    return
  }
  visible.value = true
  form.skillId = skillId || 0
  form.proposedChange = ''
  form.reason = ''
  form.currentVersion = 0
  form.skillName = ''
  currentBody.value = ''
  loadSkills()
  if (skillId) onSkillSelect(skillId)
}

async function doSubmit() {
  if (!form.skillId || !form.proposedChange) {
    ElMessage.warning('请选择 Skill 并填写改进内容')
    return
  }
  submitting.value = true
  try {
    await submitEvolution({
      skillId: form.skillId,
      proposedChange: form.proposedChange,
      reason: form.reason,
      previousVersion: form.currentVersion > 0 ? `v${form.currentVersion}` : undefined,
    })
    ElMessage.success('进化提案已提交')
    visible.value = false
    emit('submitted')
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '提交失败'))
  } finally {
    submitting.value = false
  }
}

defineExpose({ open })
</script>

<template>
  <el-dialog v-model="visible" title="提交协同进化提案" width="640px" top="8vh">
    <el-form label-position="top" @submit.prevent="doSubmit">
      <el-form-item label="选择 Skill">
        <el-select
          v-model="form.skillId"
          placeholder="选择要进化的团队 Skill..."
          filterable
          style="width: 100%"
          :loading="loading"
          @change="onSkillSelect"
        >
          <el-option
            v-for="skill in skills"
            :key="skill.id"
            :label="`${skill.name} (v${skill.version})`"
            :value="skill.id"
          />
        </el-select>
      </el-form-item>

      <el-form-item v-if="currentBody" label="当前版本内容 (v{{ form.currentVersion || '?' }})">
        <div class="body-preview">{{ currentBody.slice(0, 500) }}{{ currentBody.length > 500 ? '...' : '' }}</div>
      </el-form-item>

      <el-form-item label="改进后内容 (Markdown)">
        <el-input
          v-model="form.proposedChange"
          type="textarea"
          :rows="8"
          placeholder="粘贴改进后的完整 Skill 内容..."
        />
      </el-form-item>

      <el-form-item label="改进理由">
        <el-input
          v-model="form.reason"
          type="textarea"
          :rows="2"
          placeholder="简要说明为什么这样改、改进点是什么"
        />
      </el-form-item>
    </el-form>

    <template #footer>
      <el-button @click="visible = false">取消</el-button>
      <el-button type="primary" :loading="submitting" @click="doSubmit">提交提案</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.body-preview {
  max-height: 200px;
  overflow-y: auto;
  padding: 10px 12px;
  background: #f5f7fa;
  border: 1px solid #e4e7ed;
  border-radius: 6px;
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
  color: var(--muted);
  font-family: Consolas, monospace;
}
</style>
