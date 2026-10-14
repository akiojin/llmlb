import { QueryClient } from '@tanstack/react-query'
import { describe, expect, it, vi } from 'vitest'
import { queryKeys } from '@/lib/queryKeys'
import type { DashboardChange } from '@/lib/dashboardResources'
import { invalidateDashboardSubscriptions, registerDashboardSubscription } from './dashboardSubscriptions'

function client() {
  return new QueryClient({ defaultOptions: { queries: { gcTime: Infinity, retry: false } } })
}

describe('dashboard resource subscriptions', () => {
  it('matches aggregate and same-id details, excluding other ids and resources', () => {
    const queryClient = client()
    const overview = queryKeys.dashboardOverview()
    const detailA = queryKeys.endpoint('A')
    const detailB = queryKeys.endpoint('B')
    const system = queryKeys.systemInfo()
    for (const key of [overview, detailA, detailB, system]) queryClient.setQueryData(key, 'cached')
    registerDashboardSubscription(queryClient, ['endpoints', 'metrics'], overview)
    registerDashboardSubscription(queryClient, ['endpoints'], detailA, { id: 'A' })
    registerDashboardSubscription(queryClient, ['endpoints'], detailB, { id: 'B' })
    registerDashboardSubscription(queryClient, ['system'], system)

    invalidateDashboardSubscriptions(queryClient, { changed: 'endpoints', id: 'A' })

    expect(queryClient.getQueryState(overview)?.isInvalidated).toBe(true)
    expect(queryClient.getQueryState(detailA)?.isInvalidated).toBe(true)
    expect(queryClient.getQueryState(detailB)?.isInvalidated).toBe(false)
    expect(queryClient.getQueryState(system)?.isInvalidated).toBe(false)
  })

  it('broadcasts id-less changes to every registration of that resource', () => {
    const queryClient = client()
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    registerDashboardSubscription(queryClient, ['tps'], queryKeys.allModelStats())
    registerDashboardSubscription(queryClient, ['tps'], queryKeys.endpointModelTps('A'), { id: 'A' })
    registerDashboardSubscription(queryClient, ['tps'], queryKeys.endpointModelTps('B'), { id: 'B' })
    invalidateDashboardSubscriptions(queryClient, { changed: 'tps' })
    expect(invalidate.mock.calls.map(([filter]) => filter?.queryKey)).toEqual([
      queryKeys.allModelStats(), queryKeys.endpointModelTps('A'), queryKeys.endpointModelTps('B'),
    ])
  })

  it('treats keys containing an id as aggregate unless scope is explicitly declared', () => {
    const queryClient = client()
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    registerDashboardSubscription(queryClient, ['endpoints'], queryKeys.endpoint('B'))
    invalidateDashboardSubscriptions(queryClient, { changed: 'endpoints', id: 'A' })
    expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: queryKeys.endpoint('B') })
  })

  it('deduplicates structural keys while keeping each registration independently removable', () => {
    const queryClient = client()
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    const first = registerDashboardSubscription(queryClient, ['metrics', 'metrics'], queryKeys.auditLogs({ page: 1, per_page: 50 }))
    const second = registerDashboardSubscription(queryClient, ['metrics'], queryKeys.auditLogs({ per_page: 50, page: 1 }))
    invalidateDashboardSubscriptions(queryClient, { changed: 'metrics' })
    expect(invalidate).toHaveBeenCalledTimes(1)
    first()
    first()
    invalidateDashboardSubscriptions(queryClient, { changed: 'metrics' })
    expect(invalidate).toHaveBeenCalledTimes(2)
    second()
    invalidateDashboardSubscriptions(queryClient, { changed: 'metrics' })
    expect(invalidate).toHaveBeenCalledTimes(2)
  })

  it('only deduplicates matching scopes, so another id cannot suppress a match', () => {
    const queryClient = client()
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    registerDashboardSubscription(queryClient, ['metrics'], queryKeys.dashboardOverview(), { id: 'B' })
    registerDashboardSubscription(queryClient, ['metrics'], queryKeys.dashboardOverview(), { id: 'A' })
    invalidateDashboardSubscriptions(queryClient, { changed: 'metrics', id: 'A' })
    expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: queryKeys.dashboardOverview() })
  })

  it('keeps a newer registration when an old cleanup is called again', () => {
    const queryClient = client()
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    const oldCleanup = registerDashboardSubscription(queryClient, ['metrics'], queryKeys.dashboardOverview())
    oldCleanup()
    registerDashboardSubscription(queryClient, ['metrics'], queryKeys.dashboardOverview())
    oldCleanup()
    invalidateDashboardSubscriptions(queryClient, { changed: 'metrics' })
    expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: queryKeys.dashboardOverview() })
  })

  it('keeps the existing React Query prefix boundary', () => {
    const queryClient = client()
    const page = queryKeys.clientRanking(2, 10, undefined)
    queryClient.setQueryData(page, 'cached')
    queryClient.setQueryData(['client-ranking-other'], 'cached')
    registerDashboardSubscription(queryClient, ['metrics'], queryKeys.clientRanking())
    invalidateDashboardSubscriptions(queryClient, { changed: 'metrics' })
    expect(queryClient.getQueryState(page)?.isInvalidated).toBe(true)
    expect(queryClient.getQueryState(['client-ranking-other'])?.isInvalidated).toBe(false)
  })

  it('isolates registrations by QueryClient even for identical keys', () => {
    const first = client()
    const second = client()
    const key = queryKeys.dashboardOverview()
    first.setQueryData(key, 'cached')
    second.setQueryData(key, 'cached')
    registerDashboardSubscription(first, ['metrics'], key)
    registerDashboardSubscription(second, ['metrics'], key)
    invalidateDashboardSubscriptions(first, { changed: 'metrics' })
    expect(first.getQueryState(key)?.isInvalidated).toBe(true)
    expect(second.getQueryState(key)?.isInvalidated).toBe(false)
  })

  it.each(['unknown', 'constructor', 'toString', '__proto__'])('ignores unknown resource %s', (changed) => {
    const queryClient = client()
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    registerDashboardSubscription(queryClient, ['metrics'], queryKeys.dashboardOverview())
    invalidateDashboardSubscriptions(queryClient, { changed } as DashboardChange)
    expect(invalidate).not.toHaveBeenCalled()
  })

  it('is harmless with an empty resource list or without registrations', () => {
    const queryClient = client()
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    invalidateDashboardSubscriptions(queryClient, { changed: 'metrics' })
    registerDashboardSubscription(queryClient, [], queryKeys.dashboardOverview())
    invalidateDashboardSubscriptions(queryClient, { changed: 'metrics' })
    expect(invalidate).not.toHaveBeenCalled()
  })
})
