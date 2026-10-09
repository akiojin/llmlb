import { useCallback, useState } from 'react'
import { useQuery, type UseQueryResult } from '@tanstack/react-query'
import { dashboardApi } from '@/lib/api/dashboard'
import { systemApi } from '@/lib/api/system'
import type { DashboardOverview, RequestResponsesPage } from '@/lib/api/dashboard'
import type { DashboardEndpoint } from '@/lib/api/endpoints'
import type { SystemInfo } from '@/lib/api/system'
import { queryKeys } from '@/lib/queryKeys'
import { useInvalidateOn } from '@/hooks/useInvalidateOn'

export interface DashboardDataViewModel extends Pick<UseQueryResult<DashboardOverview>,
  'data' | 'isLoading' | 'error' | 'refetch'
> {
  systemInfo: SystemInfo | undefined
  requestResponsesData: RequestResponsesPage | undefined
  isLoadingHistory: boolean
  endpointsData: DashboardEndpoint[] | undefined
  isLoadingEndpoints: boolean
  lastRefreshed: Date | null
  fetchTimeMs: number | null
}

/** The dashboard's notified queries; remaining page state migrates in T007. */
export function useDashboardDataViewModel({ pollingInterval, isViewer }: {
  pollingInterval: number
  isViewer: boolean
}): DashboardDataViewModel {
  const [lastRefreshed, setLastRefreshed] = useState<Date | null>(null)
  const [fetchTimeMs, setFetchTimeMs] = useState<number | null>(null)
  const fetchWithTiming = useCallback(async () => {
    const started = performance.now()
    const result = await dashboardApi.getOverview()
    setFetchTimeMs(Math.round(performance.now() - started))
    setLastRefreshed(new Date())
    return result
  }, [])

  const { data, isLoading, error, refetch } = useQuery({
    queryKey: queryKeys.dashboardOverview(),
    queryFn: fetchWithTiming,
    refetchInterval: pollingInterval,
  })
  useInvalidateOn(['endpoints', 'metrics'], queryKeys.dashboardOverview())

  const { data: systemInfo } = useQuery({
    queryKey: queryKeys.systemInfo(),
    queryFn: () => systemApi.getSystem(),
    refetchInterval: pollingInterval,
    enabled: !isViewer,
  })
  useInvalidateOn(['system'], queryKeys.systemInfo())

  const { data: requestResponsesData, isLoading: isLoadingHistory } = useQuery({
    queryKey: queryKeys.requestResponses(),
    queryFn: () => dashboardApi.getRequestResponses({ limit: 100 }),
    refetchInterval: pollingInterval,
    enabled: !isViewer,
  })
  useInvalidateOn(['endpoints'], queryKeys.requestResponses())

  const { data: endpointsData, isLoading: isLoadingEndpoints } = useQuery({
    queryKey: queryKeys.dashboardEndpoints(),
    queryFn: () => dashboardApi.getEndpoints(),
    refetchInterval: pollingInterval,
    enabled: !isViewer,
  })
  useInvalidateOn(['endpoints'], queryKeys.dashboardEndpoints())

  return {
    data, isLoading, error, refetch, systemInfo, requestResponsesData,
    isLoadingHistory, endpointsData, isLoadingEndpoints, lastRefreshed, fetchTimeMs,
  }
}
