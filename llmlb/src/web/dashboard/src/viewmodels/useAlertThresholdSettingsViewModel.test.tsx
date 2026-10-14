import type { ReactNode } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider, useQuery } from '@tanstack/react-query'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { clientsApi, type ClientRankingResponse } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { DASHBOARD_RESOURCES } from '@/lib/dashboardResources'
import { invalidateDashboardSubscriptions } from '@/hooks/dashboardSubscriptions'
import { deferred } from '@/test/render'
import { useAlertThresholdSettingsViewModel } from './useAlertThresholdSettingsViewModel'

type Threshold = { key: string; value: string }
const threshold = (value: string): Threshold => ({ key: 'ip_alert_threshold', value })
const ranking: ClientRankingResponse = { rankings: [], total_count: 0, page: 1, per_page: 20 }
const unrelatedKeys = [queryKeys.clientTimeline(), queryKeys.clientModels(), queryKeys.clientHeatmap(undefined),
  queryKeys.clientDetail('192.0.2.10'), queryKeys.clientApiKeys('192.0.2.10')]

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

describe('alert threshold settings ViewModel', () => {
  it('owns the default display, editing snapshot, cancel and Escape/Enter commands', async () => {
    const initial = deferred<Threshold>()
    vi.spyOn(clientsApi, 'getAlertThreshold').mockReturnValue(initial.promise)
    const update = vi.spyOn(clientsApi, 'updateAlertThreshold').mockResolvedValue(threshold('500'))
    const { wrapper } = setup()
    const { result } = renderHook(() => useAlertThresholdSettingsViewModel(), { wrapper })
    expect(result.current.thresholdLabel).toBe('100 requests')
    expect(result.current.editing).toBe(false)
    await act(async () => initial.resolve(threshold('250')))
    await waitFor(() => expect(result.current.thresholdLabel).toBe('250 requests'))
    act(() => result.current.startEditing())
    expect(result.current.inputValue).toBe('250')
    act(() => result.current.setInputValue('900'))
    act(() => result.current.cancelEditing())
    expect(result.current.editing).toBe(false)
    act(() => result.current.startEditing())
    expect(result.current.inputValue).toBe('250')
    act(() => result.current.setInputValue('800'))
    act(() => result.current.handleKeyDown('Escape'))
    expect(result.current.editing).toBe(false)
    expect(update).not.toHaveBeenCalled()
    act(() => result.current.startEditing())
    act(() => result.current.setInputValue('500'))
    act(() => result.current.handleKeyDown('Enter'))
    await waitFor(() => expect(update).toHaveBeenCalledExactlyOnceWith('500'))
  })

  it('keeps parseInt base ten validation and exposes pending without closing early', async () => {
    vi.spyOn(clientsApi, 'getAlertThreshold').mockResolvedValue(threshold('250'))
    const saving = deferred<Threshold>()
    const update = vi.spyOn(clientsApi, 'updateAlertThreshold').mockReturnValue(saving.promise)
    const { wrapper } = setup()
    const { result } = renderHook(() => useAlertThresholdSettingsViewModel(), { wrapper })
    await waitFor(() => expect(result.current.thresholdLabel).toBe('250 requests'))
    act(() => result.current.startEditing())
    for (const value of ['', '0', '-5', 'no number', '0x10']) {
      act(() => result.current.setInputValue(value))
      act(() => result.current.save())
    }
    expect(update).not.toHaveBeenCalled()
    act(() => result.current.setInputValue('12.9requests'))
    act(() => result.current.save())
    await waitFor(() => expect(result.current.isPending).toBe(true))
    expect(update).toHaveBeenCalledExactlyOnceWith('12')
    expect(result.current.editing).toBe(true)
    expect(result.current.inputValue).toBe('12.9requests')
  })

  it('closes on success before refreshed GETs settle, invalidating every ranking page and no other client cache', async () => {
    const refreshedThreshold = deferred<Threshold>()
    const refreshedRanking = deferred<ClientRankingResponse>()
    const getThreshold = vi.spyOn(clientsApi, 'getAlertThreshold')
      .mockResolvedValueOnce(threshold('250')).mockReturnValueOnce(refreshedThreshold.promise)
    const getRanking = vi.spyOn(clientsApi, 'getClientRanking')
      .mockResolvedValueOnce(ranking).mockReturnValueOnce(refreshedRanking.promise)
    vi.spyOn(clientsApi, 'updateAlertThreshold').mockResolvedValue(threshold('500'))
    const { client, wrapper } = setup()
    client.setQueryData(queryKeys.clientRanking(2, 20, '192.0.2.10'), ranking)
    unrelatedKeys.forEach((key) => client.setQueryData(key, []))
    const { result } = renderHook(() => {
      const settings = useAlertThresholdSettingsViewModel()
      useQuery({ queryKey: queryKeys.clientRanking(1, 20, undefined),
        queryFn: () => clientsApi.getClientRanking({ page: 1, per_page: 20, ip: undefined }) })
      return settings
    }, { wrapper })
    await waitFor(() => expect(result.current.thresholdLabel).toBe('250 requests'))
    await waitFor(() => expect(getRanking).toHaveBeenCalledTimes(1))
    act(() => result.current.startEditing())
    act(() => result.current.setInputValue('500'))
    act(() => result.current.save())
    await waitFor(() => expect(result.current.editing).toBe(false))
    expect(result.current.isPending).toBe(false)
    expect(result.current.thresholdLabel).toBe('250 requests')
    expect(getThreshold).toHaveBeenCalledTimes(2)
    expect(getRanking).toHaveBeenCalledTimes(2)
    expect(client.getQueryState(queryKeys.alertThreshold())?.fetchStatus).toBe('fetching')
    expect(client.getQueryState(queryKeys.clientRanking(2, 20, '192.0.2.10'))?.isInvalidated).toBe(true)
    for (const key of unrelatedKeys) expect(client.getQueryState(key)?.isInvalidated).toBe(false)
    await act(async () => {
      refreshedThreshold.resolve(threshold('500'))
      refreshedRanking.resolve(ranking)
    })
    await waitFor(() => expect(result.current.thresholdLabel).toBe('500 requests'))
  })

  it('retains a failed edit and input without refreshing either original key', async () => {
    const getThreshold = vi.spyOn(clientsApi, 'getAlertThreshold').mockResolvedValue(threshold('250'))
    const saving = deferred<Threshold>()
    vi.spyOn(clientsApi, 'updateAlertThreshold').mockReturnValue(saving.promise)
    const { client, wrapper } = setup()
    client.setQueryData(queryKeys.clientRanking(1, 20, undefined), ranking)
    const { result } = renderHook(() => useAlertThresholdSettingsViewModel(), { wrapper })
    await waitFor(() => expect(result.current.thresholdLabel).toBe('250 requests'))
    act(() => result.current.startEditing())
    act(() => result.current.setInputValue('500'))
    act(() => result.current.save())
    await waitFor(() => expect(result.current.isPending).toBe(true))
    await act(async () => saving.reject(new Error('save failed')))
    await waitFor(() => expect(result.current.isPending).toBe(false))
    expect(result.current.editing).toBe(true)
    expect(result.current.inputValue).toBe('500')
    expect(getThreshold).toHaveBeenCalledTimes(1)
    expect(client.getQueryState(queryKeys.clientRanking(1, 20, undefined))?.isInvalidated).toBe(false)
  })

  it('inherits provider polling without subscribing to any WebSocket resource', async () => {
    vi.useFakeTimers()
    const getThreshold = vi.spyOn(clientsApi, 'getAlertThreshold').mockResolvedValue(threshold('250'))
    const { client, wrapper } = setup(true)
    const { unmount } = renderHook(() => useAlertThresholdSettingsViewModel(), { wrapper })
    await act(() => vi.advanceTimersByTimeAsync(0))
    expect(getThreshold).toHaveBeenCalledTimes(1)
    for (const changed of DASHBOARD_RESOURCES) {
      await act(() => invalidateDashboardSubscriptions(client, { changed }))
    }
    expect(getThreshold).toHaveBeenCalledTimes(1)
    await act(() => vi.advanceTimersByTimeAsync(5000))
    expect(getThreshold).toHaveBeenCalledTimes(2)
    unmount()
    await act(() => vi.advanceTimersByTimeAsync(5000))
    expect(getThreshold).toHaveBeenCalledTimes(2)
  })

  it('still refreshes threshold and the ranking prefix when an in-flight save succeeds after unmount', async () => {
    vi.spyOn(clientsApi, 'getAlertThreshold').mockResolvedValue(threshold('250'))
    const saving = deferred<Threshold>()
    const update = vi.spyOn(clientsApi, 'updateAlertThreshold').mockReturnValue(saving.promise)
    const { client, wrapper } = setup()
    const rankingKeys = [queryKeys.clientRanking(1, 20, undefined), queryKeys.clientRanking(2, 20, '192.0.2.10')]
    rankingKeys.forEach((key) => client.setQueryData(key, ranking))
    unrelatedKeys.forEach((key) => client.setQueryData(key, []))
    const { result, unmount } = renderHook(() => useAlertThresholdSettingsViewModel(), { wrapper })
    await waitFor(() => expect(result.current.thresholdLabel).toBe('250 requests'))
    act(() => result.current.startEditing())
    act(() => result.current.setInputValue('500'))
    act(() => result.current.save())
    await waitFor(() => expect(update).toHaveBeenCalledTimes(1))
    unmount()
    await act(async () => saving.resolve(threshold('500')))
    await waitFor(() => expect(client.getQueryState(queryKeys.alertThreshold())?.isInvalidated).toBe(true))
    for (const key of rankingKeys) expect(client.getQueryState(key)?.isInvalidated).toBe(true)
    for (const key of unrelatedKeys) expect(client.getQueryState(key)?.isInvalidated).toBe(false)
  })
})
