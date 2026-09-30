<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useTeamStore } from '../../stores/useTeamStore'

defineProps<{ collapsed: boolean }>()

const route = useRoute()
const { t } = useI18n()
const teamStore = useTeamStore()

// Wrapped in computed so the labels re-evaluate when the language changes -
// as a plain array they would be frozen at setup time.
const baseMenuItems = computed(() => [
  { path: '/admin', title: t('nav.admin'), match: ['/admin', '/admin/login', '/admin/users'] },
  { path: '/admin/daily-report', title: t('nav.dailyReport'), match: ['/admin/daily-report'] },
  { path: '/workbench', title: t('nav.workbench'), match: ['/workbench', '/workflows', '/drafts', '/garden'] },
  { path: '/skills', title: t('nav.skills'), match: ['/skills'] },
  { path: '/agents', title: t('nav.agents'), match: ['/agents'] },
  { path: '/resources', title: t('nav.resources'), match: ['/resources', '/sources', '/admin/config'] },
  { path: '/community', title: t('nav.community'), match: ['/community'] },
  { path: '/team', title: t('nav.team'), match: ['/team'] },
  { path: '/vuln-scanner', title: t('nav.vulnScanner'), match: ['/vuln-scanner'] },
])

const menuItems = computed(() => {
  if (!teamStore.isAuthenticated) return baseMenuItems.value
  return [
    ...baseMenuItems.value,
    { path: '/team/models', title: t('nav.teamModels'), match: ['/team/models'] },
  ]
})

const activePath = computed(() => route.path)

function isActive(item: { match: string[] }) {
  return item.match.includes(activePath.value)
}
</script>

<template>
  <nav class="nav">
    <router-link
      v-for="item in menuItems"
      :key="item.path"
      :to="item.path"
      :class="{ active: isActive(item) }"
      :title="collapsed ? item.title : undefined"
    >
      <i></i>
      <span v-show="!collapsed">{{ item.title }}</span>
    </router-link>
  </nav>
</template>

<style scoped>
.nav {
  display: grid;
  gap: 4px;
}

.nav a {
  border: 0;
  background: transparent;
  color: #cbd5e1;
  text-align: left;
  padding: 10px 12px;
  border-radius: 8px;
  display: grid;
  grid-template-columns: 18px 1fr;
  gap: 10px;
  align-items: center;
  cursor: pointer;
  text-decoration: none;
  font-size: 14px;
  transition: .15s;
  overflow: hidden;
  white-space: nowrap;
}

.nav a.active, .nav a:hover {
  color: #ecfeff;
  background: rgba(45,212,191,.1);
  box-shadow: inset 0 0 0 1px rgba(45,212,191,.14);
}

.nav a i {
  width: 16px;
  height: 16px;
  border: 1.8px solid currentColor;
  border-radius: 4px;
  position: relative;
  flex-shrink: 0;
}

.nav a.active i:after {
  content: "";
  position: absolute;
  right: -4px;
  top: -4px;
  width: 7px;
  height: 7px;
  background: #2dd4bf;
  border: 2px solid #07111f;
  border-radius: 50%;
}
</style>
