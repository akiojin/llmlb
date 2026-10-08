import { useEffect } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import type { DashboardResource } from '@/lib/dashboardResources'
import type { DashboardQueryKey } from '@/lib/queryKeys'
import { registerDashboardSubscription, type DashboardSubscriptionScope } from './dashboardSubscriptions'

/** Declare a query's resources and optional detail scope for this mounted hook. */
export function useInvalidateOn(
  resources: readonly DashboardResource[],
  queryKey: DashboardQueryKey,
  { id }: DashboardSubscriptionScope = {},
): void {
  const queryClient = useQueryClient()
  useEffect(
    () => registerDashboardSubscription(queryClient, resources, queryKey, { id }),
    [queryClient, resources, queryKey, id],
  )
}
