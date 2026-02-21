// Users API

import { fetchWithAuth } from './client'

export interface User {
  id: string
  username: string
  role: 'admin' | 'user'
  created_at: string
}

export const usersApi = {
  list: async (): Promise<User[]> => {
    const res = await fetchWithAuth<{ users: User[] }>('/api/users')
    return res.users
  },

  create: (data: { username: string; password: string; role: string }) =>
    fetchWithAuth<User>('/api/users', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  update: (
    id: string,
    data: { username?: string; password?: string; role?: string }
  ) =>
    fetchWithAuth<User>(`/api/users/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    }),

  delete: (id: string) =>
    fetchWithAuth<void>(`/api/users/${id}`, { method: 'DELETE' }),
}
