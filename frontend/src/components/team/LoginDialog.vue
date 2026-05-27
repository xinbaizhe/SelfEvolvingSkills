<script setup lang="ts">
import { reactive, ref } from 'vue'
import { useTeamStore } from '../../stores/useTeamStore'

const store = useTeamStore()
const visible = ref(false)
const form = reactive({ serverUrl: 'http://localhost:8080', username: '', password: '' })
const loading = ref(false)
const localError = ref('')

const emit = defineEmits<{
  (e: 'loggedIn'): void
}>()

function open() {
  visible.value = true
  localError.value = ''
  form.password = ''
}

function close() {
  visible.value = false
  localError.value = ''
}

async function doLogin() {
  if (!form.serverUrl || !form.username || !form.password) {
    localError.value = '请填写完整信息'
    return
  }
  loading.value = true
  localError.value = ''
  try {
    const ok = await store.login(form.serverUrl, form.username, form.password)
    if (ok) {
      visible.value = false
      emit('loggedIn')
    } else {
      localError.value = store.error || '登录失败'
    }
  } finally {
    loading.value = false
  }
}

defineExpose({ open })
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="login-overlay" @click.self="close" @keydown.escape="close">
      <div class="login-panel">
        <button class="close-btn" @click="close" title="关闭">
          <svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <path d="M5 5l10 10M15 5l-10 10" />
          </svg>
        </button>

        <div class="panel-left">
          <div class="brand-area">
            <div class="brand-icon">
              <svg width="40" height="40" viewBox="0 0 40 40" fill="none">
                <rect x="2" y="2" width="36" height="36" rx="10" stroke="currentColor" stroke-width="1.5" opacity="0.3"/>
                <circle cx="14" cy="14" r="4" fill="currentColor" opacity="0.7"/>
                <circle cx="26" cy="14" r="4" fill="currentColor" opacity="0.7"/>
                <circle cx="20" cy="26" r="4" fill="currentColor" opacity="0.7"/>
                <path d="M14 18v4M26 18v4M16 26h8" stroke="currentColor" stroke-width="1.2" opacity="0.5"/>
              </svg>
            </div>
            <h2>团队版登录</h2>
            <p>连接到团队服务器，同步共享 Skills 和模型配置</p>
          </div>
          <div class="tips">
            <div class="tip-item">
              <span class="tip-num">1</span>
              <span>确保团队服务器已启动</span>
            </div>
            <div class="tip-item">
              <span class="tip-num">2</span>
              <span>输入管理员提供的账号密码</span>
            </div>
            <div class="tip-item">
              <span class="tip-num">3</span>
              <span>登录后可浏览、安装、分享 Skills</span>
            </div>
          </div>
        </div>

        <div class="panel-right">
          <form class="login-form" @submit.prevent="doLogin">
            <div class="form-group">
              <label>服务器地址</label>
              <input
                v-model="form.serverUrl"
                type="text"
                placeholder="http://192.168.1.100:8080"
                autocomplete="url"
              />
            </div>
            <div class="form-group">
              <label>用户名</label>
              <input
                v-model="form.username"
                type="text"
                placeholder="输入用户名"
                autocomplete="username"
              />
            </div>
            <div class="form-group">
              <label>密码</label>
              <input
                v-model="form.password"
                type="password"
                placeholder="输入密码"
                autocomplete="current-password"
                @keyup.enter="doLogin"
              />
            </div>

            <div v-if="localError" class="form-error">{{ localError }}</div>

            <div class="form-actions">
              <button type="button" class="btn-cancel" @click="close">取消</button>
              <button type="submit" class="btn-submit" :disabled="loading">
                {{ loading ? '登录中...' : '登录' }}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.login-overlay {
  position: fixed;
  inset: 0;
  z-index: 3000;
  display: grid;
  place-items: center;
  background: rgba(15, 23, 42, 0.65);
  backdrop-filter: blur(6px);
  -webkit-backdrop-filter: blur(6px);
  animation: fadeIn 0.2s ease;
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

.login-panel {
  position: relative;
  display: grid;
  grid-template-columns: 320px 360px;
  background: #fff;
  border-radius: 16px;
  box-shadow:
    0 24px 80px rgba(15, 23, 42, 0.32),
    0 0 0 1px rgba(15, 23, 42, 0.08);
  overflow: hidden;
  animation: slideUp 0.25s ease;
  max-width: 95vw;
}

@keyframes slideUp {
  from { opacity: 0; transform: translateY(20px) scale(0.97); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

.close-btn {
  position: absolute;
  top: 14px;
  right: 14px;
  z-index: 2;
  width: 32px;
  height: 32px;
  border: 0;
  background: transparent;
  color: #94a3b8;
  border-radius: 8px;
  cursor: pointer;
  display: grid;
  place-items: center;
  transition: .15s;
}
.close-btn:hover {
  background: #f1f5f9;
  color: #475569;
}

.panel-left {
  padding: 40px 32px;
  background: linear-gradient(165deg, #0b1a2e 0%, #0f2744 40%, #0d1f38 100%);
  color: #e2e8f0;
  display: flex;
  flex-direction: column;
  justify-content: center;
}

.brand-area {
  text-align: center;
  margin-bottom: 32px;
}

.brand-icon {
  color: #38bdf8;
  margin-bottom: 16px;
  display: inline-flex;
}

.brand-area h2 {
  margin: 0 0 8px;
  font-size: 22px;
  font-weight: 700;
  color: #f8fafc;
}

.brand-area p {
  margin: 0;
  font-size: 13px;
  color: #94a3b8;
  line-height: 1.55;
}

.tips {
  display: grid;
  gap: 12px;
}

.tip-item {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 13px;
  color: #94a3b8;
}

.tip-num {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: rgba(56, 189, 248, 0.15);
  color: #38bdf8;
  display: grid;
  place-items: center;
  font-size: 11px;
  font-weight: 700;
  flex-shrink: 0;
}

.panel-right {
  padding: 40px 36px;
  display: flex;
  flex-direction: column;
  justify-content: center;
}

.login-form {
  display: grid;
  gap: 20px;
}

.form-group {
  display: grid;
  gap: 6px;
}

.form-group label {
  font-size: 13px;
  font-weight: 600;
  color: #334155;
}

.form-group input {
  width: 100%;
  padding: 10px 14px;
  border: 1.5px solid #e2e8f0;
  border-radius: 10px;
  font-size: 14px;
  color: #1e293b;
  background: #f8fafc;
  outline: none;
  transition: .15s;
}

.form-group input::placeholder {
  color: #94a3b8;
}

.form-group input:focus {
  border-color: #38bdf8;
  box-shadow: 0 0 0 3px rgba(56, 189, 248, 0.1);
  background: #fff;
}

.form-error {
  padding: 8px 12px;
  background: #fef2f2;
  border: 1px solid #fecaca;
  border-radius: 8px;
  color: #dc2626;
  font-size: 13px;
}

.form-actions {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
  padding-top: 4px;
}

.btn-cancel,
.btn-submit {
  padding: 10px 20px;
  border-radius: 10px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: .15s;
  border: 0;
}

.btn-cancel {
  background: #f1f5f9;
  color: #475569;
}
.btn-cancel:hover {
  background: #e2e8f0;
}

.btn-submit {
  background: linear-gradient(135deg, #0ea5e9, #0284c7);
  color: #fff;
  box-shadow: 0 4px 14px rgba(14, 165, 233, 0.3);
}
.btn-submit:hover {
  box-shadow: 0 6px 20px rgba(14, 165, 233, 0.42);
  transform: translateY(-1px);
}
.btn-submit:disabled {
  opacity: 0.6;
  cursor: not-allowed;
  transform: none;
}

@media (max-width: 720px) {
  .login-panel {
    grid-template-columns: 1fr;
    max-width: 92vw;
  }
  .panel-left {
    padding: 28px 24px;
    text-align: center;
  }
  .panel-right {
    padding: 28px 24px;
  }
}
</style>
