import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useAuth } from '@/hooks/useAuth'
import { usageApi, type UsageRow } from '@/lib/api/usage'
import { apiKeysApi } from '@/lib/api/api-keys'

function currentUtcMonth() {
  const now = new Date()
  return { from: `${now.toISOString().slice(0, 7)}-01`,
    to: new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth() + 1, 0)).toISOString().slice(0, 10) }
}

function usageLabel(row: UsageRow, groupBy: 'organization' | 'account'): string {
  if (groupBy === 'organization') return row.organization_name ?? (row.organization_id ? 'Deleted organization' : 'Unattributed')
  if (row.account_name) return row.account_name
  if (!row.account_id) return 'Unattributed'
  return `${row.account_deleted ? 'Deleted' : 'Former'} account ${row.account_id.slice(0, 8)}`
}

export function useUsageViewModel() {
  const { user } = useAuth()
  const isGlobal = user?.role === 'admin'
  const [period, setPeriod] = useState(currentUtcMonth)
  const [groupBy, setGroupBy] = useState<'organization' | 'account'>(isGlobal ? 'organization' : 'account')
  const [organizationId, setOrganizationId] = useState<string | undefined>()
  const [page, setPage] = useState(1)
  const [detail, setDetail] = useState<UsageRow | null>(null)
  const validPeriod = !!period.from && !!period.to && period.from <= period.to
  const query = { ...period, group_by: groupBy, organization_id: organizationId, page, per_page: 10 }
  const scopeIdentity = [user?.id, user?.scope, user?.organization_id, user?.organization_role]
  const usage = useQuery({ queryKey: ['usage', ...scopeIdentity, query], queryFn: () => usageApi.get(query),
    enabled: validPeriod && user?.scope !== 'unassigned' })
  const keys = useQuery({ queryKey: ['usage-detail-keys', ...scopeIdentity, detail?.account_id],
    queryFn: apiKeysApi.list, enabled: !!detail?.account_id })
  return {
    isGlobal, scope: user?.scope, data: usage.data, isLoading: usage.isLoading,
    rows: (usage.data?.rows ?? []).map((row) => ({ ...row, label: usageLabel(row, groupBy) })),
    detailTitle: detail ? usageLabel(detail, groupBy) : 'Usage detail',
    error: !validPeriod ? 'Choose a valid UTC date range.' : usage.error instanceof Error ? usage.error.message : null,
    period, groupBy, organizationId, page, detail,
    setDate: (field: 'from' | 'to', value: string) => { setPeriod((old) => ({ ...old, [field]: value })); setPage(1); setDetail(null) },
    setGroupBy: (value: 'organization' | 'account') => { setGroupBy(value); setOrganizationId(undefined); setPage(1); setDetail(null) },
    openRow: (row: UsageRow) => {
      if (groupBy === 'organization' && row.organization_id) { setOrganizationId(row.organization_id); setGroupBy('account'); setPage(1) }
      else setDetail(row)
    },
    clearOrganization: () => { setOrganizationId(undefined); setGroupBy('organization'); setPage(1); setDetail(null) },
    closeDetail: () => setDetail(null),
    detailKeys: keys.data?.filter((key) => key.usage_account_id === detail?.account_id && key.organization_id === detail?.organization_id) ?? [],
    detailKeysError: keys.error instanceof Error ? keys.error.message : null,
    previous: () => setPage((p) => Math.max(1, p - 1)), next: () => setPage((p) => p + 1),
    refresh: () => { void usage.refetch() },
  }
}
