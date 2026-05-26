export function getErrorMessage(e: unknown, fallback = '操作失败'): string {
  return e instanceof Error ? e.message : fallback
}
