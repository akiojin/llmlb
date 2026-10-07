import { queryKeys, type DashboardQueryKey } from '@/lib/queryKeys'
import type { DashboardEvent, DashboardEventType } from './useWebSocket'

function endpointLifecycleKeys(data: DashboardEvent['data']): DashboardQueryKey[] {
  const keys: DashboardQueryKey[] = [
    queryKeys.dashboardOverview(),
    queryKeys.dashboardEndpoints(),
    queryKeys.requestResponses(),
  ]
  // Endpoint detail used by the endpoint playground (EndpointPlayground.tsx)
  if (data?.runtime_id) keys.push(queryKeys.endpoint(data.runtime_id))
  return keys
}

/**
 * SPEC #582 FR-048f: event type -> query keys to invalidate when it arrives.
 *
 * The mapped type makes this table exhaustive over `DashboardEventType`:
 * adding an event type fails `tsc` until the table says which queries that
 * event invalidates. Keys must match those used by the dashboard queries
 * (Dashboard.tsx etc.).
 */
export const DASHBOARD_EVENT_INVALIDATIONS: {
  [T in DashboardEventType]: (data: DashboardEvent['data']) => DashboardQueryKey[]
} = {
  connected: () => [],
  NodeRegistered: endpointLifecycleKeys,
  EndpointStatusChanged: endpointLifecycleKeys,
  NodeRemoved: endpointLifecycleKeys,
  MetricsUpdated: () => [queryKeys.dashboardOverview()],
  // SPEC-4bb5b55f: Invalidate TPS data for the affected endpoint
  TpsUpdated: (data) => (data?.endpoint_id ? [queryKeys.endpointModelTps(data.endpoint_id)] : []),
  // Invalidate system-info so other clients see update state changes
  UpdateStateChanged: () => [queryKeys.systemInfo()],
}

/** Query keys to invalidate when a dashboard WebSocket event arrives. */
export function queryKeysToInvalidate(event: DashboardEvent): DashboardQueryKey[] {
  // The server may send a type this client does not know. An own-property
  // check also keeps inherited names such as "constructor" from being taken
  // for a rule.
  if (!Object.prototype.hasOwnProperty.call(DASHBOARD_EVENT_INVALIDATIONS, event.type)) return []
  return DASHBOARD_EVENT_INVALIDATIONS[event.type](event.data)
}
