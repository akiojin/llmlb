import type { DashboardEvent, DashboardEventType } from './useWebSocket'

/**
 * Query keys a dashboard WebSocket event can invalidate.
 * Each one must match the `queryKey` of the dashboard query it refreshes
 * (Dashboard.tsx, EndpointModelsTable.tsx, ModelsTable.tsx).
 */
export type InvalidatedQueryKey =
  | readonly ['dashboard-overview']
  | readonly ['request-responses']
  | readonly ['endpoint-model-tps', endpointId: string]
  | readonly ['system-info']

/** The keys an event invalidates: a fixed list, or one derived from the event payload. */
type InvalidationRule =
  | readonly InvalidatedQueryKey[]
  | ((event: DashboardEvent) => readonly InvalidatedQueryKey[])

/**
 * SPEC #582 FR-048f: which queries each event type invalidates.
 *
 * The mapped type requires a rule for every `DashboardEventType`, so adding an
 * event type without deciding what it invalidates fails the typecheck.
 */
export type DashboardEventInvalidations = {
  readonly [T in DashboardEventType]: InvalidationRule
}

export const DASHBOARD_EVENT_INVALIDATIONS: DashboardEventInvalidations = {
  connected: [],
  NodeRegistered: [['dashboard-overview'], ['request-responses']],
  NodeRemoved: [['dashboard-overview'], ['request-responses']],
  NodeStatusChanged: [['dashboard-overview'], ['request-responses']],
  MetricsUpdated: [['dashboard-overview']],
  // SPEC-4bb5b55f: only the TPS data of the affected endpoint
  TpsUpdated: ({ data }) => (data?.endpoint_id ? [['endpoint-model-tps', data.endpoint_id]] : []),
  // Other clients see update state changes through system-info
  UpdateStateChanged: [['system-info']],
}

/** Query keys to invalidate when `event` arrives on the dashboard WebSocket. */
export function queryKeysToInvalidate(event: DashboardEvent): readonly InvalidatedQueryKey[] {
  // The payload is unvalidated JSON: a server newer than this client may send an
  // event type the table does not know. Such an event invalidates nothing.
  if (!Object.prototype.hasOwnProperty.call(DASHBOARD_EVENT_INVALIDATIONS, event.type)) return []
  const rule = DASHBOARD_EVENT_INVALIDATIONS[event.type]
  return typeof rule === 'function' ? rule(event) : rule
}
