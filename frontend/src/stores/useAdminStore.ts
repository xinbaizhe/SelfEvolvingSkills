import { defineStore } from 'pinia'
import { ref } from 'vue'
import { login as apiLogin, fetchUsers, createUser, updateUser, deleteUser } from '../api/admin'
import type { AdminUser } from '../types/admin'

export const useAdminStore = defineStore('admin', () => {
  const authenticated = ref(false)
  const username = ref('')
  const users = ref<AdminUser[]>([])

  async function login(username_: string, password: string) {
    const res = await apiLogin(username_, password)
    if (res.success && res.data) {
      authenticated.value = true
      username.value = res.data.username
    }
    return res
  }

  function logout() {
    authenticated.value = false
    username.value = ''
  }

  async function loadUsers() {
    const res = await fetchUsers()
    if (res.success && res.data) {
      users.value = res.data as AdminUser[]
    }
  }

  async function addUser(username_: string, password: string) {
    const res = await createUser(username_, password)
    if (res.success) await loadUsers()
    return res
  }

  async function editUser(id: number, data: Record<string, unknown>) {
    const res = await updateUser(id, data)
    if (res.success) await loadUsers()
    return res
  }

  async function removeUser(id: number) {
    const res = await deleteUser(id)
    if (res.success) await loadUsers()
    return res
  }

  return { authenticated, username, users, login, logout, loadUsers, addUser, editUser, removeUser }
})
