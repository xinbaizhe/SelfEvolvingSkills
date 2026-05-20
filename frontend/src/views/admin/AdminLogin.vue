<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAdminStore } from '../../stores/useAdminStore'

const router = useRouter()
const store = useAdminStore()
const username = ref('admin')
const password = ref('')
const error = ref('')
const loading_ = ref(false)

async function doLogin() {
  error.value = ''
  loading_.value = true
  try {
    const res = await store.login(username.value, password.value)
    if (res.success) {
      router.push('/admin')
    } else {
      error.value = '用户名或密码错误'
    }
  } catch {
    error.value = '登录失败'
  } finally {
    loading_.value = false
  }
}
</script>

<template>
  <div style="display: flex; justify-content: center; align-items: center; min-height: 60vh">
    <el-card style="width: 400px">
      <template #header><h3 style="text-align: center">管理员登录</h3></template>
      <el-form @submit.prevent="doLogin">
        <el-form-item label="用户名">
          <el-input v-model="username" placeholder="请输入用户名" />
        </el-form-item>
        <el-form-item label="密码">
          <el-input v-model="password" type="password" placeholder="请输入密码" show-password />
        </el-form-item>
        <el-form-item v-if="error">
          <span style="color: #f56c6c">{{ error }}</span>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" :loading="loading_" style="width: 100%" @click="doLogin">登 录</el-button>
        </el-form-item>
      </el-form>
    </el-card>
  </div>
</template>
