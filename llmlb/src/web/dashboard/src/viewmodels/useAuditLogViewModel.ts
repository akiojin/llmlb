import { useCallback, useEffect, useRef, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useAuth } from '@/hooks/useAuth'
import {
  auditLogApi,
  type AuditLogEntry,
  type AuditLogFilters,
  type HashChainVerifyResult,
} from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { formatRelativeTime } from '@/lib/utils'

export type AuditLogSelectFilter = 'actor_type' | 'http_method' | 'status_code'

export interface AuditLogRow extends Pick<AuditLogEntry,
  'id' | 'http_method' | 'request_path' | 'status_code' | 'actor_type' | 'client_ip'
> {
  timestampLabel: string
  actorLabel: string
  clientHref: string | null
  durationLabel: string
  tokenLabel: string
}

export interface AuditLogVerificationViewModel {
  isVerifying: boolean
  result: HashChainVerifyResult | null
  error: string | null
  verify: () => Promise<void>
}

export interface AuditLogViewModel {
  canView: boolean
  entries: AuditLogRow[]
  isLoading: boolean
  filters: AuditLogFilters
  searchText: string
  currentPage: number
  totalPages: number
  totalCount: number | undefined
  changePage: (page: number) => void
  changeSearch: (value: string) => void
  changeFilter: (key: AuditLogSelectFilter, value: string) => void
  verification: AuditLogVerificationViewModel
}

export function useAuditLogViewModel(): AuditLogViewModel {
  const { user } = useAuth()
  const canView = user?.role === 'admin'
  const [filters, setFilters] = useState<AuditLogFilters>({ page: 1, per_page: 50 })
  const [searchText, setSearchText] = useState('')
  const searchTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const [isVerifying, setIsVerifying] = useState(false)
  const [result, setResult] = useState<HashChainVerifyResult | null>(null)
  const [error, setError] = useState<string | null>(null)

  const { data, isLoading } = useQuery({
    queryKey: queryKeys.auditLogs(filters),
    queryFn: () => auditLogApi.list(filters),
    enabled: canView,
  })

  useEffect(() => () => {
    if (searchTimer.current) clearTimeout(searchTimer.current)
  }, [])

  const changePage = useCallback((page: number) => {
    setFilters((previous) => ({ ...previous, page }))
  }, [])

  const changeSearch = useCallback((value: string) => {
    setSearchText(value)
    if (searchTimer.current) clearTimeout(searchTimer.current)
    searchTimer.current = setTimeout(() => {
      setFilters({ ...filters, search: value || undefined, page: 1 })
      searchTimer.current = null
    }, 300)
  }, [filters])

  const changeFilter = useCallback((key: AuditLogSelectFilter, value: string) => {
    // Select values are text; the API's status_code filter is a number.
    const nextValue = value === 'all' ? undefined : key === 'status_code' ? Number(value) : value
    setFilters({ ...filters, [key]: nextValue, page: 1 })
  }, [filters])

  const verify = async () => {
    if (!canView) return
    setIsVerifying(true)
    setError(null)
    try {
      setResult(await auditLogApi.verify())
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : 'Verification failed')
    } finally {
      setIsVerifying(false)
    }
  }

  const entries = (data?.items ?? []).map((entry): AuditLogRow => ({
    id: entry.id,
    http_method: entry.http_method,
    request_path: entry.request_path,
    status_code: entry.status_code,
    actor_type: entry.actor_type,
    client_ip: entry.client_ip,
    timestampLabel: formatRelativeTime(entry.timestamp),
    actorLabel: entry.actor_username || entry.actor_id || '-',
    clientHref: entry.client_ip ? `?tab=clients&ip=${encodeURIComponent(entry.client_ip)}` : null,
    durationLabel: entry.duration_ms != null ? `${entry.duration_ms}ms` : '-',
    tokenLabel: entry.total_tokens != null ? entry.total_tokens.toLocaleString() : '-',
  }))

  return {
    canView,
    entries,
    isLoading,
    filters,
    searchText,
    currentPage: filters.page || 1,
    totalPages: data ? Math.ceil(data.total / (filters.per_page || 50)) : 0,
    totalCount: data?.total,
    changePage,
    changeSearch,
    changeFilter,
    verification: { isVerifying, result, error, verify },
  }
}
