import i18n from '../i18n'

// Byte units (B/KB/MB/GB) are the same in both languages, so formatBytes needs
// no translation and is deliberately left alone.

export function formatBytes(bytes: number | null | undefined): string {
  const value = Number(bytes ?? 0)
  if (!value) return '-'
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`
  if (value < 1024 * 1024 * 1024) return `${(value / 1024 / 1024).toFixed(2)} MB`
  return `${(value / 1024 / 1024 / 1024).toFixed(2)} GB`
}

export function formatUptime(seconds: number | null | undefined): string {
  const total = Number(seconds ?? 0)
  if (!total) return '-'
  const days = Math.floor(total / 86400)
  const hours = Math.floor((total % 86400) / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  // Reads the global composer rather than a scoped `useI18n()`: this is a plain
  // module with no setup context. The global `t` still tracks the locale ref, so
  // a caller rendering the result inside a template re-renders on a switch.
  if (days > 0) return i18n.global.t('common.uptime.daysHours', { days, hours })
  if (hours > 0) return i18n.global.t('common.uptime.hoursMinutes', { hours, minutes })
  return i18n.global.t('common.uptime.minutes', { minutes })
}
