<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAdminStore } from '../../stores/useAdminStore'

const { t } = useI18n()
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
      error.value = t('admin.login.badCredentials')
    }
  } catch {
    error.value = t('admin.login.loginFailed')
  } finally {
    loading_.value = false
  }
}
</script>

<template>
  <div style="display: flex; justify-content: center; align-items: center; min-height: 60vh">
    <el-card style="width: 400px">
      <template #header><h3 style="text-align: center">{{ t('admin.login.title') }}</h3></template>
      <el-form @submit.prevent="doLogin">
        <el-form-item :label="t('admin.common.username')">
          <el-input v-model="username" :placeholder="t('admin.common.usernamePlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('admin.common.password')">
          <el-input v-model="password" type="password" :placeholder="t('admin.common.passwordPlaceholder')" show-password />
        </el-form-item>
        <el-form-item v-if="error">
          <span style="color: #f56c6c">{{ error }}</span>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" :loading="loading_" style="width: 100%" @click="doLogin">{{ t('admin.login.submit') }}</el-button>
        </el-form-item>
      </el-form>
    </el-card>
  </div>
</template>
