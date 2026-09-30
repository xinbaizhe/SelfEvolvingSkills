import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import App from './App.vue'
import router from './router'
import i18n, { applyDocumentLang, locale, refineLocaleFromSystem } from './i18n'

const app = createApp(App)
app.use(createPinia())
app.use(router)
app.use(i18n)
// Element Plus's own locale is deliberately NOT pinned here - App.vue supplies
// it reactively through ElConfigProvider so it follows the language switcher.
app.use(ElementPlus, { size: 'default' })
app.mount('#app')

applyDocumentLang(locale.value)
// Refines the first guess with the OS locale, without blocking the first paint.
void refineLocaleFromSystem()
