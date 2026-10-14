// Users API

import { fetchWithAuth } from './client'

export interface User {
  id: string
  username: string
  role: 'admin' | 'viewer'
  /** Destination for operational notifications. Not a login identifier. */
  email: string | null
  created_at: string
}

export interface CreateUserResponse {
  user: User
  generated_password: string
}

export const usersApi = {
  list: async (): Promise<User[]> => {
    const res = await fetchWithAuth<{ users: User[] }>('/api/users')
    return res.users
  },

  create: (data: { username: string; role: string; email?: string }) =>
    fetchWithAuth<CreateUserResponse>('/api/users', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  // `email: ''` clears the notification email; leaving it out keeps it.
  update: (
    id: string,
    data: { username?: string; password?: string; role?: string; email?: string }
  ) =>
    fetchWithAuth<User>(`/api/users/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    }),

  delete: (id: string) =>
    fetchWithAuth<void>(`/api/users/${id}`, { method: 'DELETE' }),
}
