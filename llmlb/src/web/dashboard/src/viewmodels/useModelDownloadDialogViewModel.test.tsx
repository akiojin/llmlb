import type { ReactNode } from 'react'
import { act, renderHook } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { endpointsApi, type DashboardEndpoint, type DownloadTask } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { invalidateDashboardSubscriptions } from '@/hooks/dashboardSubscriptions'
import { useModelDownloadDialogViewModel } from './useModelDownloadDialogViewModel'

const { toast } = vi.hoisted(() => ({ toast: vi.fn() }))
vi.mock('@/hooks/use-toast', () => ({ toast }))
const endpoint = { id: 'A', name: 'first', endpoint_type: 'xllm' } as DashboardEndpoint
const task = (status: DownloadTask['status'] = 'downloading', extra: Partial<DownloadTask> = {}): DownloadTask => ({
  task_id: 'task-A', model: 'tiny', status, progress: 35, speed_mbps: 1.25, eta_seconds: 65.9, ...extra,
})
function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((yes) => { resolve = yes })
  return { promise, resolve }
}
function setup(open = true, target: DashboardEndpoint | null = endpoint) {
  const client = new QueryClient({ defaultOptions: { queries: {
    retry: false, gcTime: Infinity, refetchInterval: 5000, refetchIntervalInBackground: true,
  } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
  const props = { endpoint: target, open, onOpenChange: vi.fn() }
  const hook = renderHook((value) => useModelDownloadDialogViewModel(value), { wrapper, initialProps: props })
  return { ...hook, client, props }
}
async function tick(ms = 1) { await act(() => vi.advanceTimersByTimeAsync(ms)) }
async function start(hook: ReturnType<typeof setup>) {
  act(() => hook.result.current.setModelName('tiny'))
  act(() => hook.result.current.handleDownload())
  await tick()
}
afterEach(() => { vi.useRealTimers(); toast.mockClear() })

describe('model download dialog ViewModel (#872)', () => {
  it('polls immediately then every 2 seconds, selects task id before model and preserves progress formatting', async () => {
    vi.useFakeTimers()
    vi.spyOn(endpointsApi, 'downloadModel').mockResolvedValue({ task_id: 'task-A' })
    const progress = vi.spyOn(endpointsApi, 'getDownloadProgress').mockResolvedValue({ tasks: [
      task('completed', { task_id: 'unrelated' }), task(),
    ] })
    const hook = setup()
    expect(progress).not.toHaveBeenCalled()
    await start(hook)
    expect(progress).toHaveBeenCalledTimes(1)
    expect(hook.result.current.status).toBe('downloading')
    expect(hook.result.current.progress).toBe(35)
    expect(hook.result.current.progressMessage).toBe('1.3 Mbps / ETA 1m 5s')
    await tick(1998)
    expect(progress).toHaveBeenCalledTimes(1)
    await tick(1)
    expect(progress).toHaveBeenCalledTimes(2)
    hook.unmount()
    await tick(10000)
    expect(progress).toHaveBeenCalledTimes(2)
  })

  it('keeps waiting/model fallback and retries temporary HTTP failures only at the 2 second poll', async () => {
    vi.useFakeTimers()
    vi.spyOn(endpointsApi, 'downloadModel').mockResolvedValue({ task_id: 'task-A' })
    const progress = vi.spyOn(endpointsApi, 'getDownloadProgress')
      .mockResolvedValueOnce({ tasks: [] }).mockRejectedValueOnce(new Error('temporary'))
      .mockResolvedValue({ tasks: [task('pending', { task_id: 'fallback', speed_mbps: undefined, eta_seconds: undefined })] })
    const hook = setup()
    await start(hook)
    expect(hook.result.current.progressMessage).toBe('Waiting for download to start...')
    await tick(2000)
    expect(hook.result.current.status).toBe('downloading')
    expect(toast).not.toHaveBeenCalled()
    await tick(1998)
    expect(progress).toHaveBeenCalledTimes(2)
    await tick(1)
    expect(progress).toHaveBeenCalledTimes(3)
    await tick() // React Query delivers the fetched snapshot on its notification timer.
    expect(hook.result.current.progressMessage).toBe('Downloading...')
    hook.unmount()
  })

  it('retains the last progress when a later response temporarily omits the task', async () => {
    vi.useFakeTimers()
    vi.spyOn(endpointsApi, 'downloadModel').mockResolvedValue({ task_id: 'task-A' })
    vi.spyOn(endpointsApi, 'getDownloadProgress').mockResolvedValueOnce({ tasks: [task()] })
      .mockResolvedValue({ tasks: [] })
    const hook = setup()
    await start(hook)
    expect(hook.result.current.progress).toBe(35)
    await tick(2000)
    expect(hook.result.current.progressMessage).toBe('Waiting for download to start...')
    expect(hook.result.current.progress).toBe(35)
    hook.unmount()
  })

  it('completes once, refreshes exactly the two original keys without waiting and stops polling', async () => {
    vi.useFakeTimers()
    vi.spyOn(endpointsApi, 'downloadModel').mockResolvedValue({ task_id: 'task-A' })
    const progress = vi.spyOn(endpointsApi, 'getDownloadProgress').mockResolvedValueOnce({ tasks: [task()] })
      .mockResolvedValue({ tasks: [task('completed')] })
    const hook = setup()
    const refresh = deferred<void>()
    const invalidate = vi.spyOn(hook.client, 'invalidateQueries').mockReturnValue(refresh.promise)
    await start(hook)
    act(() => hook.result.current.handleClose())
    expect(hook.props.onOpenChange).not.toHaveBeenCalled()
    expect(toast).toHaveBeenCalledWith({ title: 'Download in progress', description: 'Please wait for the download to complete' })
    await tick(2000)
    expect(hook.result.current.status).toBe('completed')
    expect(hook.result.current.progress).toBe(100)
    expect(hook.result.current.progressMessage).toBe('Download completed')
    expect(invalidate.mock.calls.map(([options]) => options?.queryKey)).toEqual([
      queryKeys.endpointModels('A'), queryKeys.dashboardEndpoints(),
    ])
    expect(toast.mock.calls.filter(([value]) => value.title === 'Download Completed')).toHaveLength(1)
    hook.rerender({ ...hook.props })
    await tick(10000)
    expect(progress).toHaveBeenCalledTimes(2)
    expect(invalidate).toHaveBeenCalledTimes(2)
    act(() => hook.result.current.handleClose())
    expect(hook.props.onOpenChange).toHaveBeenCalledExactlyOnceWith(false)
    refresh.resolve()
    hook.unmount()
  })

  it.each(['failed', 'cancelled'] as const)('stops on %s, retains the error and permits retry without old terminal data', async (status) => {
    vi.useFakeTimers()
    vi.spyOn(endpointsApi, 'downloadModel').mockResolvedValueOnce({ task_id: 'task-A' }).mockResolvedValue({ task_id: 'retry' })
    const progress = vi.spyOn(endpointsApi, 'getDownloadProgress').mockResolvedValueOnce({ tasks: [task(status, { error: 'disk full' })] })
      .mockResolvedValue({ tasks: [task('downloading', { task_id: 'retry' })] })
    const hook = setup()
    await start(hook)
    expect(hook.result.current.status).toBe('error')
    expect(hook.result.current.errorMessage).toBe('disk full')
    expect(hook.result.current.progressMessage).toBe('')
    await tick(10000)
    expect(progress).toHaveBeenCalledTimes(1)
    act(() => hook.result.current.handleDownload())
    await tick()
    expect(hook.result.current.status).toBe('downloading')
    expect(hook.result.current.errorMessage).toBe('')
    expect(progress).toHaveBeenCalledTimes(2)
    hook.unmount()
  })

  it.each(['close', 'unmount'] as const)('cancels in-flight progress on %s with no late completion update or toast', async (action) => {
    vi.useFakeTimers()
    vi.spyOn(endpointsApi, 'downloadModel').mockResolvedValue({ task_id: 'task-A' })
    const pending = deferred<{ tasks: DownloadTask[] }>()
    let signal: AbortSignal | undefined
    const progress = vi.spyOn(endpointsApi, 'getDownloadProgress').mockImplementation((_id, value) => {
      signal = value
      return pending.promise
    })
    const hook = setup()
    const invalidate = vi.spyOn(hook.client, 'invalidateQueries')
    await start(hook)
    expect(signal?.aborted).toBe(false)
    if (action === 'close') hook.rerender({ ...hook.props, open: false })
    else hook.unmount()
    expect(signal?.aborted).toBe(true)
    await act(async () => { pending.resolve({ tasks: [task('completed')] }); await pending.promise })
    await tick(10000)
    expect(progress).toHaveBeenCalledTimes(1)
    expect(invalidate).not.toHaveBeenCalled()
    expect(toast).not.toHaveBeenCalled()
    hook.unmount()
  })

  it('does not start polling after a late POST settles in a discarded session', async () => {
    vi.useFakeTimers()
    const pending = deferred<{ task_id: string }>()
    vi.spyOn(endpointsApi, 'downloadModel').mockReturnValue(pending.promise)
    const progress = vi.spyOn(endpointsApi, 'getDownloadProgress')
    const hook = setup()
    await start(hook)
    expect(hook.result.current.isDownloadPending).toBe(true)
    hook.unmount()
    await act(async () => { pending.resolve({ task_id: 'task-A' }); await pending.promise })
    await tick(10000)
    expect(progress).not.toHaveBeenCalled()
    expect(toast).not.toHaveBeenCalled()
  })

  it('preserves validation, mutation errors, endpoint gates and scoped WS isolation', async () => {
    vi.useFakeTimers()
    const download = vi.spyOn(endpointsApi, 'downloadModel').mockRejectedValue(new Error('request denied'))
    const progress = vi.spyOn(endpointsApi, 'getDownloadProgress')
    const hook = setup()
    act(() => hook.result.current.handleDownload())
    expect(download).not.toHaveBeenCalled()
    expect(toast).toHaveBeenCalledWith({ title: 'Model name required', description: 'Please enter a model name', variant: 'destructive' })
    act(() => hook.result.current.setModelName(' tiny '))
    act(() => hook.result.current.handleDownload())
    await tick()
    expect(download).toHaveBeenCalledExactlyOnceWith('A', { model: 'tiny' })
    expect(hook.result.current.status).toBe('error')
    expect(hook.result.current.errorMessage).toBe('request denied')
    expect(toast).toHaveBeenCalledWith({ title: 'Download Failed', description: 'request denied', variant: 'destructive' })
    await act(() => invalidateDashboardSubscriptions(hook.client, { changed: 'endpoints', id: 'A' }))
    expect(progress).not.toHaveBeenCalled()
    hook.unmount()
    for (const target of [null, { ...endpoint, endpoint_type: 'vllm' as const }]) {
      const unsupported = setup(true, target)
      expect(unsupported.result.current.supportsDownload).toBe(false)
      act(() => unsupported.result.current.handleDownload())
      unsupported.unmount()
    }
    const closed = setup(false)
    act(() => closed.result.current.handleDownload())
    closed.unmount()
    expect(download).toHaveBeenCalledTimes(1)
  })
})
