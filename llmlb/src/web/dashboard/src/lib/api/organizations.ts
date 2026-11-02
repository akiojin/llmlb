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
  organization_id?: string | null
  organization_role?: 'admin' | 'member' | null
  is_active?: boolean
}

export const organizationsApi = {
  list: () => fetchWithAuth<{ organizations: Organization[] }>('/api/organizations').then((r) => r.organizations),
  create: (name: string) => fetchWithAuth<Organization>('/api/organizations', { method: 'POST', body: JSON.stringify({ name }) }),
  rename: (id: string, name: string) => fetchWithAuth<Organization>(`/api/organizations/${id}`, { method: 'PATCH', body: JSON.stringify({ name }) }),
  archive: (id: string) => fetchWithAuth<void>(`/api/organizations/${id}`, { method: 'DELETE' }),
  members: (id: string) => fetchWithAuth<{ users: OrganizationAccount[] }>(`/api/organizations/${id}/members`).then((r) => r.users),
  setMembership: (id: string, organization_id: string | null, organization_role: 'admin' | 'member') =>
    fetchWithAuth<void>(`/api/users/${id}/membership`, { method: 'PUT', body: JSON.stringify({ organization_id, organization_role }) }),
}
