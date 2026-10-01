/**
 * Compile-time tests for the event -> query invalidation table (SPEC #582 FR-048f).
 *
 * Nothing here runs and nothing imports this file, so it is not part of the bundle.
 * `tsc --noEmit` (`make dashboard-checks`) is the test runner: a `@ts-expect-error`
 * that stops erroring, or an `Expect<...>` that stops being `true`, fails the typecheck.
 */
import type { DashboardEventType } from './useWebSocket'
import {
  DASHBOARD_EVENT_INVALIDATIONS,
  queryKeysToInvalidate,
  type DashboardEventInvalidations,
  type InvalidatedQueryKey,
} from './dashboardEventInvalidation'

type Expect<T extends true> = T
type Equal<A, B> =
  (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false

// The table has a rule for every event type the client can receive, and for nothing else.
export type TableCoversEveryEventType = Expect<
  Equal<keyof DashboardEventInvalidations, DashboardEventType>
>

// Issue #781: the three canonical events that gained a publisher have a rule.
export type TableCoversIssue781Events = Expect<
  Equal<
    Extract<keyof DashboardEventInvalidations, 'NodeRegistered' | 'NodeRemoved' | 'MetricsUpdated'>,
    'NodeRegistered' | 'NodeRemoved' | 'MetricsUpdated'
  >
>

// A table that forgets an event type does not compile.
declare const withoutMetricsUpdated: Omit<DashboardEventInvalidations, 'MetricsUpdated'>
// @ts-expect-error -- the rule for MetricsUpdated is missing
export const tableMissingARule: DashboardEventInvalidations = withoutMetricsUpdated

// A table cannot carry a rule for an event type the client does not declare.
export const tableWithUnknownEvent: DashboardEventInvalidations = {
  ...DASHBOARD_EVENT_INVALIDATIONS,
  // @ts-expect-error -- 'EndpointRenamed' is not a DashboardEventType
  EndpointRenamed: [['dashboard-overview']],
}

// A rule can only name query keys declared as invalidatable.
export const tableWithUnknownQueryKey: DashboardEventInvalidations = {
  ...DASHBOARD_EVENT_INVALIDATIONS,
  // @ts-expect-error -- 'dashboard-overveiw' is a typo, not an invalidatable query key
  MetricsUpdated: [['dashboard-overveiw']],
}

// A parameterised key needs its parameter.
export const tableWithIncompleteQueryKey: DashboardEventInvalidations = {
  ...DASHBOARD_EVENT_INVALIDATIONS,
  // @ts-expect-error -- 'endpoint-model-tps' is keyed by endpoint id
  TpsUpdated: [['endpoint-model-tps']],
}

// Callers get the declared keys back, not an untyped `unknown[]`.
export const invalidatedKeys: readonly InvalidatedQueryKey[] = queryKeysToInvalidate({
  type: 'MetricsUpdated',
})
