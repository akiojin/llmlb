import { fetchWithAuth } from './client'
import type { User } from './users'

export interface RegistrationRequest {
  id: string
  username: string
  created_at: string
}

export const registrationRequestsApi = {
  list: () => fetchWithAuth<RegistrationRequest[]>('/api/registration-requests'),
  approve: (id: string, role: 'admin' | 'viewer') =>
    fetchWithAuth<User>(`/api/registration-requests/${id}/approve`, {
      method: 'POST',
      body: JSON.stringify({ role }),
    }),
  reject: (id: string) =>
    fetchWithAuth<void>(`/api/registration-requests/${id}`, { method: 'DELETE' }),
}
