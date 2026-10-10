import type { ReactNode } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { catalogApi, endpointsApi, type CatalogSearchResult } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { registerDashboardSubscription, invalidateDashboardSubscriptions } from '@/hooks/dashboardSubscriptions'
import { useModelAddWizardViewModel } from './useModelAddWizardViewModel'
import { useModelDeleteDialogViewModel } from './useModelDeleteDialogViewModel'

const { toast } = vi.hoisted(() => ({ toast: vi.fn() }))
vi.mock('@/hooks/use-toast', () => ({ toast }))
const model: CatalogSearchResult = {
  repo_id: 'org/tiny', engine_names: { xllm: 'tiny', ollama: null, empty: '' },
  supports_download: ['xllm'],
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
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: Error) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
function catalog() {
  const search = vi.spyOn(catalogApi, 'search').mockResolvedValue({ models: [model] })
  const detail = vi.spyOn(catalogApi, 'getModel').mockResolvedValue({ ...model, siblings: [] })
  const recommend = vi.spyOn(catalogApi, 'recommendEndpoints').mockResolvedValue({ endpoints: [
    { id: 'A', name: 'first', endpoint_type: 'xllm', can_download: true, has_model: false },
    { id: 'B', name: 'second', endpoint_type: 'ollama', can_download: true, has_model: true },
    { id: 'C', name: 'unsupported', endpoint_type: 'vllm', can_download: false, has_model: false },
  ] })
  return { search, detail, recommend }
}
afterEach(() => { vi.useRealTimers(); toast.mockClear() })

describe('model add wizard ViewModel', () => {
  it('keeps the 300ms debounce, two-character minimum and timer cleanup', async () => {
    vi.useFakeTimers()
    const { search, detail, recommend } = catalog()
    const { wrapper } = setup()
    const { result, unmount } = renderHook(() => useModelAddWizardViewModel(), { wrapper })
    expect(search).not.toHaveBeenCalled()
    expect(detail).not.toHaveBeenCalled()
    expect(recommend).not.toHaveBeenCalled()
    act(() => result.current.handleSearchChange('a'))
    await act(() => vi.advanceTimersByTimeAsync(300))
    expect(search).not.toHaveBeenCalled()
    act(() => result.current.handleSearchChange('tiny'))
    await act(() => vi.advanceTimersByTimeAsync(299))
    expect(search).not.toHaveBeenCalled()
    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(search).toHaveBeenCalledExactlyOnceWith('tiny', 20)
    act(() => result.current.handleSearchChange('discard'))
    unmount()
    await act(() => vi.advanceTimersByTimeAsync(300))
    expect(search).toHaveBeenCalledTimes(1)
  })

  it('preserves detail/recommend step gates, inherited polling and catalog isolation from WS', async () => {
    vi.useFakeTimers()
    const { search, detail, recommend } = catalog()
    const { client, wrapper } = setup(true)
    const { result, unmount } = renderHook(() => useModelAddWizardViewModel(), { wrapper })
    act(() => result.current.handleSearchChange('tiny'))
    await act(() => vi.advanceTimersByTimeAsync(301))
    act(() => result.current.handleSelectModel(model))
    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(detail).toHaveBeenCalledExactlyOnceWith(model.repo_id)
    expect(recommend).not.toHaveBeenCalled()
    expect(result.current.compatibleEngineEntries).toEqual([['xllm', 'tiny']])
    act(() => result.current.handleProceedToEndpoints())
    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(recommend).toHaveBeenCalledExactlyOnceWith(model.repo_id)
    expect(result.current.downloadableEndpoints.map((ep) => ep.id)).toEqual(['A', 'B'])
    await act(() => invalidateDashboardSubscriptions(client, { changed: 'endpoints', id: 'A' }))
    expect(search).toHaveBeenCalledTimes(1)
    expect(detail).toHaveBeenCalledTimes(1)
    expect(recommend).toHaveBeenCalledTimes(1)
    await act(() => vi.advanceTimersByTimeAsync(5000))
    expect(search).toHaveBeenCalledTimes(2)
    expect(detail).toHaveBeenCalledTimes(2)
    expect(recommend).toHaveBeenCalledTimes(2)
    act(() => result.current.handleBack())
    act(() => result.current.handleBack())
    await act(() => vi.advanceTimersByTimeAsync(5000))
    expect(result.current.step).toBe('search')
    expect(search).toHaveBeenCalledTimes(3)
    expect(detail).toHaveBeenCalledTimes(2)
    expect(recommend).toHaveBeenCalledTimes(2)
    unmount()
  })

  it('downloads sequentially, continues after failure and refreshes only after the batch without waiting', async () => {
    catalog()
    const first = deferred<{ task_id: string }>()
    const second = deferred<{ task_id: string }>()
    const download = vi.spyOn(endpointsApi, 'downloadModel')
      .mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
    const { client, wrapper } = setup()
    const cleanupExtra = [
      registerDashboardSubscription(client, ['endpoints'], queryKeys.dashboardOverview()),
      registerDashboardSubscription(client, ['endpoints'], queryKeys.requestResponses()),
      registerDashboardSubscription(client, ['endpoints'], queryKeys.endpointModels('unrelated'), { id: 'unrelated' }),
    ]
    const refresh = deferred<void>()
    const invalidate = vi.spyOn(client, 'invalidateQueries').mockReturnValue(refresh.promise)
    const { result, unmount } = renderHook(() => useModelAddWizardViewModel(), { wrapper })
    await act(() => result.current.handleStartDownload())
    expect(download).not.toHaveBeenCalled()
    act(() => result.current.handleSelectModel(model))
    act(() => result.current.handleProceedToEndpoints())
    act(() => { result.current.toggleEndpoint('A'); result.current.toggleEndpoint('B') })
    let batch!: Promise<void>
    act(() => { batch = result.current.handleStartDownload() })
    await waitFor(() => expect(download).toHaveBeenCalledTimes(1))
    expect(result.current.downloadStatuses).toEqual({ A: 'downloading', B: 'pending' })
    expect(result.current.allDownloadsFinished).toBe(false)
    expect(invalidate).not.toHaveBeenCalled()
    await act(async () => { first.reject(new Error('first failed')); await Promise.resolve() })
    await waitFor(() => expect(download).toHaveBeenCalledTimes(2))
    expect(result.current.downloadStatuses).toEqual({ A: 'failed', B: 'downloading' })
    expect(invalidate).not.toHaveBeenCalled()
    unmount()
    await act(async () => { second.resolve({ task_id: 'second' }); await batch })
    expect(download.mock.calls).toEqual([
      ['A', { model: model.repo_id, hf_repo: model.repo_id }],
      ['B', { model: model.repo_id, hf_repo: model.repo_id }],
    ])
    expect(invalidate.mock.calls.map(([options]) => options?.queryKey)).toEqual([
      queryKeys.dashboardEndpoints(), queryKeys.models(),
    ])
    expect(toast).toHaveBeenCalledExactlyOnceWith({ title: 'Download requests sent',
      description: 'Initiated download of org/tiny to 2 endpoint(s)' })
    unmount()
    cleanupExtra.forEach((cleanup) => cleanup())
    invalidate.mockClear()
    await invalidateDashboardSubscriptions(client, { changed: 'endpoints' })
    expect(invalidate).not.toHaveBeenCalled()
  })
})

