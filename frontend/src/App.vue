<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useRoute } from 'vue-router'
import { check } from '@tauri-apps/plugin-updater'
import { ElMessageBox } from 'element-plus'
import AppSidebar from './components/layout/AppSidebar.vue'

const route = useRoute()
const sidebarCollapsed = ref(false)
const appStartedAt = Date.now()
const runtime = ref('0s')
let runtimeTimer: ReturnType<typeof setInterval> | null = null

function toggleSidebar() {
  sidebarCollapsed.value = !sidebarCollapsed.value
}

async function checkUpdate() {
  // 开发模式下跳过更新检查（没有 latest.json 会 404）
  if (import.meta.env.DEV) return
  try {
    const update = await check({ timeout: 8000 })
    if (update) {
      const action = await ElMessageBox.confirm(
        `发现新版本 ${update.version}，是否立即更新？`,
        '更新提示',
        { confirmButtonText: '立即更新', cancelButtonText: '稍后再说', type: 'info' }
      ).catch(() => 'cancel')
      if (action === 'confirm') {
        await update.downloadAndInstall()
      }
    }
  } catch (_) {
    // 静默失败，不影响正常使用
  }
}

onMounted(() => {
  checkUpdate()
  runtimeTimer = setInterval(() => {
    const diff = Math.floor((Date.now() - appStartedAt) / 1000)
    const h = Math.floor(diff / 3600)
    const m = Math.floor((diff % 3600) / 60)
    const s = diff % 60
    runtime.value = h > 0 ? `${h}h ${m}m ${s}s` : m > 0 ? `${m}m ${s}s` : `${s}s`
  }, 1000)
})

onUnmounted(() => {
  if (runtimeTimer) clearInterval(runtimeTimer)
})
</script>

<template>
  <div class="app" :class="{ 'sidebar-collapsed': sidebarCollapsed }">
    <aside class="sidebar">
      <div class="brand">
        <div class="mark" aria-hidden="true">
          <span class="mark-node n1"></span>
          <span class="mark-node n2"></span>
          <span class="mark-node n3"></span>
          <span class="mark-node n4"></span>
          <span class="mark-node n5"></span>
        </div>
        <div v-show="!sidebarCollapsed">
          <strong>Self Evolving Skills</strong>
          <span>从重复工作流中沉淀本地 Skills</span>
        </div>
        <button class="sidebar-toggle" @click="toggleSidebar" :title="sidebarCollapsed ? '展开侧边栏' : '收起侧边栏'">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <path d="M6 3L10 8L6 13" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </button>
      </div>
      <AppSidebar :collapsed="sidebarCollapsed" />
      <section v-show="!sidebarCollapsed" class="privacy-notice">
        <h3>本地隐私模式</h3>
        <p>扫描、索引和分析均在本机完成。导出敏感内容前会显式确认，默认减少路径和正文暴露。</p>
      </section>
      <section v-show="!sidebarCollapsed" class="about-section">
        <a href="https://github.com/xinbaizhe/SelfEvolvingSkills" target="_blank" title="GitHub">
          <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/></svg>
          GitHub
        </a>
        <span v-show="!sidebarCollapsed" class="version">v1.0.3</span>
      </section>
    </aside>
    <main>
      <header class="topbar">
        <div class="title">
          <h1>Self Evolving Skills</h1>
          <p>运行时长 {{ runtime }} · 从重复工作流中沉淀可复用 Skills</p>
        </div>
        <div class="actions">
          <router-link to="/admin" class="btn ghost">扫描本机</router-link>
          <router-link to="/workbench" class="btn primary">生成 Skill</router-link>
        </div>
      </header>
      <router-view v-slot="{ Component }">
        <Transition name="fade" mode="out-in">
          <component :is="Component" :key="route.fullPath" />
        </Transition>
      </router-view>
    </main>
  </div>
</template>

