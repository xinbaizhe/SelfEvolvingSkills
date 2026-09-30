import { invoke } from '@tauri-apps/api/core'

export const LOCALES = ['en', 'zh-CN'] as const

export type AppLocale = (typeof LOCALES)[number]

export const DEFAULT_LOCALE: AppLocale = 'en'

const STORAGE_KEY = 'ses.locale'

export function isAppLocale(value: unknown): value is AppLocale {
  return typeof value === 'string' && (LOCALES as readonly string[]).includes(value)
}

/**
 * Maps an arbitrary locale tag to one we ship. Anything Chinese-script becomes
 * zh-CN; everything else falls back to English. Returns null when there is
 * nothing usable to map, so callers can fall through to the next source.
 */
export function normalizeLocale(tag: string | null | undefined): AppLocale | null {
  const lower = tag?.trim().toLowerCase()
  if (!lower) return null
  return lower.startsWith('zh') ? 'zh-CN' : 'en'
}

/** The language the user explicitly picked, if they ever did. */
export function readStoredLocale(): AppLocale | null {
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    return isAppLocale(stored) ? stored : null
  } catch {
    // Storage can be unavailable (disabled, or a locked-down webview).
    return null
  }
}

export function storeLocale(locale: AppLocale): void {
  try {
    localStorage.setItem(STORAGE_KEY, locale)
  } catch {
    // Not being able to remember the choice is not worth failing over.
  }
}

/**
 * Synchronous first guess, used to render the very first frame without waiting
 * on the backend. `navigator.language` is a decent approximation on Windows and
 * macOS; on Linux it often ignores LANG, which is what resolveSystemLocale()
 * corrects a moment later.
 */
export function initialLocale(): AppLocale {
  return readStoredLocale() ?? normalizeLocale(navigator.language) ?? DEFAULT_LOCALE
}

/**
 * The OS locale, read in Rust. Deliberately not from the webview: on Linux
 * `navigator.language` frequently does not reflect LANG/LC_*, and that is the
 * exact case this feature exists to handle.
 *
 * Returns null when unavailable - notably in plain `vite dev`, where there is
 * no Tauri backend to call.
 */
export async function resolveSystemLocale(): Promise<AppLocale | null> {
  try {
    return normalizeLocale(await invoke<string>('get_system_locale'))
  } catch {
    return null
  }
}
