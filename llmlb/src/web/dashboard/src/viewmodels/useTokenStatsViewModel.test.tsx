import type { ReactNode } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { dashboardApi, type DailyTokenStats, type MonthlyTokenStats } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { DASHBOARD_RESOURCES } from '@/lib/dashboardResources'
import { invalidateDashboardSubscriptions } from '@/hooks/dashboardSubscriptions'
import { FakeWebSocket } from '@/test/fake-websocket'
import { deferred } from '@/test/render'
import { useTokenStatsViewModel } from './useTokenStatsViewModel'

const daily: DailyTokenStats = {
  date: '2026-09-30', request_count: 12,
  total_input_tokens: 1500, total_output_tokens: 2500, total_tokens: 4000,
}
const monthly: MonthlyTokenStats = {
  month: '2026-09', request_count: 340,
  total_input_tokens: 1_200_000, total_output_tokens: 3_400_000, total_tokens: 4_600_000,
}

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

afterEach(() => vi.useRealTimers())

describe('token statistics ViewModel', () => {
  it('starts both existing queries together and settles daily/monthly loading independently', async () => {
    const dailyResponse = deferred<DailyTokenStats[]>()
    const monthlyResponse = deferred<MonthlyTokenStats[]>()
    const getDaily = vi.spyOn(dashboardApi, 'getDailyTokenStats').mockReturnValue(dailyResponse.promise)
    const getMonthly = vi.spyOn(dashboardApi, 'getMonthlyTokenStats').mockReturnValue(monthlyResponse.promise)
    const { client, wrapper } = setup()
    const { result } = renderHook(() => useTokenStatsViewModel(), { wrapper })

    expect(getDaily).toHaveBeenCalledExactlyOnceWith(7)
    expect(getMonthly).toHaveBeenCalledExactlyOnceWith(6)
    expect(result.current).toMatchObject({
      loadingDaily: true, loadingMonthly: true,
      dailyRows: [], monthlyRows: [], dailyChart: [], monthlyChart: [],
    })

    await act(async () => dailyResponse.resolve([daily]))
    await waitFor(() => expect(result.current.loadingDaily).toBe(false))
    expect(result.current.loadingMonthly).toBe(true)
    expect(result.current.dailyRows).toEqual([{
      label: '2026-09-30', requestsLabel: '12', inputTokensLabel: '1.5K',
      outputTokensLabel: '2.5K', totalTokensLabel: '4.0K',
    }])
    expect(result.current.dailyChart).toEqual([{ label: '2026-09-30', input: 1500, output: 2500 }])
    expect(client.getQueryData(queryKeys.tokenStatsDaily())).toEqual([daily])
    expect(result.current.monthlyRows).toEqual([])

    await act(async () => monthlyResponse.resolve([monthly]))
    await waitFor(() => expect(result.current.loadingMonthly).toBe(false))
    expect(result.current.monthlyRows).toEqual([{
      label: '2026-09', requestsLabel: '340', inputTokensLabel: '1.2M',
      outputTokensLabel: '3.4M', totalTokensLabel: '4.6M',
    }])
    expect(result.current.monthlyChart).toEqual([{ label: '2026-09', input: 1_200_000, output: 3_400_000 }])
    expect(client.getQueryData(queryKeys.tokenStatsMonthly())).toEqual([monthly])
  })

  it('preserves response order and zero/sub-thousand/compact number labels without changing chart numbers', async () => {
    vi.spyOn(dashboardApi, 'getDailyTokenStats').mockResolvedValue([
      { ...daily, date: 'original date', request_count: 0, total_input_tokens: 999, total_output_tokens: 1, total_tokens: 1000 },
      { ...daily, date: 'earlier date' },
    ])
    vi.spyOn(dashboardApi, 'getMonthlyTokenStats').mockResolvedValue([
      { ...monthly, month: 'original month', request_count: 1_000_000, total_input_tokens: 0, total_output_tokens: 0, total_tokens: 0 },
    ])
    const { wrapper } = setup()
    const { result } = renderHook(() => useTokenStatsViewModel(), { wrapper })
    await waitFor(() => expect(result.current.loadingDaily || result.current.loadingMonthly).toBe(false))

    expect(result.current.dailyRows.map((row) => row.label)).toEqual(['original date', 'earlier date'])
    expect(result.current.dailyRows[0]).toEqual({
      label: 'original date', requestsLabel: '0', inputTokensLabel: '999', outputTokensLabel: '1', totalTokensLabel: '1.0K',
    })
    expect(result.current.dailyChart).toEqual([
      { label: 'original date', input: 999, output: 1 },
      { label: 'earlier date', input: 1500, output: 2500 },
    ])
    expect(result.current.monthlyRows[0]).toEqual({
      label: 'original month', requestsLabel: '1.0M', inputTokensLabel: '0', outputTokensLabel: '0', totalTokensLabel: '0',
    })
    expect([0, 999, 1000, 1_000_000].map(result.current.formatChartNumber)).toEqual(['0', '999', '1.0K', '1.0M'])
  })

  it.each(['daily', 'monthly'] as const)('keeps an empty %s response independent of the other period', async (emptyPeriod) => {
    vi.spyOn(dashboardApi, 'getDailyTokenStats').mockResolvedValue(emptyPeriod === 'daily' ? [] : [daily])
    vi.spyOn(dashboardApi, 'getMonthlyTokenStats').mockResolvedValue(emptyPeriod === 'monthly' ? [] : [monthly])
    const { wrapper } = setup()
    const { result } = renderHook(() => useTokenStatsViewModel(), { wrapper })
    await waitFor(() => expect(result.current.loadingDaily || result.current.loadingMonthly).toBe(false))

    expect(result.current.dailyRows).toHaveLength(emptyPeriod === 'daily' ? 0 : 1)
    expect(result.current.dailyChart).toHaveLength(emptyPeriod === 'daily' ? 0 : 1)
    expect(result.current.monthlyRows).toHaveLength(emptyPeriod === 'monthly' ? 0 : 1)
    expect(result.current.monthlyChart).toHaveLength(emptyPeriod === 'monthly' ? 0 : 1)
  })

  it.each(['daily', 'monthly'] as const)('retains the empty presentation on a %s failure while the other period succeeds', async (failedPeriod) => {
    const getDaily = vi.spyOn(dashboardApi, 'getDailyTokenStats')
    const getMonthly = vi.spyOn(dashboardApi, 'getMonthlyTokenStats')
    if (failedPeriod === 'daily') {
      getDaily.mockRejectedValue(new Error('daily unavailable'))
      getMonthly.mockResolvedValue([monthly])
    } else {
      getDaily.mockResolvedValue([daily])
      getMonthly.mockRejectedValue(new Error('monthly unavailable'))
    }
    const { wrapper } = setup()
    const { result } = renderHook(() => useTokenStatsViewModel(), { wrapper })
    await waitFor(() => expect(result.current.loadingDaily || result.current.loadingMonthly).toBe(false))

    expect(result.current.dailyRows).toHaveLength(failedPeriod === 'daily' ? 0 : 1)
    expect(result.current.dailyChart).toHaveLength(failedPeriod === 'daily' ? 0 : 1)
    expect(result.current.monthlyRows).toHaveLength(failedPeriod === 'monthly' ? 0 : 1)
    expect(result.current.monthlyChart).toHaveLength(failedPeriod === 'monthly' ? 0 : 1)
  })

  it('inherits both provider polling intervals, refreshes presentation and stops polling on unmount', async () => {
    vi.useFakeTimers()
    const getDaily = vi.spyOn(dashboardApi, 'getDailyTokenStats').mockResolvedValue([daily])
    const getMonthly = vi.spyOn(dashboardApi, 'getMonthlyTokenStats').mockResolvedValue([monthly])
    const { wrapper } = setup(true)
    const { result, unmount } = renderHook(() => useTokenStatsViewModel(), { wrapper })
    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(getDaily).toHaveBeenCalledTimes(1)
    expect(getMonthly).toHaveBeenCalledTimes(1)
    expect(result.current.dailyRows[0].requestsLabel).toBe('12')
    expect(result.current.monthlyRows[0].requestsLabel).toBe('340')

    getDaily.mockResolvedValue([{ ...daily, request_count: 30 }])
    getMonthly.mockResolvedValue([{ ...monthly, request_count: 400 }])
    await act(() => vi.advanceTimersByTimeAsync(5000))
    expect(getDaily).toHaveBeenCalledTimes(2)
    expect(getMonthly).toHaveBeenCalledTimes(2)
    expect(result.current.dailyRows[0].requestsLabel).toBe('30')
    expect(result.current.monthlyRows[0].requestsLabel).toBe('400')

    unmount()
    await act(() => vi.advanceTimersByTimeAsync(10000))
    expect(getDaily).toHaveBeenCalledTimes(2)
    expect(getMonthly).toHaveBeenCalledTimes(2)
  })

  it('adds no dashboard notification dependency or WebSocket connection', async () => {
    const getDaily = vi.spyOn(dashboardApi, 'getDailyTokenStats').mockResolvedValue([daily])
    const getMonthly = vi.spyOn(dashboardApi, 'getMonthlyTokenStats').mockResolvedValue([monthly])
    const { client, wrapper } = setup()
    const { result } = renderHook(() => useTokenStatsViewModel(), { wrapper })
    await waitFor(() => expect(result.current.loadingDaily || result.current.loadingMonthly).toBe(false))

    await act(async () => {
      for (const changed of DASHBOARD_RESOURCES) {
        await invalidateDashboardSubscriptions(client, { changed })
      }
    })
    expect(getDaily).toHaveBeenCalledExactlyOnceWith(7)
    expect(getMonthly).toHaveBeenCalledExactlyOnceWith(6)
    expect(FakeWebSocket.instances).toHaveLength(0)
  })
})
