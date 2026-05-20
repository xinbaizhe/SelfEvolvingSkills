<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { useAdminStore } from '../../stores/useAdminStore'

const store = useAdminStore()
const dialogVisible = ref(false)
const newUsername = ref('')
const newPassword = ref('')
const editingUser = ref<any>(null)

onMounted(() => store.loadUsers())

function openCreate() {
  newUsername.value = ''
  newPassword.value = ''
  editingUser.value = null
  dialogVisible.value = true
}

async function saveUser() {
  if (editingUser.value) {
    const data: Record<string, any> = {}
    if (newPassword.value) data.password = newPassword.value
    await store.editUser(editingUser.value.id, data)
  } else {
    const res = await store.addUser(newUsername.value, newPassword.value)
    if (res.success) {
      ElMessage.success('用户创建成功')
    } else {
      ElMessage.error(res.error || '创建失败')
    }
  }
  dialogVisible.value = false
}

async function toggleActive(user: any) {
  await store.editUser(user.id, { is_active: !user.is_active })
  ElMessage.success(user.is_active ? '已停用' : '已激活')
}

async function removeUser(user: any) {
  await store.removeUser(user.id)
  ElMessage.success('已删除')
}
</script>

<template>
  <div>
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px">
      <h2>用户管理</h2>
      <el-button type="primary" @click="openCreate">添加用户</el-button>
    </div>

    <el-table :data="store.users" stripe>
      <el-table-column prop="id" label="ID" width="60" />
      <el-table-column prop="username" label="用户名" />
      <el-table-column prop="is_active" label="状态" width="100">
        <template #default="{ row }">
          <el-tag :type="row.is_active ? 'success' : 'danger'">{{ row.is_active ? '启用' : '停用' }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="created_at" label="创建时间" width="180" />
      <el-table-column label="操作" width="180">
        <template #default="{ row }">
          <el-button size="small" @click="toggleActive(row)">{{ row.is_active ? '停用' : '启用' }}</el-button>
          <el-button size="small" type="danger" @click="removeUser(row)">删除</el-button>
        </template>
      </el-table-column>
    </el-table>

    <el-dialog v-model="dialogVisible" :title="editingUser ? '编辑用户' : '添加用户'" width="400px">
      <el-form>
        <el-form-item label="用户名">
          <el-input v-model="newUsername" placeholder="请输入用户名" :disabled="!!editingUser" />
        </el-form-item>
        <el-form-item label="密码">
          <el-input v-model="newPassword" type="password" placeholder="请输入密码" show-password />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" @click="saveUser">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>
