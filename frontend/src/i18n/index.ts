import { createI18n } from 'vue-i18n'

// Domain-scoped locale files: kept split by feature so each stays reviewable,
// and so a feature area can be translated on its own.
import enCommon from './locales/en/common.json'
import enCore from './locales/en/core.json'
import enAdmin from './locales/en/admin.json'
import enTeam from './locales/en/team.json'
import enVuln from './locales/en/vuln.json'
import enWorkbench from './locales/en/workbench.json'

import zhCommon from './locales/zh-CN/common.json'
import zhCore from './locales/zh-CN/core.json'
import zhAdmin from './locales/zh-CN/admin.json'
import zhTeam from './locales/zh-CN/team.json'
import zhVuln from './locales/zh-CN/vuln.json'
import zhWorkbench from './locales/zh-CN/workbench.json'

import {
  DEFAULT_LOCALE,
  initialLocale,
  readStoredLocale,
  resolveSystemLocale,
  storeLocale,
  type AppLocale,
} from './detect'

export type { AppLocale }
export { LOCALES, isAppLocale } from './detect'
export { readStoredLocale, storeLocale } from './detect'

const messages = {
  en: { ...enCommon, ...enCore, ...enAdmin, ...enTeam, ...enVuln, ...enWorkbench },
  'zh-CN': { ...zhCommon, ...zhCore, ...zhAdmin, ...zhTeam, ...zhVuln, ...zhWorkbench },
}

const i18n = createI18n({
  // Composition API mode; `legacy: false` is what makes `useI18n()` work and
  // `locale` a reactive ref.
  legacy: false,
  globalInjection: true,
  locale: initialLocale(),
  fallbackLocale: DEFAULT_LOCALE,
  messages,
})

/** Reactive current locale, for components that need to react to a switch. */
export const locale = i18n.global.locale

/**
 * Keeps the document's `lang` attribute in step with the UI language, so screen
 * readers and font fallback behave. index.html ships `lang="en"` as the default.
 */
export function applyDocumentLang(next: AppLocale): void {
  document.documentElement.lang = next
}

/** Switch language at runtime and remember the choice. */
export function setLocale(next: AppLocale): void {
  locale.value = next
  storeLocale(next)
  applyDocumentLang(next)
}

/**
 * Refines the synchronous first guess using the OS locale. Skipped when the
 * user has already made an explicit choice - that always wins.
 *
 * Called after mount rather than before, so the first frame renders immediately
 * instead of waiting on an IPC round trip.
 */
export async function refineLocaleFromSystem(): Promise<void> {
  if (readStoredLocale()) return
  const detected = await resolveSystemLocale()
  if (detected && detected !== locale.value) {
    locale.value = detected
    applyDocumentLang(detected)
  }
}

export default i18n
