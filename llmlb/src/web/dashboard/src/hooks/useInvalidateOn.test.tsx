import { StrictMode, type ReactNode } from 'react'
import { act, renderHook } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { describe, expect, it, vi } from 'vitest'
import { queryKeys, type DashboardQueryKey } from '@/lib/queryKeys'
import type { DashboardChange, DashboardResource } from '@/lib/dashboardResources'
import { invalidateDashboardSubscriptions } from './dashboardSubscriptions'
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

// Keep the scoped T003 parity baseline after deleting the old production table.
const PARITY_MATRIX: { [T in DashboardResource]: {
  change: DashboardChange & { changed: T }; keys: DashboardQueryKey[]
} } = {
  endpoints: {
    change: { changed: 'endpoints', id: ID_A },
    keys: [queryKeys.dashboardOverview(), queryKeys.dashboardEndpoints(), queryKeys.requestResponses(), queryKeys.endpoint(ID_A)],
  },
  metrics: { change: { changed: 'metrics', id: ID_A }, keys: [queryKeys.dashboardOverview()] },
  tps: { change: { changed: 'tps', id: ID_A }, keys: [queryKeys.endpointModelTps(ID_A)] },
  system: { change: { changed: 'system' }, keys: [queryKeys.systemInfo()] },
}

function useTestSubscriptions() {
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
  it.each(Object.entries(PARITY_MATRIX))('retains the frozen T003 scoped contract for %s', (_resource, { change, keys }) => {
    const queryClient = client()
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    const { unmount } = renderHook(useTestSubscriptions, { wrapper: provider(queryClient) })
    act(() => { invalidateDashboardSubscriptions(queryClient, change) })
    expect(invalidate.mock.calls.map(([filter]) => filter?.queryKey)).toEqual(keys)
    unmount()
    invalidate.mockClear()
    act(() => { invalidateDashboardSubscriptions(queryClient, change) })
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
    act(() => { void invalidateDashboardSubscriptions(queryClient, { changed: 'tps', id: ID_A }) })
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
    act(() => { void invalidateDashboardSubscriptions(queryClient, { changed: 'metrics' }) })
    expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: queryKeys.dashboardOverview() })
    unmount()
    invalidate.mockClear()
    act(() => { void invalidateDashboardSubscriptions(queryClient, { changed: 'metrics' }) })
    expect(invalidate).not.toHaveBeenCalled()
  })
})

it('returns an owner-only refresh command without broadcasting to other subscribers', async () => {
  const client = new QueryClient()
  const invalidate = vi.spyOn(client, 'invalidateQueries').mockResolvedValue()
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
  const { result } = renderHook(() => {
    const refresh = useInvalidateOn(['endpoints'], queryKeys.endpointModels('A'), { id: 'A' })
    useInvalidateOn(['endpoints'], queryKeys.endpointModels('B'), { id: 'B' })
    useInvalidateOn(['endpoints'], queryKeys.dashboardOverview())
    return refresh
  }, { wrapper })
  expect(result.current).toBeTypeOf('function')
  await act(() => result.current())
  expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: queryKeys.endpointModels('A') })
})
