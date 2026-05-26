<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { updateSkill } from '../../api/skills'
import { getErrorMessage } from '../../utils/error'
import type { SkillItem } from '../../api/skills'

const props = defineProps<{
  visible: boolean
  skill: SkillItem | null
}>()

const emit = defineEmits<{
  (e: 'update:visible', value: boolean): void
  (e: 'saved'): void
}>()

const saving = ref(false)
const editForm = reactive({ name: '', description: '', category: '' })

const dialogVisible = computed({
  get: () => props.visible,
  set: (value) => emit('update:visible', value),
})

const editOriginalName = ref('')

function open(skill: SkillItem) {
  editOriginalName.value = skill.name
  editForm.name = skill.name
  editForm.description = skill.description || ''
  editForm.category = skill.category || ''
}

defineExpose({ open })

async function saveEdit() {
  saving.value = true
  try {
    const res = await updateSkill(editOriginalName.value, {
      name: editForm.name || undefined,
      description: editForm.description || null,
      category: editForm.category || null,
    })
    if (!res.success) throw new Error(res.error || '更新失败')
    ElMessage.success('Skill 已更新')
    emit('update:visible', false)
    emit('saved')
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '更新失败'))
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <el-dialog v-model="dialogVisible" title="编辑 Skill" width="520px" top="10vh" @closed="editOriginalName = ''">
    <el-form label-position="top">
      <el-form-item label="名称">
        <el-input v-model="editForm.name" placeholder="Skill 名称" />
      </el-form-item>
      <el-form-item label="描述">
        <el-input v-model="editForm.description" type="textarea" :rows="3" placeholder="简要描述" />
      </el-form-item>
      <el-form-item label="分类">
        <el-input v-model="editForm.category" placeholder="如 frontend、backend、devops" />
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="emit('update:visible', false)">取消</el-button>
      <el-button type="primary" :loading="saving" @click="saveEdit">保存</el-button>
    </template>
  </el-dialog>
</template>
