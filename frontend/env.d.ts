/// <reference types="vite/client" />

declare module '*.vue' {
  import type { DefineComponent } from 'vue'
  const component: DefineComponent<{}, {}, any>
  export default component
}

interface Window {
  electronAPI?: {
    platform: string
    isElectron: boolean
    apiBaseUrl: string
    apiToken: string
    selectDirectory?: () => Promise<string | null>
  }
  __TAURI_INTERNALS__?: unknown
}
