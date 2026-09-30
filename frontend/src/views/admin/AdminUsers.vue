<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { useAdminStore } from '../../stores/useAdminStore'
import type { AdminUser } from '../../types/admin'

const { t } = useI18n()
const store = useAdminStore()
const dialogVisible = ref(false)
const newUsername = ref('')
const newPassword = ref('')
const editingUser = ref<AdminUser | null>(null)

onMounted(() => store.loadUsers())

function openCreate() {
  newUsername.value = ''
  newPassword.value = ''
  editingUser.value = null
  dialogVisible.value = true
}

async function saveUser() {
  if (editingUser.value) {
    const data: Record<string, unknown> = {}
    if (newPassword.value) data.password = newPassword.value
    await store.editUser(editingUser.value.id, data)
  } else {
    const res = await store.addUser(newUsername.value, newPassword.value)
    if (res.success) {
      ElMessage.success(t('admin.users.createSuccess'))
    } else {
      ElMessage.error(res.error || t('admin.users.createFailed'))
    }
  }
  dialogVisible.value = false
}

async function toggleActive(user: AdminUser) {
  await store.editUser(user.id, { is_active: !user.is_active })
  ElMessage.success(user.is_active ? t('admin.users.deactivated') : t('admin.users.activated'))
}

async function removeUser(user: AdminUser) {
  await store.removeUser(user.id)
  ElMessage.success(t('admin.users.deleted'))
}
</script>

<template>
  <div>
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px">
      <h2>{{ t('admin.users.title') }}</h2>
      <el-button type="primary" @click="openCreate">{{ t('admin.users.addUser') }}</el-button>
    </div>

    <el-table :data="store.users" stripe>
      <el-table-column prop="id" label="ID" width="60" />
      <el-table-column prop="username" :label="t('admin.common.username')" />
      <el-table-column prop="is_active" :label="t('admin.common.statusLabel')" width="100">
        <template #default="{ row }">
          <el-tag :type="row.is_active ? 'success' : 'danger'">{{ row.is_active ? t('admin.users.enable') : t('admin.users.disable') }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="created_at" :label="t('admin.common.created')" width="180" />
      <el-table-column :label="t('admin.common.action')" width="180">
        <template #default="{ row }">
          <el-button size="small" @click="toggleActive(row)">{{ row.is_active ? t('admin.users.disable') : t('admin.users.enable') }}</el-button>
          <el-button size="small" type="danger" @click="removeUser(row)">{{ t('common.delete') }}</el-button>
        </template>
      </el-table-column>
    </el-table>

    <el-dialog v-model="dialogVisible" :title="editingUser ? t('admin.users.editUser') : t('admin.users.addUser')" width="400px">
      <el-form>
        <el-form-item :label="t('admin.common.username')">
          <el-input v-model="newUsername" :placeholder="t('admin.common.usernamePlaceholder')" :disabled="!!editingUser" />
        </el-form-item>
        <el-form-item :label="t('admin.common.password')">
          <el-input v-model="newPassword" type="password" :placeholder="t('admin.common.passwordPlaceholder')" show-password />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" @click="saveUser">{{ t('common.save') }}</el-button>
      </template>
    </el-dialog>
  </div>
</template>
