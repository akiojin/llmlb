import { useQuery } from '@tanstack/react-query'
import { endpointsApi, type DashboardEndpoint } from '@/lib/api/endpoints'
import { queryKeys } from '@/lib/queryKeys'
import { useInvalidateOn } from '@/hooks/useInvalidateOn'

export interface EndpointViewModel {
  endpoint: DashboardEndpoint | undefined
  isLoadingEndpoint: boolean
}

export function useEndpointViewModel(endpointId: string): EndpointViewModel {
  const { data: endpoint, isLoading: isLoadingEndpoint } = useQuery({
    queryKey: queryKeys.endpoint(endpointId),
    queryFn: () => endpointsApi.get(endpointId),
  })
  useInvalidateOn(['endpoints'], queryKeys.endpoint(endpointId), { id: endpointId })
  return { endpoint, isLoadingEndpoint }
}
