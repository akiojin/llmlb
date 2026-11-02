import { fetchWithAuth } from './client'

export interface Organization {
  id: string
  name: string
  created_at: string
  updated_at: string
  archived_at: string | null
}

export interface OrganizationAccount {
  id: string
  username: string
  role: string
  organization_id: string | null
  organization_role: 'admin' | 'member' | null
  disabled: boolean
}

export interface MembershipUpdate {
  organization_id: string | null
  organization_role: 'admin' | 'member'
}

export interface OrganizationKeyRequest {
  name: string
  usage_account_id: string
  organization_id: string
  permissions: ['openai.inference', 'openai.models.read']
}

export const organizationsApi = {
  list: () => fetchWithAuth<{ organizations: Organization[] }>('/api/organizations').then((data) => data.organizations),
  create: (name: string) => fetchWithAuth<Organization>('/api/organizations', { method: 'POST', body: JSON.stringify({ name }) }),
  rename: (id: string, name: string) => fetchWithAuth<Organization>(`/api/organizations/${encodeURIComponent(id)}`, {
    method: 'PUT', body: JSON.stringify({ name }),
  }),
  archive: (id: string) => fetchWithAuth<void>(`/api/organizations/${encodeURIComponent(id)}`, { method: 'DELETE' }),
  accounts: () => fetchWithAuth<{ users: OrganizationAccount[] }>('/api/users').then((data) => data.users),
  setMembership: (id: string, data: MembershipUpdate) => fetchWithAuth<OrganizationAccount>(`/api/users/${encodeURIComponent(id)}/membership`, {
    method: 'PUT', body: JSON.stringify(data),
  }),
  setDisabled: (id: string, disabled: boolean) => fetchWithAuth<OrganizationAccount>(`/api/users/${encodeURIComponent(id)}/disabled`, {
    method: 'PUT', body: JSON.stringify({ disabled }),
  }),
  issueKey: (data: OrganizationKeyRequest) => fetchWithAuth<{ id: string; key: string }>('/api/me/api-keys', {
    method: 'POST', body: JSON.stringify(data),
  }),
}
