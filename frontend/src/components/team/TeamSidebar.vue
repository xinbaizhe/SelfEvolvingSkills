<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from 'vue'
import { useTeamStore } from '../../stores/useTeamStore'
import { checkTeamConnection, type ConnectionStatus } from '../../api/team'
import LoginDialog from './LoginDialog.vue'

defineProps<{ collapsed: boolean }>()

const store = useTeamStore()
const loginDialog = ref<InstanceType<typeof LoginDialog> | null>(null)
const connectionStatus = ref<ConnectionStatus | null>(null)
let pollTimer: ReturnType<typeof setInterval> | null = null

async function checkConnection() {
  if (!store.isAuthenticated) {
    connectionStatus.value = null
    return
  }
  try {
    connectionStatus.value = await checkTeamConnection()
  } catch {
    connectionStatus.value = { connected: false, authenticated: false }
  }
}

function startPolling() {
  stopPolling()
  if (store.isAuthenticated) {
    checkConnection()
    pollTimer = setInterval(checkConnection, 30_000)
  }
}

function stopPolling() {
  if (pollTimer) {
    clearInterval(pollTimer)
    pollTimer = null
  }
}

watch(() => store.isAuthenticated, () => {
  if (store.isAuthenticated) {
    startPolling()
  } else {
    stopPolling()
    connectionStatus.value = null
  }
})

onMounted(() => {
  if (store.isAuthenticated) startPolling()
})

onUnmounted(() => stopPolling())

async function handleLogout() {
  stopPolling()
  connectionStatus.value = null
  await store.logout()
}

function statusColor(): string {
  if (!connectionStatus.value) return '#475569'
  if (connectionStatus.value.authenticated) return '#22c55e'
  if (connectionStatus.value.connected) return '#f59e0b'
  return '#ef4444'
}

function statusTitle(): string {
  if (!connectionStatus.value) return '检查中...'
  if (connectionStatus.value.authenticated) return '已连接'
  if (connectionStatus.value.connected) return 'Token 已过期'
  return '无法连接服务器'
}
</script>

<template>
  <LoginDialog ref="loginDialog" @logged-in="() => {}" />

  <div class="team-sidebar" v-show="!collapsed">
    <template v-if="!store.isAuthenticated">
      <button class="team-login-btn" @click="loginDialog?.open()">
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none"><rect x="1.5" y="4.5" width="13" height="9" rx="1.5" stroke="currentColor" stroke-width="1.2"/><circle cx="8" cy="9" r="1.5" fill="currentColor"/><path d="M5.5 4.5V3a2 2 0 012-2h1a2 2 0 012 2v1.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/></svg>
        登录团队版
      </button>
    </template>
    <template v-else>
      <div class="team-user">
        <div class="team-user-avatar">{{ store.username.charAt(0).toUpperCase() }}</div>
        <div class="team-user-info">
          <div class="team-user-name">{{ store.username }}</div>
          <div class="team-user-dept">{{ store.dept_name }}</div>
        </div>
        <span
          class="connection-dot"
          :style="{ background: statusColor() }"
          :title="statusTitle()"
        ></span>
        <button class="team-logout-btn" @click="handleLogout" title="退出登录">
          <svg width="12" height="12" viewBox="0 0 16 16" fill="none"><path d="M6 2H3a1 1 0 00-1 1v10a1 1 0 001 1h3M11 11l4-3-4-3M15 8H6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
        </button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.team-sidebar {
  padding: 12px;
  border-top: 1px solid rgba(148,163,184,.16);
  border-bottom: 1px solid rgba(148,163,184,.16);
}

.team-login-btn {
  width: 100%;
  padding: 10px 12px;
  background: rgba(45,212,191,.08);
  border: 1px solid rgba(45,212,191,.22);
  color: #94a3b8;
  border-radius: 8px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  font-size: 13px;
  transition: .15s;
}
.team-login-btn:hover {
  background: rgba(45,212,191,.16);
  color: #ecfeff;
  border-color: rgba(45,212,191,.42);
}

.team-user {
  display: flex;
  align-items: center;
  gap: 10px;
}

.team-user-avatar {
  width: 32px;
  height: 32px;
  border-radius: 8px;
  background: rgba(45,212,191,.24);
  color: #2dd4bf;
  display: grid;
  place-items: center;
  font-weight: 700;
  font-size: 14px;
  flex-shrink: 0;
}

.team-user-info {
  flex: 1;
  min-width: 0;
}

.team-user-name {
  font-size: 13px;
  font-weight: 600;
  color: #e2e8f0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.team-user-dept {
  font-size: 11px;
  color: #64748b;
  margin-top: 2px;
}

.connection-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
  transition: background .3s;
}

.team-logout-btn {
  background: transparent;
  border: 1px solid rgba(148,163,184,.14);
  color: #64748b;
  padding: 4px;
  border-radius: 6px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  transition: .15s;
}
.team-logout-btn:hover {
  color: #f87171;
  border-color: rgba(248,113,113,.3);
}
</style>