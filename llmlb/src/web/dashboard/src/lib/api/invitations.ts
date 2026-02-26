// Invitations API

import { fetchWithAuth } from './client'

export interface Invitation {
  id: string
  created_by: string
  created_at: string
  expires_at: string
  status: 'active' | 'used' | 'revoked'
  used_by?: string
  used_at?: string
}

export interface CreateInvitationResponse {
  id: string
  code: string
  created_at: string
  expires_at: string
}

export const invitationsApi = {
  list: async (): Promise<Invitation[]> => {
    const res = await fetchWithAuth<{ invitations: Invitation[] }>('/api/invitations')
    return res.invitations
  },

  create: (expiresInHours?: number) =>
    fetchWithAuth<CreateInvitationResponse>('/api/invitations', {
      method: 'POST',
      body: JSON.stringify({ expires_in_hours: expiresInHours }),
    }),

  revoke: (id: string) =>
    fetchWithAuth<void>(`/api/invitations/${id}`, { method: 'DELETE' }),
}
