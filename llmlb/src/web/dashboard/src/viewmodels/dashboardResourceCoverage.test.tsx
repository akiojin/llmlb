import { StrictMode, type ReactNode } from 'react'
import { act, renderHook } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { dashboardApi, endpointsApi, systemApi } from '@/lib/api'
import { DASHBOARD_RESOURCES, type DashboardChange, type DashboardResource } from '@/lib/dashboardResources'
import { queryKeys, type DashboardQueryKey } from '@/lib/queryKeys'
import { FakeWebSocket } from '@/test/fake-websocket'
import { useWebSocket } from '@/hooks/useWebSocket'
import { useDashboardDataViewModel } from './useDashboardDataViewModel'
import { useEndpointViewModel } from './useEndpointViewModel'
import { useEndpointModelTpsViewModel } from './useEndpointModelTpsViewModel'

const A = '11111111-2222-3333-4444-555555555555'
const B = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee'

// Frozen T003/T004 contract, rather than a production resource -> query table.
// Extending the resource union requires a new behavior assertion at compile time.
const INVALIDATION_MATRIX: {
  [R in DashboardResource]: { change: DashboardChange & { changed: R }; keys: DashboardQueryKey[] }
} = {
  endpoints: {
    change: { changed: 'endpoints', id: A },
    keys: [queryKeys.dashboardOverview(), queryKeys.dashboardEndpoints(), queryKeys.requestResponses(), queryKeys.endpoint(A)],
  },
  metrics: { change: { changed: 'metrics', id: A }, keys: [queryKeys.dashboardOverview()] },
  tps: { change: { changed: 'tps', id: A }, keys: [queryKeys.endpointModelTps(A)] },
  system: { change: { changed: 'system' }, keys: [queryKeys.systemInfo()] },
}

const CACHE_KEYS = [
  queryKeys.dashboardOverview(), queryKeys.dashboardEndpoints(), queryKeys.requestResponses(),
  queryKeys.systemInfo(), queryKeys.endpoint(A), queryKeys.endpoint(B),
  queryKeys.endpointModelTps(A), queryKeys.endpointModelTps(B), queryKeys.auditLogs({}),
]

beforeEach(() => {
  // Keep I/O pending: this suite measures invalidation, not API response content.
  vi.spyOn(dashboardApi, 'getOverview').mockImplementation(() => new Promise(() => {}))
  vi.spyOn(dashboardApi, 'getEndpoints').mockImplementation(() => new Promise(() => {}))
  vi.spyOn(dashboardApi, 'getRequestResponses').mockImplementation(() => new Promise(() => {}))
  vi.spyOn(systemApi, 'getSystem').mockImplementation(() => new Promise(() => {}))
  vi.spyOn(endpointsApi, 'get').mockImplementation(() => new Promise(() => {}))
  vi.spyOn(endpointsApi, 'getModelTps').mockImplementation(() => new Promise(() => {}))
})

function setup() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } })
  const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
  const wrapper = ({ children }: { children: ReactNode }) => (
    <StrictMode><QueryClientProvider client={queryClient}>{children}</QueryClientProvider></StrictMode>
  )
  const hook = renderHook(() => {
    useDashboardDataViewModel({ pollingInterval: 10000, isViewer: false })
    useEndpointViewModel(A)
    useEndpointViewModel(B)
    useEndpointModelTpsViewModel(A)
    useEndpointModelTpsViewModel(B)
    useWebSocket()
  }, { wrapper })
  for (const key of CACHE_KEYS) queryClient.setQueryData(key, {})
  const receive = (change: DashboardChange) => act(() => FakeWebSocket.latest().receive(JSON.stringify(change)))
  return { ...hook, queryClient, invalidate, receive }
}

function expectInvalidated(queryClient: QueryClient, expected: readonly DashboardQueryKey[]) {
  for (const key of CACHE_KEYS) {
    expect(queryClient.getQueryState(key)?.isInvalidated, JSON.stringify(key)).toBe(
      expected.some((candidate) => JSON.stringify(candidate) === JSON.stringify(key)),
    )
  }
}

describe('production ViewModel resource coverage (SPEC #821 T005)', () => {
  it('has a compiler-enforced behavior row for every wire resource', () => {
    expect(Object.keys(INVALIDATION_MATRIX).sort()).toEqual([...DASHBOARD_RESOURCES].sort())
  })

  it.each(Object.entries(INVALIDATION_MATRIX))('%s is subscribed by a mounted ViewModel with T003-equivalent results', (_resource, { change, keys }) => {
    const { queryClient, invalidate, receive } = setup()
    receive(change)
    expect(invalidate).toHaveBeenCalled()
    expectInvalidated(queryClient, keys)
  })

  it.each(DASHBOARD_RESOURCES)('%s without an id retains legacy effective cache invalidation', (changed) => {
    const { queryClient, receive } = setup()
    receive({ changed })
    const keys = changed === 'endpoints'
      ? [...INVALIDATION_MATRIX.endpoints.keys, queryKeys.endpoint(B)]
      : changed === 'tps'
        ? [queryKeys.endpointModelTps(A), queryKeys.endpointModelTps(B)]
        : INVALIDATION_MATRIX[changed].keys
    expectInvalidated(queryClient, keys)
  })

  it('removes all production subscriptions when their ViewModels unmount', () => {
    const { queryClient, invalidate, unmount } = setup()
    unmount()
    invalidate.mockClear()
    // A different socket owner can remain mounted after these views disappear.
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    )
    renderHook(() => useWebSocket(), { wrapper })
    for (const changed of DASHBOARD_RESOURCES) {
      act(() => FakeWebSocket.latest().receive(JSON.stringify({ changed })))
    }
    expect(invalidate).not.toHaveBeenCalled()
  })
})