<style>
:root {
  --bg: #f0f4f8;
  --panel: #ffffff;
  --ink: #111827;
  --muted: #667085;
  --line: #e2e8f0;
  --blue: #0d9488;
  --blue-light: #14b8a6;
  --green: #2dd4bf;
  --orange: #d98612;
  --red: #d64f4f;
  --violet: #7c3aed;
  --violet-light: #a78bfa;
  --amber: #f59e0b;
  --shadow: 0 4px 24px rgba(0,0,0,.06);
  --shadow-hover: 0 12px 32px rgba(0,0,0,.08);
  --radius: 12px;
  --sidebar-width: 250px;
  --sidebar-collapsed-width: 62px;
}

* { box-sizing: border-box; }

body {
  margin: 0;
  background: var(--bg);
  color: var(--ink);
  font-family: Inter, "Segoe UI", "Microsoft YaHei", Arial, sans-serif;
  letter-spacing: 0;
}

button, .btn {
  font: inherit;
  cursor: pointer;
  text-decoration: none;
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.app {
  min-height: 100vh;
  display: grid;
  grid-template-columns: var(--sidebar-width) minmax(0, 1fr);
  transition: grid-template-columns 0.2s;
}

.app.sidebar-collapsed {
  grid-template-columns: var(--sidebar-collapsed-width) minmax(0, 1fr);
}

.sidebar {
  height: 100vh;
  position: sticky;
  top: 0;
  background:
    radial-gradient(circle at 24px 22px, rgba(45,212,191,.16), transparent 26px),
    linear-gradient(180deg, #07111f 0%, #0b1322 54%, #08101d 100%);
  color: #f8fafc;
  padding: 22px 14px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  overflow: hidden;
  transition: padding 0.2s;
  border-right: 1px solid rgba(148,163,184,.18);
}

.sidebar-collapsed .sidebar {
  padding: 22px 10px;
}

.brand {
  display: flex;
  gap: 10px;
  align-items: center;
  padding-bottom: 12px;
  border-bottom: 1px solid rgba(148,163,184,.16);
}

.mark {
  width: 38px;
  height: 38px;
  border-radius: 8px;
  background:
    linear-gradient(135deg, rgba(45,212,191,.16), rgba(56,189,248,.06)),
    #0f172a;
  position: relative;
  flex-shrink: 0;
  border: 1px solid rgba(45,212,191,.48);
  box-shadow: 0 0 0 1px rgba(15,118,110,.24), 0 12px 28px rgba(45,212,191,.12);
  overflow: hidden;
}

.mark:before {
  content: "";
  position: absolute;
  inset: 7px;
  border: 1px solid rgba(45,212,191,.22);
  border-radius: 6px;
}

.mark:after {
  content: "";
  position: absolute;
  left: 9px;
  right: 9px;
  top: 19px;
  height: 1px;
  background: linear-gradient(90deg, transparent, rgba(45,212,191,.95), transparent);
  box-shadow:
    8px -8px 0 rgba(45,212,191,.7),
    4px 8px 0 rgba(56,189,248,.62);
  transform: rotate(-24deg);
}

.mark-node {
  position: absolute;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #2dd4bf;
  box-shadow: 0 0 14px rgba(45,212,191,.85);
}

.mark-node.n1 { left: 9px; top: 12px; }
.mark-node.n2 { left: 21px; top: 8px; background: #7dd3fc; }
.mark-node.n3 { right: 8px; top: 18px; }
.mark-node.n4 { left: 15px; bottom: 8px; background: #7dd3fc; }
.mark-node.n5 { right: 12px; bottom: 10px; }

.sidebar-toggle {
  margin-left: auto;
  background: rgba(15,23,42,.8);
  border: 1px solid rgba(45,212,191,.22);
  color: #94a3b8;
  padding: 6px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 11px;
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: .15s;
  line-height: 1;
}
.sidebar-toggle:hover { color: #ecfeff; background: rgba(45,212,191,.12); border-color: rgba(45,212,191,.42); }
.sidebar-toggle svg { transition: transform .2s; }
.sidebar-collapsed .sidebar-toggle svg { transform: rotate(180deg); }

.brand strong { display: block; font-size: 14px; }
.brand span { display: block; margin-top: 2px; color: #a8b3c7; font-size: 11px; }

.privacy-notice {
  margin-top: auto;
  padding: 12px;
  border: 1px solid rgba(45,212,191,.18);
  border-radius: 8px;
  background: rgba(15,23,42,.72);
  box-shadow: inset 0 1px 0 rgba(255,255,255,.04);
}

.privacy-notice h3 { margin: 0 0 8px; font-size: 12px; }
.privacy-notice p { margin: 0; color: #a8b3c7; font-size: 11px; line-height: 1.5; }

.about-section {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 10px 0 4px;
}
.about-section a {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  color: #64748b;
  font-size: 11px;
  text-decoration: none;
  transition: color .15s;
}
.about-section a:hover { color: #a8b3c7; }
.about-section .version { color: #475569; font-size: 10px; }

main { min-width: 0; }

.topbar {
  min-height: 64px;
  position: sticky;
  top: 0;
  z-index: 10;
  background: rgba(255,255,255,.82);
  backdrop-filter: blur(18px);
  -webkit-backdrop-filter: blur(18px);
  border-bottom: 1px solid var(--line);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  padding: 0 28px;
}

.title h1 { margin: 0; font-size: 20px; }
.title p { margin: 4px 0 0; color: var(--muted); font-size: 13px; }
.actions { display: flex; gap: 10px; flex-wrap: wrap; justify-content: flex-end; }

.primary, .ghost { border-radius: 10px; cursor: pointer; padding: 10px 16px; display: inline-flex; align-items: center; gap: 8px; font-size: 14px; font-weight: 500; transition: all .2s; }
.primary { border: 0; color: #fff; background: linear-gradient(135deg, var(--blue), var(--blue-light)); box-shadow: 0 4px 16px rgba(13,148,136,.25); }
.primary:hover { box-shadow: 0 8px 24px rgba(13,148,136,.35); transform: translateY(-1px); }
.ghost { border: 1px solid var(--line); background: #fff; color: var(--ink); }
.ghost:hover { border-color: var(--blue); color: var(--blue); }

.chip {
  display: inline-flex;
  align-items: center;
  padding: 4px 10px;
  border-radius: 999px;
  background: #e8f7f5;
  color: var(--blue);
  font-size: 12px;
  font-weight: 500;
  white-space: nowrap;
}

.chip.green { background: #e9f8f3; color: #0c8265; }
.chip.orange { background: #fff5e4; color: #a56200; }
.chip.red { background: #fff0f0; color: var(--red); }
.chip.violet { background: #f0edff; color: var(--violet); }

.metrics {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 14px;
  margin-bottom: 20px;
}

.metric, .panel, .card {
  background: rgba(255,255,255,.85);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border: 1px solid var(--line);
  border-radius: var(--radius);
  box-shadow: var(--shadow);
  transition: all .25s;
}

.metric:hover, .card:hover {
  box-shadow: var(--shadow-hover);
  transform: translateY(-2px);
  border-color: rgba(13,148,136,.18);
}

.metric {
  padding: 20px;
  min-height: 118px;
}

.metric label {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  color: var(--muted);
  font-size: 12px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: .5px;
  margin-bottom: 14px;
}

.metric strong { display: block; font-size: 28px; line-height: 1; }
.metric small { display: block; color: var(--muted); margin-top: 12px; line-height: 1.45; }

.page-view {
  padding: 26px 28px 34px;
}

.grid {
  display: grid;
  grid-template-columns: minmax(0, 1.45fr) minmax(360px, .9fr);
  gap: 20px;
  align-items: start;
}

.two {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 20px;
  margin-top: 20px;
}

.cards {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 14px;
  margin-bottom: 20px;
}

.stack { display: grid; gap: 20px; }

.panel {
  overflow: hidden;
  box-shadow: var(--shadow);
  border-radius: var(--radius);
}

.head {
  padding: 20px;
  border-bottom: 1px solid var(--line);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.head h2 { margin: 0; font-size: 16px; }
.head p { margin: 4px 0 0; color: var(--muted); font-size: 12px; line-height: 1.45; }

.body { padding: 18px; display: grid; gap: 14px; }

.item {
  display: grid;
  grid-template-columns: 34px minmax(0, 1fr) 112px 86px;
  gap: 12px;
  align-items: center;
  padding: 14px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: rgba(255,255,255,.7);
  cursor: pointer;
  transition: .2s;
}

.item:hover, .item.active {
  border-color: rgba(13,148,136,.4);
  background: rgba(255,255,255,.95);
  box-shadow: 0 8px 24px rgba(13,148,136,.1);
  transform: translateY(-1px);
}

.rank {
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  background: #1f2937;
  color: #fff;
  border-radius: 8px;
  font-weight: 700;
}

.rank.high { background: #0f766e; }
.rank.med { background: var(--orange); }
.rank.low { background: var(--violet); }

.item h3 { margin: 0 0 6px; font-size: 14px; }
.item p { margin: 0; color: var(--muted); font-size: 12px; line-height: 1.45; }

.score { font-size: 12px; color: var(--muted); display: grid; gap: 6px; }

.bar {
  height: 8px;
  background: #e7ecf3;
  border-radius: 999px;
  overflow: hidden;
}

.bar span {
  display: block;
  height: 100%;
  background: linear-gradient(90deg, #2dd4bf, #0891b2);
}

.node {
  display: grid;
  grid-template-columns: 30px minmax(0, 1fr) auto;
  gap: 10px;
  align-items: center;
  padding: 11px;
  border: 1px solid #e6ebf2;
  border-radius: 8px;
  background: #fff;
}

.node-icon {
  width: 30px;
  height: 30px;
  border-radius: 7px;
  background: #e8f7f5;
  color: var(--blue);
  display: grid;
  place-items: center;
}

.node b { display: block; font-size: 13px; }
.node span { color: var(--muted); font-size: 12px; line-height: 1.45; }

.card {
  padding: 20px;
  min-height: 150px;
  display: grid;
  gap: 10px;
  cursor: pointer;
}

.card h3 { margin: 0; font-size: 15px; font-weight: 600; }
.card p { margin: 0; color: var(--muted); font-size: 13px; line-height: 1.55; }

.codebox {
  margin: 0;
  background: #111827;
  color: #d1d5db;
  border-radius: 8px;
  padding: 14px;
  overflow: auto;
  font-family: Consolas, "SFMono-Regular", monospace;
  font-size: 12px;
  line-height: 1.55;
}

.log { max-height: 352px; overflow: auto; }

.log-item {
  display: grid;
  grid-template-columns: 78px minmax(0, 1fr);
  gap: 12px;
  font-size: 12px;
  line-height: 1.5;
}

.log-item time { color: var(--muted); font-variant-numeric: tabular-nums; }
.log-item b { display: block; font-size: 13px; margin-bottom: 2px; }

table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

th, td {
  text-align: left;
  padding: 13px 16px;
  border-bottom: 1px solid var(--line);
  vertical-align: top;
}

th { color: var(--muted); font-size: 12px; background: #fafbfe; }

tbody tr { cursor: pointer; }
tbody tr:hover { background: #f7faff; }

td small { display: block; color: var(--muted); margin-top: 4px; line-height: 1.45; }

.meta { color: var(--muted); font-size: 12px; line-height: 1.5; }

.fade-enter-active, .fade-leave-active {
  transition: opacity .15s ease;
}

.fade-enter-from, .fade-leave-to {
  opacity: 0;
}

@media (max-width: 1120px) {
  .app { grid-template-columns: 1fr; }
  .sidebar { height: auto; position: static; }
  .grid, .two { grid-template-columns: 1fr; }
  .metrics { grid-template-columns: repeat(2, 1fr); }
  .cards { grid-template-columns: 1fr; }
}

@media (max-width: 720px) {
  .topbar { position: static; align-items: flex-start; flex-direction: column; padding: 18px; }
  .page-view { padding: 18px; }
  .metrics { grid-template-columns: 1fr; }
  .item { grid-template-columns: 34px minmax(0, 1fr); }
}
</style>
