import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import {
  clientsApi,
  type ClientIpRanking,
  type ClientRankingResponse,
  type UniqueIpTimelinePoint,
  type ModelDistribution,
  type HeatmapCell,
} from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'

export interface ClientsTabViewModel {
  isLoading: boolean
  page: number
  perPage: number
  ipFilter: string | undefined
  ipFilterLabel: string | null
  rankings: ClientIpRanking[]
  totalCount: number
  timelineData: UniqueIpTimelinePoint[]
  modelsData: ModelDistribution[]
  heatmapData: HeatmapCell[]
  setPage: (page: number) => void
  clearIpFilter: () => void
}

export function useClientsTabViewModel(): ClientsTabViewModel {
  const [page, setPage] = useState(1)
  const perPage = 20
  const ipFilter = useMemo(() => {
    const params = new URLSearchParams(window.location.search)
    return params.get('ip') || undefined
  }, [])

  // These client statistics retain provider polling and have no WS resource dependency.
  const { data, isLoading } = useQuery<ClientRankingResponse>({
    queryKey: queryKeys.clientRanking(page, perPage, ipFilter),
    queryFn: () => clientsApi.getClientRanking({ page, per_page: perPage, ip: ipFilter }),
  })
  const { data: timelineData } = useQuery<UniqueIpTimelinePoint[]>({
    queryKey: queryKeys.clientTimeline(),
    queryFn: () => clientsApi.getTimeline(),
  })
  const { data: modelsData } = useQuery<ModelDistribution[]>({
    queryKey: queryKeys.clientModels(),
    queryFn: () => clientsApi.getModels(),
  })
  const { data: heatmapData } = useQuery<HeatmapCell[]>({
    queryKey: queryKeys.clientHeatmap(ipFilter),
    queryFn: () => clientsApi.getHeatmap({ ip: ipFilter }),
  })

  const clearIpFilter = () => {
    const params = new URLSearchParams(window.location.search)
    params.delete('ip')
    window.location.search = params.toString()
  }

  return {
    isLoading, page, perPage, ipFilter,
    ipFilterLabel: ipFilter ? `Filtered: ${ipFilter}` : null,
    rankings: data?.rankings ?? [],
    totalCount: data?.total_count ?? 0,
    timelineData: timelineData ?? [],
    modelsData: modelsData ?? [],
    heatmapData: heatmapData ?? [],
    setPage, clearIpFilter,
  }
}
