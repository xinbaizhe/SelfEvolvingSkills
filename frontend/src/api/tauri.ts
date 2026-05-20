import { invoke } from '@tauri-apps/api/core'

export function api<T = unknown>(
  method: string,
  path: string,
  params?: Record<string, unknown> | null,
  body?: Record<string, unknown> | null,
): Promise<T> {
  return invoke<T>('api_request', {
    method,
    path,
    params: params ?? null,
    body: body ?? null,
  })
}

export default { api }
