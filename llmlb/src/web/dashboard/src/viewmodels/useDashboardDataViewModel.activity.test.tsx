import type { ReactNode } from 'react'
import { act, renderHook } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { dashboardApi } from '@/lib/api/dashboard'
import { systemApi } from '@/lib/api/system'
import { FakeWebSocket } from '@/test/fake-websocket'
import { useWebSocket } from '@/hooks/useWebSocket'
import { queryKeys } from '@/lib/queryKeys'
import { useDashboardDataViewModel } from './useDashboardDataViewModel'

const activity = {
  timestamp: '2026-10-22T00:00:00Z',
  bucket_seconds: 1,
  window_seconds: 60,
  total: { active_requests: 2, queued_requests: 1, requests_per_second: Array(60).fill(0) },
  endpoints: [{ endpoint_id: 'ep-1', active_requests: 2, queued_requests: 0, requests_per_second: Array(60).fill(0) }],
}

beforeEach(() => {
  vi.spyOn(dashboardApi, 'getOverview').mockImplementation(() => new Promise(() => {}))
  vi.spyOn(dashboardApi, 'getEndpoints').mockImplementation(() => new Promise(() => {}))
  vi.spyOn(dashboardApi, 'getRequestResponses').mockImplementation(() => new Promise(() => {}))
  vi.spyOn(dashboardApi, 'getDailyTokenStats').mockResolvedValue([])
  vi.spyOn(systemApi, 'getSystem').mockImplementation(() => new Promise(() => {}))
  vi.useFakeTimers()
})

afterEach(() => vi.useRealTimers())

function setup(isViewer = false) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  )
  return {
    queryClient,
    ...renderHook(() => {
      const data = useDashboardDataViewModel({ pollingInterval: 10000, isViewer })
      useWebSocket({ enabled: !isViewer })
      return data
    }, { wrapper }),
  }
}

describe('dashboard request activity (Issue #937)', () => {
  it('reads today requests from the UTC daily count rather than lifetime totals and refreshes every five seconds', async () => {
    vi.setSystemTime(new Date('2026-10-23T03:00:00Z'))
    vi.spyOn(dashboardApi, 'getActivity').mockResolvedValue(activity)
    const getDaily = vi.mocked(dashboardApi.getDailyTokenStats).mockResolvedValue([
      { date: '2026-10-22', request_count: 999, total_input_tokens: 0, total_output_tokens: 0, total_tokens: 0 },
      { date: '2026-10-23', request_count: 27, total_input_tokens: 0, total_output_tokens: 0, total_tokens: 0 },
    ])
    const { result } = setup(true)
    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(getDaily).toHaveBeenCalledWith(1)
    expect(result.current.todayRequests).toBe(27)
    await act(() => vi.advanceTimersByTimeAsync(4999))
    expect(getDaily).toHaveBeenCalledTimes(2)
  })

  it.each([false, true])('polls activity every second independently of the overview (viewer=%s)', async (isViewer) => {
    const getActivity = vi.spyOn(dashboardApi, 'getActivity').mockResolvedValue(activity)
    const { result } = setup(isViewer)
    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(result.current.activityData).toEqual(activity)

    await act(() => vi.advanceTimersByTimeAsync(998))
    expect(getActivity).toHaveBeenCalledTimes(1)
    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(getActivity).toHaveBeenCalledTimes(2)
    expect(dashboardApi.getOverview).toHaveBeenCalledTimes(1)
    if (isViewer) {
      expect(FakeWebSocket.instances).toHaveLength(0)
      expect(dashboardApi.getEndpoints).not.toHaveBeenCalled()
    }
  })

  it('refreshes lightweight activity on metrics notifications and retains one-second polling while connected', async () => {
    const getActivity = vi.spyOn(dashboardApi, 'getActivity').mockResolvedValue(activity)
    const { result, queryClient } = setup()
    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(result.current.activityData).toEqual(activity)
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    act(() => FakeWebSocket.latest().onopen?.())
    act(() => FakeWebSocket.latest().receive(JSON.stringify({ changed: 'metrics', id: 'ep-1' })))
    expect(invalidate).toHaveBeenCalledWith({ queryKey: queryKeys.dashboardActivity() })
    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(getActivity).toHaveBeenCalledTimes(2)

    await act(() => vi.advanceTimersByTimeAsync(1000))
    expect(getActivity).toHaveBeenCalledTimes(3)
  })

  it('keeps the last sample and exposes failure instead of showing false idle', async () => {
    const getActivity = vi.spyOn(dashboardApi, 'getActivity').mockResolvedValue(activity)
    const { result } = setup()
    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(result.current.activityData).toEqual(activity)
    getActivity.mockRejectedValue(new Error('activity unavailable'))

    await act(() => vi.advanceTimersByTimeAsync(1000))
    expect(result.current.activityError).toBeInstanceOf(Error)
    expect(result.current.activityData?.total.active_requests).toBe(2)

    getActivity.mockResolvedValue({ ...activity, total: { ...activity.total, active_requests: 0 } })
    await act(() => vi.advanceTimersByTimeAsync(1000))
    expect(result.current.activityError).toBeNull()
    expect(result.current.activityData?.total.active_requests).toBe(0)
  })
})
