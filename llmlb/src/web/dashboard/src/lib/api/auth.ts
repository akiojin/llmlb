// Auth API

import { createApiErrorFromResponse, fetchWithAuth, API_BASE } from './client'

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
      throw await createApiErrorFromResponse(response)
    }

    return response.json()
  },

  logout: async () => {
    await fetchWithAuth('/api/auth/logout', { method: 'POST' })
  },

  me: () =>
    fetchWithAuth<{ user_id: string; username: string; role: string; must_change_password: boolean }>('/api/auth/me'),

  changePassword: async (currentPassword: string, newPassword: string) => {
    await fetchWithAuth('/api/auth/change-password', {
      method: 'PUT',
      body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }),
    })
  },

  forgotPassword: async (email: string) => {
    const response = await fetch(`${API_BASE}/api/auth/forgot-password`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email }),
    })

    if (!response.ok) {
      throw await createApiErrorFromResponse(response)
    }
  },

  resetPassword: async (token: string, newPassword: string) => {
    const response = await fetch(`${API_BASE}/api/auth/reset-password`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ token, new_password: newPassword }),
    })

    if (!response.ok) {
      throw await createApiErrorFromResponse(response)
    }
  },

  register: async (data: RegisterRequest): Promise<RegisterResponse> => {
    const response = await fetch(`${API_BASE}/api/auth/register`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    })

    if (!response.ok) {
      throw await createApiErrorFromResponse(response)
    }

    return response.json()
  },
}
