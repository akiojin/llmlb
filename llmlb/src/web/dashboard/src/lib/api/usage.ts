import { fetchWithAuth } from './client'

export type UsageGrouping = 'organization' | 'account'

export interface UsageFilters {
  group_by: UsageGrouping
  /** UTC calendar days, both endpoints inclusive in the public API. */
  from: string
  to: string
  organization_id?: string
  account_id?: string
  page: number
  per_page: number
}

export interface UsageTotals {
  request_count: number
  input_tokens: number
  output_tokens: number
  total_tokens: number
  measured_requests: number
  estimated_requests: number
  missing_requests: number
  pending_requests?: number
}

export interface UsageRow extends UsageTotals {
  organization_id: string | null
  organization_name: string | null
  organization_archived: boolean
  account_id: string | null
  account_name: string | null
  account_deleted: boolean
}

export interface UsagePage {
  group_by: UsageGrouping
  from: string
  to: string
  page: number
  per_page: number
  total: number
  rows: UsageRow[]
  /** Totals include every scoped row across all pages. */
  totals: UsageTotals
  coverage_start: string | null
}

export const usageApi = {
  list: (filters: UsageFilters) => fetchWithAuth<UsagePage>('/api/dashboard/usage', { params: { ...filters } }),
}
