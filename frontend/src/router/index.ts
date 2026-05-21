import { createRouter, createWebHashHistory } from 'vue-router'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', redirect: '/admin' },
    { path: '/dashboard', name: 'dashboard', component: () => import('../views/DashboardView.vue') },
    { path: '/workbench', name: 'skillsWorkbench', component: () => import('../views/SkillsWorkbenchView.vue') },
    { path: '/resources', name: 'resourcesConfig', component: () => import('../views/ResourceConfigView.vue') },
    { path: '/sources', name: 'sources', component: () => import('../views/SourcesView.vue') },
    { path: '/workflows', name: 'workflows', component: () => import('../views/WorkflowsView.vue') },
    { path: '/drafts', name: 'drafts', component: () => import('../views/SkillDraftsView.vue') },
    { path: '/skills', name: 'skills', component: () => import('../views/SkillsView.vue') },
    { path: '/garden', name: 'garden', component: () => import('../views/SkillGardenView.vue') },
    { path: '/agents', name: 'agents', component: () => import('../views/AgentsView.vue') },
    { path: '/conversations', name: 'conversations', component: () => import('../views/ConversationsView.vue') },
    { path: '/scan', redirect: '/admin' },
    { path: '/export', redirect: '/skills' },
    { path: '/community', name: 'community', component: () => import('../views/CommunitySkillsView.vue') },
    { path: '/admin/login', name: 'adminLogin', component: () => import('../views/admin/AdminLogin.vue') },
    { path: '/admin', name: 'adminDashboard', component: () => import('../views/admin/AdminDashboard.vue') },
    { path: '/admin/users', name: 'adminUsers', component: () => import('../views/admin/AdminUsers.vue') },
    { path: '/admin/config', name: 'adminConfig', component: () => import('../views/admin/AdminConfig.vue') },
  ],
})

export default router
