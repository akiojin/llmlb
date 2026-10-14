import { useQuery } from '@tanstack/react-query'
import { endpointsApi, type ModelTpsEntry } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { useEndpointModelTpsViewModel } from './useEndpointModelTpsViewModel'

function formatApiKindLabel(apiKind: ModelTpsEntry['api_kind']): string {
  switch (apiKind) {
    case 'chat_completions':
      return 'chat'
    case 'completions':
      return 'completion'
    case 'responses':
      return 'responses'
    default:
      return apiKind
  }
}

function formatTps(tps: number | null): string {
  if (tps == null) return '-'
  return `${tps.toFixed(1)} tok/s`
}

export interface ModelEndpointStatsViewModel {
  totalRequests: string
  successfulRequests: string
  failedRequests: string
  modelTpsSummary: string
  playgroundHref: string
}

/** Preserve provider polling for stats and the composed TPS hook's scoped subscription. */
export function useModelEndpointStatsViewModel(endpointId: string, modelId: string): ModelEndpointStatsViewModel {
  const { data: stats } = useQuery({
    queryKey: queryKeys.endpointModelStats(endpointId),
    queryFn: () => endpointsApi.getModelStats(endpointId),
  })
  const { tpsEntries } = useEndpointModelTpsViewModel(endpointId)

  const modelStat = stats?.find((s) => s.model_id === modelId)
  const totalRequests = modelStat?.total_requests ?? 0
  const successfulRequests = modelStat?.successful_requests ?? 0
  const failedRequests = modelStat?.failed_requests ?? 0
  const modelTps = (tpsEntries ?? [])
    .filter((entry) => entry.model_id === modelId && entry.source === 'production')
    .sort((a, b) => a.api_kind.localeCompare(b.api_kind))
  const modelTpsSummary =
    modelTps.length > 0
      ? modelTps
          .map((entry) => `${formatApiKindLabel(entry.api_kind)} ${formatTps(entry.tps)}`)
          .join(' | ')
      : '-'

  return {
    totalRequests: totalRequests.toLocaleString(),
    successfulRequests: successfulRequests.toLocaleString(),
    failedRequests: failedRequests.toLocaleString(),
    modelTpsSummary,
    playgroundHref: `#playground/${endpointId}`,
  }
}
