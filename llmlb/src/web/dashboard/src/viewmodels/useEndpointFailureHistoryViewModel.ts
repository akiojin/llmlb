import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useAuth } from '@/hooks/useAuth'
import { useInvalidateOn } from '@/hooks/useInvalidateOn'
import { dashboardApi, type RequestHistoryItem } from '@/lib/api/dashboard'
import { ApiError } from '@/lib/api/client'
import { queryKeys } from '@/lib/queryKeys'

/** Fetch retained failures on demand; lifetime counters need not have matching logs. */
export function useEndpointFailureHistoryViewModel(endpointId: string) {
  const { user } = useAuth()
  const canRead = !!user && user.role !== 'viewer'
  const [page, setPage] = useState(1)
  const key = queryKeys.endpointFailures(endpointId, page)
  const { data, isPending, error, refetch } = useQuery({
    queryKey: key,
    queryFn: () => dashboardApi.getRequestResponses({ endpoint_id: endpointId, status: 'error', page, per_page: 25 }),
    enabled: canRead,
    retry: false,
  })
  useInvalidateOn(['endpoints'], key)
  const unavailableReason = !canRead || (error instanceof ApiError && error.status === 403)
    ? 'You do not have permission to view request history.'
    : error ? 'Request history could not be loaded. Retry to check the recorded failure reasons.' : null
  const history: RequestHistoryItem[] = (data?.records ?? []).map(record => ({
    request_id: record.id, timestamp: record.timestamp, model: record.model,
    node_id: record.endpoint_id, node_name: record.endpoint_name, status: record.status.type,
    duration_ms: record.duration_ms,
    error: record.status.type === 'error' ? record.status.message : undefined,
    request_body: record.request_body, response_body: record.response_body, client_ip: record.client_ip,
  }))
  return {
    history, page, setPage, totalPages: Math.ceil((data?.total_count ?? 0) / 25),
    isLoading: isPending && canRead, unavailableReason,
    isEmpty: !!data && !error && data.total_count === 0,
    isEmptyPage: !!data && !error && data.total_count > 0 && data.records.length === 0,
    canRetry: canRead && !(error instanceof ApiError && error.status === 403),
    retry: () => { if (canRead) void refetch() },
  }
}