describe('model delete ViewModel', () => {
  const options = () => ({ modelId: 'org/tiny', endpointId: 'A', endpointName: 'first',
    endpointType: 'xllm', onOpenChange: vi.fn(), onDeleted: vi.fn() })

  it('refreshes only its three keys, closes immediately and retains the target across rerenders', async () => {
    const deletion = deferred<void>()
    const remove = vi.spyOn(endpointsApi, 'deleteModel').mockReturnValue(deletion.promise)
    const { client, wrapper } = setup()
    const cleanupExtra = [
      registerDashboardSubscription(client, ['endpoints'], queryKeys.dashboardOverview()),
      registerDashboardSubscription(client, ['endpoints'], queryKeys.requestResponses()),
      registerDashboardSubscription(client, ['endpoints'], queryKeys.endpointModels('unrelated'), { id: 'unrelated' }),
    ]
    const refresh = deferred<void>()
    const invalidate = vi.spyOn(client, 'invalidateQueries').mockReturnValue(refresh.promise)
    const props = options()
    const { result, rerender, unmount } = renderHook((value) => useModelDeleteDialogViewModel(value),
      { wrapper, initialProps: props })
    act(() => result.current.handleDelete())
    await waitFor(() => expect(result.current.isDeleting).toBe(true))
    expect(remove).toHaveBeenCalledExactlyOnceWith('A', 'org/tiny')
    expect(invalidate).not.toHaveBeenCalled()
    await act(async () => { deletion.resolve(); await Promise.resolve() })
    await waitFor(() => expect(result.current.isDeleting).toBe(false))
    expect(props.onOpenChange).toHaveBeenCalledExactlyOnceWith(false)
    expect(props.onDeleted).toHaveBeenCalledTimes(1)
    expect(new Set(invalidate.mock.calls.map(([value]) => JSON.stringify(value?.queryKey)))).toEqual(
      new Set([queryKeys.endpointModels('A'), queryKeys.dashboardEndpoints(), queryKeys.models()].map((key) => JSON.stringify(key))))
    expect(toast).toHaveBeenCalledWith({ title: 'Model deleted', description: 'org/tiny has been removed from first' })
    refresh.resolve()
    invalidate.mockClear()
    rerender({ ...props, endpointId: 'B' })
    act(() => result.current.handleDelete())
    await waitFor(() => expect(remove).toHaveBeenCalledTimes(2))
    await waitFor(() => expect(invalidate).toHaveBeenCalledTimes(3))
    expect(invalidate.mock.calls.map(([value]) => value?.queryKey)).toEqual([
      queryKeys.endpointModels('B'), queryKeys.dashboardEndpoints(), queryKeys.models(),
    ])
    unmount()
    cleanupExtra.forEach((cleanup) => cleanup())
    invalidate.mockClear()
    await invalidateDashboardSubscriptions(client, { changed: 'endpoints' })
    expect(invalidate).not.toHaveBeenCalled()
  })

  it('keeps a failed deletion open and preserves its error toast without refreshing', async () => {
    vi.spyOn(endpointsApi, 'deleteModel').mockRejectedValue(new Error('endpoint unreachable'))
    const { client, wrapper } = setup()
    const invalidate = vi.spyOn(client, 'invalidateQueries')
    const props = options()
    const { result } = renderHook(() => useModelDeleteDialogViewModel(props), { wrapper })
    act(() => result.current.handleDelete())
    await waitFor(() => expect(toast).toHaveBeenCalledWith({ title: 'Delete failed',
      description: 'endpoint unreachable', variant: 'destructive' }))
    expect(props.onOpenChange).not.toHaveBeenCalled()
    expect(props.onDeleted).not.toHaveBeenCalled()
    expect(invalidate).not.toHaveBeenCalled()
  })

  it.each(['xllm', 'ollama', 'vllm', 'lm_studio'])('retains deletion support for %s', (endpointType) => {
    const { wrapper } = setup()
    const { result } = renderHook(() => useModelDeleteDialogViewModel({ ...options(), endpointType }), { wrapper })
    expect(result.current.supportsDelete).toBe(['xllm', 'ollama'].includes(endpointType))
  })
})
