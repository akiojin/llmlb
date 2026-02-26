// Auth API

import { ApiError, fetchWithAuth, API_BASE } from './client'

export interface RegisterRequest {
  invitation_code: string
  username: string
  password: string
}

export interface RegisterResponse {
  id: string
  username: string
  role: string
  created_at: string
}

export const authApi = {
  login: async (username: string, password: string) => {
    const response = await fetch(`${API_BASE}/api/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
      credentials: 'include',
    })

    if (!response.ok) {
      throw new ApiError(response.status, response.statusText)
    }

    return response.json()
  },

  logout: async () => {
    await fetchWithAuth('/api/auth/logout', { method: 'POST' })
  },

  me: () =>
    fetchWithAuth<{ user_id: string; username: string; role: string; must_change_password: boolean }>('/api/auth/me'),

  changePassword: async (newPassword: string) => {
    await fetchWithAuth('/api/auth/change-password', {
      method: 'PUT',
      body: JSON.stringify({ new_password: newPassword }),
    })
  },

  register: async (data: RegisterRequest): Promise<RegisterResponse> => {
    const response = await fetch(`${API_BASE}/api/auth/register`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    })

    if (!response.ok) {
      const errorText = await response.text()
      throw new ApiError(response.status, response.statusText, errorText)
    }

    return response.json()
  },
}
