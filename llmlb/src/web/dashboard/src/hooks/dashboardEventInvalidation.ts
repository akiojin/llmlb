import { queryKeys, type DashboardQueryKey } from '@/lib/queryKeys'
import type { DashboardChange, DashboardResource } from '@/lib/dashboardResources'

/** Temporary central rules during T004; replaced by subscriptions in T005. */
export const DASHBOARD_EVENT_INVALIDATIONS: {
  [T in DashboardResource]: (id: DashboardChange['id']) => DashboardQueryKey[]
} = {
  endpoints: (id) => [
    queryKeys.dashboardOverview(),
    queryKeys.dashboardEndpoints(),
    queryKeys.requestResponses(),
    // An unscoped notification also refreshes every endpoint detail query.
    ...(id ? [queryKeys.endpoint(id)] : [queryKeys.endpointPrefix()]),
  ],
  metrics: () => [queryKeys.dashboardOverview()],
  tps: (id) => [id ? queryKeys.endpointModelTps(id) : queryKeys.endpointModelTpsPrefix()],
  system: () => [queryKeys.systemInfo()],
}

/** Query keys to invalidate for the current wire resource. */
export function queryKeysToInvalidate(change: DashboardChange): DashboardQueryKey[] {
  if (!Object.prototype.hasOwnProperty.call(DASHBOARD_EVENT_INVALIDATIONS, change.changed)) return []
  return DASHBOARD_EVENT_INVALIDATIONS[change.changed](change.id)
}
