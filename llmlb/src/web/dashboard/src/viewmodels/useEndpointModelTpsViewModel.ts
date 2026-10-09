import { useQuery } from '@tanstack/react-query'
import { endpointsApi, type ModelTpsEntry } from '@/lib/api/endpoints'
import { queryKeys } from '@/lib/queryKeys'
import { useInvalidateOn } from '@/hooks/useInvalidateOn'

export interface EndpointModelTpsViewModel {
  tpsEntries: ModelTpsEntry[] | undefined
  isLoading: boolean
}

export function useEndpointModelTpsViewModel(endpointId: string,
  options: { enabled?: boolean; refetchInterval?: number } = {},
): EndpointModelTpsViewModel {
  const { data: tpsEntries, isLoading } = useQuery({
    queryKey: queryKeys.endpointModelTps(endpointId),
    queryFn: () => endpointsApi.getModelTps(endpointId),
    ...options,
  })
  useInvalidateOn(['tps'], queryKeys.endpointModelTps(endpointId), { id: endpointId })
  return { tpsEntries, isLoading }
}
