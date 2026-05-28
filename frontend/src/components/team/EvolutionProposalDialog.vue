<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
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

const selectedVersionLabel = computed(() =>
  form.currentVersion > 0 ? `${form.skillName} v${form.currentVersion}` : '未选择 Skill'
)

const emit = defineEmits<{
  (e: 'submitted'): void
}>()

async function loadSkills() {
  loading.value = true
  try {
    const res = await fetchTeamSkills({ pageSize: '100' })
    skills.value = res.items
  } catch { /* 团队 Skill 加载失败时仍保留弹窗 */ }
  finally { loading.value = false }
}

async function onSkillSelect(skillId: number) {
  if (!skillId) {
    currentBody.value = ''
    form.currentVersion = 0
    form.skillName = ''
    form.proposedChange = ''
    return
  }
  loading.value = true
  try {
    const skill = await fetchTeamSkillDetail(skillId)
    currentBody.value = skill.bodyMd || ''
    form.currentVersion = skill.version || 1
    form.skillName = skill.name
    form.proposedChange = skill.bodyMd || ''
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '加载团队 Skill 详情失败'))
  } finally {
    loading.value = false
  }
}

function resetForm(skillId?: number) {
  form.skillId = skillId || 0
  form.proposedChange = ''
  form.reason = ''
  form.currentVersion = 0
  form.skillName = ''
  currentBody.value = ''
}

function open(skillId?: number) {
  if (!store.isAuthenticated) {
    ElMessage.warning('请先登录团队版')
    return
  }
  resetForm(skillId)
  visible.value = true
  loadSkills()
  if (skillId) onSkillSelect(skillId)
}

async function doSubmit() {
  if (!form.skillId || !form.proposedChange.trim()) {
    ElMessage.warning('请选择 Skill，并填写改进后的完整内容')
    return
  }
  if (!form.reason.trim()) {
    ElMessage.warning('请填写改进理由，方便团队审核')
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
  <el-dialog v-model="visible" title="提交协同进化提案" width="760px" top="6vh">
    <el-form label-position="top" @submit.prevent="doSubmit">
      <el-form-item label="选择团队 Skill">
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

      <div v-if="currentBody" class="version-panel">
        <div class="version-header">
          <span>当前版本：{{ selectedVersionLabel }}</span>
          <el-button size="small" text type="primary" @click="form.proposedChange = currentBody">恢复为当前版本</el-button>
        </div>
        <div class="body-preview">{{ currentBody }}</div>
      </div>

      <el-form-item label="改进后的完整内容 (Markdown)">
        <el-input
          v-model="form.proposedChange"
          type="textarea"
          :rows="10"
          placeholder="选择 Skill 后会自动带入当前内容，请在此基础上修改。"
        />
      </el-form-item>

      <el-form-item label="改进理由">
        <el-input
          v-model="form.reason"
          type="textarea"
          :rows="3"
          placeholder="说明触发场景、改动点和预期收益。"
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
.version-panel {
  margin-bottom: 16px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--panel);
}

.version-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--line);
  font-size: 13px;
  font-weight: 600;
}

.body-preview {
  max-height: 220px;
  overflow-y: auto;
  padding: 12px;
  background: rgba(148, 163, 184, .08);
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
  color: var(--muted);
  font-family: Consolas, monospace;
}
</style>
