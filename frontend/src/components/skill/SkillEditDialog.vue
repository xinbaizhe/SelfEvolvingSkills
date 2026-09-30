<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { updateSkill } from '../../api/skills'
import { getErrorMessage } from '../../utils/error'
import type { SkillItem } from '../../api/skills'

const { t } = useI18n()

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
    if (!res.success) throw new Error(res.error || t('workbench.common.updateFailed'))
    ElMessage.success(t('workbench.skillEdit.updated'))
    emit('update:visible', false)
    emit('saved')
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, t('workbench.common.updateFailed')))
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <el-dialog v-model="dialogVisible" :title="t('workbench.skillEdit.title')" width="520px" top="10vh" @closed="editOriginalName = ''">
    <el-form label-position="top">
      <el-form-item :label="t('workbench.common.field.name')">
        <el-input v-model="editForm.name" :placeholder="t('workbench.skillEdit.namePlaceholder')" />
      </el-form-item>
      <el-form-item :label="t('workbench.common.field.description')">
        <el-input v-model="editForm.description" type="textarea" :rows="3" :placeholder="t('workbench.common.descriptionPlaceholder')" />
      </el-form-item>
      <el-form-item :label="t('workbench.common.field.category')">
        <el-input v-model="editForm.category" :placeholder="t('workbench.skillEdit.categoryPlaceholder')" />
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="emit('update:visible', false)">{{ t('common.cancel') }}</el-button>
      <el-button type="primary" :loading="saving" @click="saveEdit">{{ t('common.save') }}</el-button>
    </template>
  </el-dialog>
</template>
