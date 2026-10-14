import { useCallback, useEffect } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import type { DashboardResource } from '@/lib/dashboardResources'
import type { DashboardQueryKey } from '@/lib/queryKeys'
import { registerDashboardSubscription, type DashboardSubscriptionScope } from './dashboardSubscriptions'

/**
 * Declare WS dependencies and return a refresh command for this query only.
 * Mutation-only caches use an empty resource list to preserve their polling.
 * The command captures its owner so an in-flight action can refresh after unmount.
 */
export function useInvalidateOn(
  resources: readonly DashboardResource[],
  queryKey: DashboardQueryKey,
  { id }: DashboardSubscriptionScope = {},
): () => Promise<void> {
  const queryClient = useQueryClient()
  useEffect(
    () => registerDashboardSubscription(queryClient, resources, queryKey, { id }),
    [queryClient, resources, queryKey, id],
  )
  return useCallback(() => queryClient.invalidateQueries({ queryKey }), [queryClient, queryKey])
}
