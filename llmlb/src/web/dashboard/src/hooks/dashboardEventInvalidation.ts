import type { QueryKey } from '@tanstack/react-query'
import type { DashboardEvent } from './useWebSocket'

/**
 * Query keys to invalidate when a dashboard WebSocket event arrives.
 * Keys must match those used by the dashboard queries (Dashboard.tsx etc.).
 */
export function queryKeysToInvalidate(event: DashboardEvent): QueryKey[] {
  switch (event.type) {
    case 'NodeRegistered':
    case 'NodeRemoved':
    case 'NodeStatusChanged':
      return [['dashboard-overview'], ['request-responses']]
    case 'MetricsUpdated':
      return [['dashboard-overview']]
    case 'TpsUpdated':
      // SPEC-4bb5b55f: Invalidate TPS data for the affected endpoint
      return event.data?.endpoint_id ? [['endpoint-model-tps', event.data.endpoint_id]] : []
    case 'UpdateStateChanged':
      // Invalidate system-info so other clients see update state changes
      return [['system-info']]
    default:
      return []
  }
}
