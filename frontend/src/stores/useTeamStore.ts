import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import {
  loginTeam as apiLogin,
  logoutTeam as apiLogout,
  getTeamSession,
  type TeamSession,
} from '../api/team'

export const useTeamStore = defineStore('team', () => {
  const authenticated = ref(false)
  const username = ref('')
  const user_id = ref(0)
  const dept_name = ref('')
  const dept_id = ref(0)
  const server_url = ref('')
  const loading = ref(false)
  const error = ref('')

  const isAuthenticated = computed(() => authenticated.value)

  async function login(serverUrl: string, user: string, password: string): Promise<boolean> {
    loading.value = true
    error.value = ''
    try {
      const session = await apiLogin(serverUrl, user, password)
      setSession(session)
      return true
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
      return false
    } finally {
      loading.value = false
    }
  }

  async function logout() {
    try {
      await apiLogout()
    } catch { /* ignore */ }
    clearSession()
  }

  async function tryRestoreSession(): Promise<boolean> {
    try {
      const session = await getTeamSession()
      if (session) {
        setSession(session)
        return true
      }
    } catch { /* no saved session */ }
    return false
  }

  function setSession(session: TeamSession) {
    authenticated.value = true
    username.value = session.username
    user_id.value = session.user_id
    dept_name.value = session.dept_name
    dept_id.value = session.dept_id
    server_url.value = session.server_url
  }

  function clearSession() {
    authenticated.value = false
    username.value = ''
    user_id.value = 0
    dept_name.value = ''
    dept_id.value = 0
    server_url.value = ''
  }

  return {
    authenticated, username, user_id, dept_name, dept_id,
    server_url, loading, error, isAuthenticated,
    login, logout, tryRestoreSession,
  }
})
