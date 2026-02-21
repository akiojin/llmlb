// API Keys API

import { fetchWithAuth } from './client'

export type ApiKeyPermission =
  | 'openai.inference'
  | 'openai.models.read'
  | 'endpoints.read'
  | 'endpoints.manage'
  | 'api_keys.manage'
  | 'users.manage'
  | 'invitations.manage'
  | 'models.manage'
  | 'registry.read'
  | 'logs.read'
  | 'metrics.read'

export interface ApiKey {
  id: string
  name: string
  key_prefix?: string
  created_by?: string
  created_at: string
  expires_at?: string
  last_used_at?: string
  permissions: ApiKeyPermission[]
}

export interface CreateApiKeyResponse {
  id: string
  name: string
  key: string // Full key only returned on creation
  key_prefix: string
  created_at: string
  expires_at?: string
  permissions: ApiKeyPermission[]
}

export const apiKeysApi = {
  list: () =>
    fetchWithAuth<{ api_keys: ApiKey[] }>('/api/me/api-keys').then(
      (res) => res.api_keys
    ),

  create: (data: {
    name: string
    expires_at?: string
  }) =>
    fetchWithAuth<CreateApiKeyResponse>('/api/me/api-keys', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  update: (id: string, data: { name?: string; expires_at?: string | null }) =>
    fetchWithAuth<ApiKey>(`/api/me/api-keys/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    }),

  delete: (id: string) =>
    fetchWithAuth<void>(`/api/me/api-keys/${id}`, { method: 'DELETE' }),
}
