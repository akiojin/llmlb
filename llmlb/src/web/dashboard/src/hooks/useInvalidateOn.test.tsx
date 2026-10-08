import { StrictMode, type ReactNode } from 'react'
import { act, renderHook } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { describe, expect, it, vi } from 'vitest'
import { queryKeys, type DashboardQueryKey } from '@/lib/queryKeys'
import type { DashboardChange, DashboardResource } from '@/lib/dashboardResources'
import { queryKeysToInvalidate } from './dashboardEventInvalidation'
import { invalidateDashboardSubscriptions } from './dashboardSubscriptions'
import type { DashboardEvent, DashboardEventType } from './useWebSocket'
import { useInvalidateOn } from './useInvalidateOn'

function client() {
  return new QueryClient({ defaultOptions: { queries: { gcTime: Infinity, retry: false } } })
}

function provider(queryClient: QueryClient) {
  return ({ children }: { children: ReactNode }) => (
    <StrictMode><QueryClientProvider client={queryClient}>{children}</QueryClientProvider></StrictMode>
  )
}

const ID_A = '11111111-2222-3333-4444-555555555555'
const ID_B = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee'

// Test-only projection of valid server payloads. Production wire changes belong to T004.
const PARITY_MATRIX: { [T in DashboardEventType]: { event: DashboardEvent; change: DashboardChange | null } } = {
  connected: { event: { type: 'connected' }, change: null },
  NodeRegistered: {
    event: { type: 'NodeRegistered', data: { runtime_id: ID_A, status: 'pending' } },
    change: { changed: 'endpoints', id: ID_A },
  },
  NodeRemoved: {
    event: { type: 'NodeRemoved', data: { runtime_id: ID_A } },
    change: { changed: 'endpoints', id: ID_A },
  },
  EndpointStatusChanged: {
    event: { type: 'EndpointStatusChanged', data: { runtime_id: ID_A, old_status: 'online', new_status: 'offline' } },
    change: { changed: 'endpoints', id: ID_A },
  },
  MetricsUpdated: {
    event: { type: 'MetricsUpdated', data: { runtime_id: ID_A } },
    change: { changed: 'metrics', id: ID_A },
  },
  TpsUpdated: {
    event: { type: 'TpsUpdated', data: { endpoint_id: ID_A, model_id: 'model', tps: 1 } },
    change: { changed: 'tps', id: ID_A },
  },
  UpdateStateChanged: { event: { type: 'UpdateStateChanged' }, change: { changed: 'system' } },
}

function useLegacySubscriptions() {
  useInvalidateOn(['endpoints', 'metrics'], queryKeys.dashboardOverview())
  useInvalidateOn(['endpoints'], queryKeys.dashboardEndpoints())
  useInvalidateOn(['endpoints'], queryKeys.requestResponses())
  useInvalidateOn(['endpoints'], queryKeys.endpoint(ID_A), { id: ID_A })
  useInvalidateOn(['endpoints'], queryKeys.endpoint(ID_B), { id: ID_B })
  useInvalidateOn(['tps'], queryKeys.endpointModelTps(ID_A), { id: ID_A })
  useInvalidateOn(['tps'], queryKeys.endpointModelTps(ID_B), { id: ID_B })
  useInvalidateOn(['system'], queryKeys.systemInfo())
}

describe('useInvalidateOn', () => {
  it.each(Object.entries(PARITY_MATRIX))('matches legacy invalidation for %s with both paths present', (_type, { event, change }) => {
    const queryClient = client()
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    const { unmount } = renderHook(useLegacySubscriptions, { wrapper: provider(queryClient) })
    act(() => { if (change) invalidateDashboardSubscriptions(queryClient, change) })
    expect(invalidate.mock.calls.map(([filter]) => filter?.queryKey)).toEqual(queryKeysToInvalidate(event))
    unmount()
    invalidate.mockClear()
    act(() => { if (change) invalidateDashboardSubscriptions(queryClient, change) })
    expect(invalidate).not.toHaveBeenCalled()
  })

  it('replaces resources, key and explicit id when the hook rerenders', () => {
    const queryClient = client()
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    type Props = { resources: DashboardResource[]; key: DashboardQueryKey; id?: string }
    const initialProps: Props = { resources: ['endpoints'], key: queryKeys.endpoint(ID_A), id: ID_A }
    const { rerender, unmount } = renderHook(
      ({ resources, key, id }: Props) => useInvalidateOn(resources, key, { id }),
      { initialProps, wrapper: provider(queryClient) },
    )
    rerender({ resources: ['tps'], key: queryKeys.endpointModelTps(ID_B), id: ID_B })
    act(() => {
      invalidateDashboardSubscriptions(queryClient, { changed: 'endpoints', id: ID_A })
      invalidateDashboardSubscriptions(queryClient, { changed: 'tps', id: ID_A })
      invalidateDashboardSubscriptions(queryClient, { changed: 'tps', id: ID_B })
    })
    expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: queryKeys.endpointModelTps(ID_B) })
    // Clearing a declared id turns the registration into an aggregate.
    rerender({ resources: ['tps'], key: queryKeys.endpointModelTps(ID_B) })
    act(() => invalidateDashboardSubscriptions(queryClient, { changed: 'tps', id: ID_A }))
    expect(invalidate).toHaveBeenCalledTimes(2)
    unmount()
  })

  it('removes the old client registration when the provider changes', () => {
    const oldClient = client()
    const newClient = client()
    const oldInvalidate = vi.spyOn(oldClient, 'invalidateQueries')
    const newInvalidate = vi.spyOn(newClient, 'invalidateQueries')
    let activeClient = oldClient
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={activeClient}>{children}</QueryClientProvider>
    )
    const { rerender, unmount } = renderHook(
      () => useInvalidateOn(['metrics'], queryKeys.dashboardOverview()), { wrapper },
    )
    activeClient = newClient
    rerender()
    act(() => {
      invalidateDashboardSubscriptions(oldClient, { changed: 'metrics' })
      invalidateDashboardSubscriptions(newClient, { changed: 'metrics' })
    })
    expect(oldInvalidate).not.toHaveBeenCalled()
    expect(newInvalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: queryKeys.dashboardOverview() })
    unmount()
  })

  it('does not accumulate duplicate registrations across equivalent inline rerenders', () => {
    const queryClient = client()
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    const { rerender, unmount } = renderHook(
      () => useInvalidateOn(['metrics'], queryKeys.dashboardOverview()), { wrapper: provider(queryClient) },
    )
    rerender()
    rerender()
    act(() => invalidateDashboardSubscriptions(queryClient, { changed: 'metrics' }))
    expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: queryKeys.dashboardOverview() })
    unmount()
    invalidate.mockClear()
    act(() => invalidateDashboardSubscriptions(queryClient, { changed: 'metrics' }))
    expect(invalidate).not.toHaveBeenCalled()
  })
})
