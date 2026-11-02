import { fetchWithAuth } from './client'

export interface UsageCounts {
  request_count: number
  input_tokens: number
  output_tokens: number
  total_tokens: number
  measured_requests: number
  estimated_requests: number
  missing_requests: number
}

export interface UsageRow extends UsageCounts {
  organization_id: string | null
  account_id: string | null
  organization_name: string | null
  account_name: string | null
  organization_archived: boolean
  account_deleted: boolean
}

export interface UsageQuery {
  group_by: 'organization' | 'account'
  from: string
  to: string
  organization_id?: string
  account_id?: string
  page: number
  per_page: number
}

export interface UsagePage {
  scope: 'global' | 'organization' | 'account'
  group_by: 'organization' | 'account'
  from: string
  to: string
  to_exclusive: string
  rows: UsageRow[]
  totals: UsageCounts
  total_count: number
  page: number
  per_page: number
  coverage_start: string | null
}

export const usageApi = {
  get: (query: UsageQuery) => fetchWithAuth<UsagePage>('/api/dashboard/usage', { params: { ...query } }),
}
