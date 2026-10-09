import type { ReactNode } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { endpointsApi, type DashboardEndpoint } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { invalidateDashboardSubscriptions, registerDashboardSubscription } from '@/hooks/dashboardSubscriptions'
import { useEndpointDetailViewModel } from './useEndpointDetailViewModel'
import { useEndpointModelsTableViewModel } from './useEndpointModelsTableViewModel'
import { useEndpointRequestChartViewModel } from './useEndpointRequestChartViewModel'

const { toast } = vi.hoisted(() => ({ toast: vi.fn() }))
vi.mock('@/hooks/use-toast', () => ({ toast }))

const endpoint: DashboardEndpoint = {
  id: 'A', name: 'alpha', base_url: 'http://alpha.test', status: 'online', endpoint_type: 'xllm',
  health_check_interval_secs: 30, inference_timeout_secs: 600, notes: 'original', error_count: 0,
  registered_at: '2026-10-01T00:00:00Z', model_count: 1,
  total_requests: 100, successful_requests: 94, failed_requests: 6, latency_ms: 0,
}

function setup(polling = false) {
  const client = new QueryClient({ defaultOptions: { queries: {
    retry: false, gcTime: Infinity, ...(polling ? { refetchInterval: 5000, refetchIntervalInBackground: true } : {}),
  } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
  return { client, wrapper }
}

afterEach(() => vi.useRealTimers())

describe('endpoint detail ViewModels', () => {
  it('keeps today acquisition gated by open and uses the supplied endpoint without a new detail GET', async () => {
    const today = vi.spyOn(endpointsApi, 'getTodayStats').mockResolvedValue({
      date: '2026-10-10', total_requests: 12, successful_requests: 10, failed_requests: 2,
    })
    const get = vi.spyOn(endpointsApi, 'get')
    const { wrapper } = setup()
    const { result, rerender } = renderHook(({ open }) => useEndpointDetailViewModel(endpoint, open, vi.fn()), {
      wrapper, initialProps: { open: false },
    })
    expect(today).not.toHaveBeenCalled()
    expect(get).not.toHaveBeenCalled()
    expect(result.current.totalRequestsLabel).toBe('100')
    expect(result.current.successRateLabel).toBe('94.0%')
    expect(result.current.requestHealth).toBe('warning')
    expect(result.current.latencyLabel).toBe('0ms')
    rerender({ open: true })
    await waitFor(() => expect(result.current.todayRequestsLabel).toBe('12'))
    expect(today).toHaveBeenCalledExactlyOnceWith('A')
    expect(get).not.toHaveBeenCalled()
  })

  it('inherits today polling while open and stops it when the modal closes', async () => {
    vi.useFakeTimers()
    const today = vi.spyOn(endpointsApi, 'getTodayStats').mockResolvedValue({
      date: '2026-10-10', total_requests: 0, successful_requests: 0, failed_requests: 0,
    })
    const { wrapper } = setup(true)
    const { result, rerender, unmount } = renderHook(({ open }) => useEndpointDetailViewModel(endpoint, open, vi.fn()), {
      wrapper, initialProps: { open: true },
    })
    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(result.current.todayRequestsLabel).toBe('-')
    expect(today).toHaveBeenCalledTimes(1)
    await act(() => vi.advanceTimersByTimeAsync(5000))
    expect(today).toHaveBeenCalledTimes(2)
    rerender({ open: false })
    await act(() => vi.advanceTimersByTimeAsync(10000))
    expect(today).toHaveBeenCalledTimes(2)
    unmount()
  })

  it('saves only changed fields and refreshes aggregate/same-id subscriptions without extending mutation pending', async () => {
    const update = vi.spyOn(endpointsApi, 'update').mockResolvedValue(endpoint)
    const { client, wrapper } = setup()
    const cleanup = [
      registerDashboardSubscription(client, ['endpoints'], queryKeys.endpoint('A'), { id: 'A' }),
      registerDashboardSubscription(client, ['endpoints'], queryKeys.endpoint('B'), { id: 'B' }),
    ]
    const invalidate = vi.spyOn(client, 'invalidateQueries').mockReturnValue(new Promise<void>(() => {}))
    const { result, unmount } = renderHook(() => useEndpointDetailViewModel(endpoint, false, vi.fn()), { wrapper })
    act(() => {
      result.current.setName('renamed')
      result.current.setHealthCheckInterval('45')
    })
    act(() => result.current.handleSave())
    await waitFor(() => expect(toast).toHaveBeenCalledWith({ title: 'Update Complete', description: 'Endpoint settings updated' }))
    expect(update).toHaveBeenCalledWith('A', { name: 'renamed', notes: undefined,
      health_check_interval_secs: 45, inference_timeout_secs: undefined })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: queryKeys.dashboardEndpoints() })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: queryKeys.endpoint('A') })
    expect(invalidate).not.toHaveBeenCalledWith({ queryKey: queryKeys.endpoint('B') })
    expect(result.current.isSaving).toBe(false)
    unmount()
    cleanup.forEach((remove) => remove())
    invalidate.mockClear()
    await invalidateDashboardSubscriptions(client, { changed: 'endpoints' })
    expect(invalidate).not.toHaveBeenCalled()
  })

  it('preserves connection/sync success and failure notifications and Playground close behavior', async () => {
    const testConnection = vi.spyOn(endpointsApi, 'test').mockResolvedValue({ success: false, message: 'unreachable' })
    vi.spyOn(endpointsApi, 'sync').mockRejectedValue(new Error('sync unavailable'))
    const onOpenChange = vi.fn()
    const { wrapper } = setup()
    const { result } = renderHook(() => useEndpointDetailViewModel(endpoint, false, onOpenChange), { wrapper })
    act(() => result.current.testConnection())
    await waitFor(() => expect(toast).toHaveBeenCalledWith({ title: 'Connection Failed', description: 'unreachable', variant: 'destructive' }))
    expect(testConnection).toHaveBeenCalledExactlyOnceWith('A')
    act(() => result.current.syncModels())
    await waitFor(() => expect(toast).toHaveBeenCalledWith({ title: 'Sync Failed', description: 'Error: sync unavailable', variant: 'destructive' }))
    act(() => result.current.openPlayground())
    expect(window.location.hash).toBe('#playground/A')
    expect(onOpenChange).toHaveBeenCalledExactlyOnceWith(false)
  })

  it('merges model metadata, max production TPS and stats while preserving labels and row order', async () => {
    vi.spyOn(endpointsApi, 'getModels').mockResolvedValue({ endpoint_id: 'A', models: [{ model_id: 'base', max_tokens: 8192, canonical_name: 'canonical' }] })
    const tps = (model_id: string, value: number | null, count: number) => ({
      model_id, api_kind: 'chat_completions' as const, source: 'production' as const, tps: value,
      request_count: count, total_output_tokens: count * 10, average_duration_ms: 1500,
    })
    vi.spyOn(endpointsApi, 'getModelTps').mockResolvedValue([tps('base', 2, 4), tps('base', 9, 7), tps('tps-only', null, 0)])
    vi.spyOn(endpointsApi, 'getModelStats').mockResolvedValue([
      { model_id: 'base', total_requests: 100, successful_requests: 79, failed_requests: 21 },
      { model_id: 'stats-only', total_requests: 0, successful_requests: 0, failed_requests: 0 },
    ])
    const { wrapper } = setup()
    const { result } = renderHook(() => useEndpointModelsTableViewModel('A', true), { wrapper })
    await waitFor(() => expect(result.current.isLoading).toBe(false))
    expect(result.current.rows.map((row) => row.model_id)).toEqual(['base', 'tps-only', 'stats-only'])
    expect(result.current.rows[0]).toMatchObject({ canonical_name: 'canonical', contextLabel: '8K',
      tpsLabel: '9.0 tok/s', requestsLabel: '7', successRateLabel: '79.0%', requestHealth: 'error', durationLabel: '1.5 s' })
    expect(result.current.rows[1]).toMatchObject({ tpsLabel: '—', successRateLabel: '-', requestHealth: 'normal' })
    expect(result.current.rows[2]).toMatchObject({ contextLabel: '—', durationLabel: '—' })
  })

  it('preserves enabled, provider 5-second model/stats polling, 10-second TPS and scoped TPS subscription', async () => {
    vi.useFakeTimers()
    const models = vi.spyOn(endpointsApi, 'getModels').mockResolvedValue({ endpoint_id: 'A', models: [] })
    const tps = vi.spyOn(endpointsApi, 'getModelTps').mockResolvedValue([])
    const stats = vi.spyOn(endpointsApi, 'getModelStats').mockResolvedValue([])
    const { client, wrapper } = setup(true)
    const { rerender, unmount } = renderHook(({ enabled }) => useEndpointModelsTableViewModel('A', enabled), { wrapper, initialProps: { enabled: false } })
    await act(async () => {})
    expect(models).not.toHaveBeenCalled()
    expect(tps).not.toHaveBeenCalled()
    expect(stats).not.toHaveBeenCalled()
    rerender({ enabled: true })
    await act(async () => {})
    await act(() => vi.advanceTimersByTimeAsync(5000))
    expect(models).toHaveBeenCalledTimes(2)
    expect(stats).toHaveBeenCalledTimes(2)
    expect(tps).toHaveBeenCalledTimes(1)
    await act(() => vi.advanceTimersByTimeAsync(5000))
    expect(models).toHaveBeenCalledTimes(3)
    expect(stats).toHaveBeenCalledTimes(3)
    expect(tps).toHaveBeenCalledTimes(2)
    const invalidate = vi.spyOn(client, 'invalidateQueries')
    await act(async () => {
      await invalidateDashboardSubscriptions(client, { changed: 'tps', id: 'B' })
      await invalidateDashboardSubscriptions(client, { changed: 'metrics', id: 'A' })
    })
    expect(invalidate).not.toHaveBeenCalled()
    await act(async () => { await invalidateDashboardSubscriptions(client, { changed: 'tps', id: 'A' }) })
    expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: queryKeys.endpointModelTps('A') })
    unmount()
    invalidate.mockClear()
    await invalidateDashboardSubscriptions(client, { changed: 'tps' })
    expect(invalidate).not.toHaveBeenCalled()
  })

  it('retains chart id gating, period-specific keys and provider polling with date fallbacks', async () => {
    vi.useFakeTimers()
    const daily = vi.spyOn(endpointsApi, 'getDailyStats').mockResolvedValue([
      { date: '2026-10-10', total_requests: 5, successful_requests: 4, failed_requests: 1 },
      { date: 'unknown', total_requests: 0, successful_requests: 0, failed_requests: 0 },
    ])
    const { wrapper } = setup(true)
    const { result, rerender } = renderHook(({ id }) => useEndpointRequestChartViewModel(id), { wrapper, initialProps: { id: '' } })
    await act(async () => {})
    expect(daily).not.toHaveBeenCalled()
    rerender({ id: 'A' })
    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(daily).toHaveBeenLastCalledWith('A', 7)
    expect(result.current.chartData).toEqual([{ date: '10/10', successful: 4, failed: 1 }, { date: 'unknown', successful: 0, failed: 0 }])
    act(() => result.current.setDays('30'))
    await act(async () => {})
    expect(daily).toHaveBeenLastCalledWith('A', 30)
    await act(() => vi.advanceTimersByTimeAsync(5000))
    expect(daily).toHaveBeenCalledTimes(3)
    act(() => result.current.setDays('90'))
    await act(async () => {})
    expect(daily).toHaveBeenLastCalledWith('A', 90)
  })
})
