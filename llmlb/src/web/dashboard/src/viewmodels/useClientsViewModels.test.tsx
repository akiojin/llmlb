import type { ReactNode } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { clientsApi, type ClientDetailResponse, type ClientRankingResponse } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { invalidateDashboardSubscriptions } from '@/hooks/dashboardSubscriptions'
import { deferred } from '@/test/render'
import { useClientsTabViewModel } from './useClientsTabViewModel'
import { useClientDrilldownViewModel } from './useClientDrilldownViewModel'

function setup(polling = false) {
  const client = new QueryClient({ defaultOptions: { queries: {
    retry: false, gcTime: Infinity,
    ...(polling ? { refetchInterval: 5000, refetchIntervalInBackground: true } : {}),
  } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
  return { client, wrapper }
}

function rankingPage(): ClientRankingResponse {
  return {
    rankings: [{ ip: '192.0.2.10', request_count: 420, last_seen: '2026-10-01T00:00:00Z', is_alert: false, api_key_count: 1 }],
    total_count: 45, page: 1, per_page: 20,
  }
}

function detail(overrides: Partial<ClientDetailResponse> = {}): ClientDetailResponse {
  return {
    total_requests: 1234, first_seen: '2026-10-01T00:00:00Z', last_seen: null,
    recent_requests: [
      { id: 'zero', timestamp: '2026-10-01T00:00:00Z', model: 'llama', status: 'success', duration_ms: 0 },
      { id: 'null', timestamp: '2026-10-01T00:00:05Z', model: 'qwen', status: 'error', duration_ms: null },
    ],
    model_distribution: [{ model: 'llama', request_count: 1234, percentage: 100 }],
    hourly_pattern: [{ hour: 9, count: 1234 }],
    ...overrides,
  }
}

function stubSummary() {
  return {
    ranking: vi.spyOn(clientsApi, 'getClientRanking').mockResolvedValue(rankingPage()),
    timeline: vi.spyOn(clientsApi, 'getTimeline').mockResolvedValue([{ hour: '2026-10-01T00:00:00Z', unique_ips: 4 }]),
    models: vi.spyOn(clientsApi, 'getModels').mockResolvedValue([{ model: 'llama', request_count: 420, percentage: 100 }]),
    heatmap: vi.spyOn(clientsApi, 'getHeatmap').mockResolvedValue([{ day_of_week: 1, hour: 9, count: 5 }]),
  }
}

function stubDetail() {
  return {
    detail: vi.spyOn(clientsApi, 'getClientDetail').mockResolvedValue(detail()),
    apiKeys: vi.spyOn(clientsApi, 'getClientApiKeys').mockResolvedValue([
      { api_key_id: 'active', name: 'ci', request_count: 1234 },
      { api_key_id: 'deleted', name: null, request_count: 0 },
    ]),
  }
}

afterEach(() => vi.useRealTimers())

describe('client ViewModel contracts', () => {
  it('owns each summary request once with existing keys and does not acquire drilldown data', async () => {
    vi.stubGlobal('location', { search: '?tab=clients&ip=192.0.2.10' })
    const api = stubSummary()
    const child = stubDetail()
    const { client, wrapper } = setup()
    const { result } = renderHook(() => useClientsTabViewModel(), { wrapper })
    await waitFor(() => expect(result.current.isLoading).toBe(false))

    expect(api.ranking).toHaveBeenCalledExactlyOnceWith({ page: 1, per_page: 20, ip: '192.0.2.10' })
    expect(api.timeline).toHaveBeenCalledTimes(1)
    expect(api.models).toHaveBeenCalledTimes(1)
    expect(api.heatmap).toHaveBeenCalledExactlyOnceWith({ ip: '192.0.2.10' })
    expect(client.getQueryData(queryKeys.clientRanking(1, 20, '192.0.2.10'))).toEqual(rankingPage())
    expect(client.getQueryData(queryKeys.clientTimeline())).toEqual(result.current.timelineData)
    expect(client.getQueryData(queryKeys.clientModels())).toEqual(result.current.modelsData)
    expect(client.getQueryData(queryKeys.clientHeatmap('192.0.2.10'))).toEqual(result.current.heatmapData)
    expect(result.current).toMatchObject({ page: 1, perPage: 20, totalCount: 45, ipFilter: '192.0.2.10', ipFilterLabel: 'Filtered: 192.0.2.10' })
    expect(result.current.rankings).toEqual(rankingPage().rankings)
    expect(child.detail).not.toHaveBeenCalled()
    expect(child.apiKeys).not.toHaveBeenCalled()
  })

  it('keeps the initial IP across rerenders and page changes while leaving unrelated summaries cached', async () => {
    const location = { search: '?tab=clients&ip=192.0.2.10' }
    vi.stubGlobal('location', location)
    const api = stubSummary()
    const { wrapper } = setup()
    const { result, rerender } = renderHook(() => useClientsTabViewModel(), { wrapper })
    await waitFor(() => expect(result.current.isLoading).toBe(false))
    location.search = '?tab=clients&ip=198.51.100.7'
    rerender()
    act(() => result.current.setPage(2))
    await waitFor(() => expect(api.ranking).toHaveBeenCalledTimes(2))
    expect(api.ranking).toHaveBeenLastCalledWith({ page: 2, per_page: 20, ip: '192.0.2.10' })
    expect(result.current.page).toBe(2)
    expect(result.current.ipFilter).toBe('192.0.2.10')
    expect(api.timeline).toHaveBeenCalledTimes(1)
    expect(api.models).toHaveBeenCalledTimes(1)
    expect(api.heatmap).toHaveBeenCalledTimes(1)
  })

  it('clears only the URL IP parameter using the current navigation URL', async () => {
    const location = { search: '?tab=clients&ip=192.0.2.10' }
    vi.stubGlobal('location', location)
    stubSummary()
    const { wrapper } = setup()
    const { result } = renderHook(() => useClientsTabViewModel(), { wrapper })
    await waitFor(() => expect(result.current.isLoading).toBe(false))
    location.search = '?tab=clients&ip=192.0.2.10&keep=1'
    act(() => result.current.clearIpFilter())
    expect(location.search).toBe('tab=clients&keep=1')
  })

  it('uses ranking loading alone and preserves the empty fallback after a failed ranking', async () => {
    vi.stubGlobal('location', { search: '?ip=' })
    const api = stubSummary()
    const ranking = deferred<ClientRankingResponse>()
    api.ranking.mockReturnValue(ranking.promise)
    api.timeline.mockReturnValue(new Promise(() => {}))
    const { wrapper } = setup()
    const { result } = renderHook(() => useClientsTabViewModel(), { wrapper })
    expect(result.current.isLoading).toBe(true)
    expect(result.current.ipFilter).toBeUndefined()
    expect(result.current.ipFilterLabel).toBeNull()
    await act(async () => ranking.reject(new Error('ranking unavailable')))
    await waitFor(() => expect(result.current.isLoading).toBe(false))
    expect(result.current.rankings).toEqual([])
    expect(result.current.totalCount).toBe(0)
    expect(result.current.timelineData).toEqual([])
  })

  it('owns detail and API keys once per IP and derives nullable dates, durations, counts and chart labels', async () => {
    const api = stubDetail()
    const { client, wrapper } = setup()
    const { result } = renderHook(() => useClientDrilldownViewModel('192.0.2.10'), { wrapper })
    await waitFor(() => expect(result.current.apiKeys).toHaveLength(2))
    expect(api.detail).toHaveBeenCalledExactlyOnceWith('192.0.2.10')
    expect(api.apiKeys).toHaveBeenCalledExactlyOnceWith('192.0.2.10')
    expect(client.getQueryData(queryKeys.clientDetail('192.0.2.10'))).toEqual(detail())
    expect(client.getQueryData(queryKeys.clientApiKeys('192.0.2.10'))).toHaveLength(2)
    expect(result.current).toMatchObject({ isLoading: false, hasData: true,
      totalRequestsLabel: `${(1234).toLocaleString()} requests`,
      firstSeenLabel: new Date('2026-10-01T00:00:00Z').toLocaleString(), lastSeenLabel: null, hasHourlyData: true,
    })
    expect(result.current.recentRequests).toEqual([
      { id: 'zero', model: 'llama', timeLabel: new Date('2026-10-01T00:00:00Z').toLocaleTimeString(), durationLabel: '0ms' },
      { id: 'null', model: 'qwen', timeLabel: new Date('2026-10-01T00:00:05Z').toLocaleTimeString(), durationLabel: '-' },
    ])
    expect(result.current.modelDistribution).toEqual(detail().model_distribution)
    expect(result.current.hourlyPattern).toEqual(detail().hourly_pattern)
    expect(result.current.apiKeys).toEqual([
      { id: 'active', name: 'ci', requestCountLabel: (1234).toLocaleString() },
      { id: 'deleted', name: null, requestCountLabel: '0' },
    ])
    expect(result.current.formatHourlyRequests(undefined)).toEqual([0, 'Requests'])
    expect(result.current.formatHourlyRequests(7)).toEqual([7, 'Requests'])
    expect(result.current.formatHour(9)).toBe('9:00')
    expect(result.current.formatHour(undefined)).toBe(':00')
  })

  it('keeps detail loading independent of API keys and switches both keys when the IP changes', async () => {
    const api = stubDetail()
    const response = deferred<ClientDetailResponse>()
    api.detail.mockReturnValueOnce(response.promise).mockResolvedValue(detail({ first_seen: null, last_seen: '2026-10-02T00:00:00Z', hourly_pattern: [{ hour: 9, count: 0 }] }))
    api.apiKeys.mockReturnValue(new Promise(() => {}))
    const { wrapper } = setup()
    const { result, rerender } = renderHook(({ ip }) => useClientDrilldownViewModel(ip), {
      wrapper, initialProps: { ip: '192.0.2.10' },
    })
    expect(result.current.isLoading).toBe(true)
    await act(async () => response.resolve(detail()))
    await waitFor(() => expect(result.current.isLoading).toBe(false))
    expect(result.current.hasData).toBe(true)
    expect(result.current.apiKeys).toEqual([])
    rerender({ ip: '198.51.100.7' })
    await waitFor(() => expect(result.current.lastSeenLabel).toBe(new Date('2026-10-02T00:00:00Z').toLocaleString()))
    expect(api.detail).toHaveBeenLastCalledWith('198.51.100.7')
    expect(api.apiKeys).toHaveBeenLastCalledWith('198.51.100.7')
    expect(result.current.firstSeenLabel).toBeNull()
    expect(result.current.hasHourlyData).toBe(false)
  })

  it.each(['empty', 'failed'])('retains the no-data branch for %s detail without adding an error surface', async (kind) => {
    const api = stubDetail()
    if (kind === 'empty') api.detail.mockResolvedValue(detail({ total_requests: 0, recent_requests: [], hourly_pattern: [] }))
    else api.detail.mockRejectedValue(new Error('detail unavailable'))
    const { wrapper } = setup()
    const { result } = renderHook(() => useClientDrilldownViewModel('192.0.2.10'), { wrapper })
    await waitFor(() => expect(result.current.isLoading).toBe(false))
    expect(result.current.hasData).toBe(false)
    expect(result.current.hasHourlyData).toBe(false)
  })

  it('inherits provider polling, stops child polling on collapse and stops all polling on parent unmount', async () => {
    vi.useFakeTimers()
    vi.stubGlobal('location', { search: '' })
    const summary = stubSummary()
    const child = stubDetail()
    const { wrapper } = setup(true)
    const parent = renderHook(() => useClientsTabViewModel(), { wrapper })
    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(child.detail).not.toHaveBeenCalled()
    const expanded = renderHook(() => useClientDrilldownViewModel('192.0.2.10'), { wrapper })
    await act(() => vi.advanceTimersByTimeAsync(1))
    await act(() => vi.advanceTimersByTimeAsync(5000))
    for (const get of Object.values(summary)) expect(get).toHaveBeenCalledTimes(2)
    for (const get of Object.values(child)) expect(get).toHaveBeenCalledTimes(2)
    expanded.unmount()
    await act(() => vi.advanceTimersByTimeAsync(5000))
    for (const get of Object.values(summary)) expect(get).toHaveBeenCalledTimes(3)
    for (const get of Object.values(child)) expect(get).toHaveBeenCalledTimes(2)
    parent.unmount()
    await act(() => vi.advanceTimersByTimeAsync(10000))
    for (const get of Object.values(summary)) expect(get).toHaveBeenCalledTimes(3)
  })

  it('adds no subscriptions for unrelated dashboard resources', async () => {
    stubSummary()
    stubDetail()
    const { client, wrapper } = setup()
    const { result, unmount } = renderHook(() => ({
      parent: useClientsTabViewModel(), child: useClientDrilldownViewModel('192.0.2.10'),
    }), { wrapper })
    await waitFor(() => expect(result.current.parent.isLoading || result.current.child.isLoading).toBe(false))
    const invalidate = vi.spyOn(client, 'invalidateQueries')
    await act(async () => {
      for (const changed of ['endpoints', 'metrics', 'tps', 'system'] as const) {
        await invalidateDashboardSubscriptions(client, { changed })
      }
    })
    expect(invalidate).not.toHaveBeenCalled()
    unmount()
  })
})
