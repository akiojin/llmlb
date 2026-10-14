import type { ReactNode } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { dashboardApi, type LogEntry, type LogResponse } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { DASHBOARD_RESOURCES } from '@/lib/dashboardResources'
import { invalidateDashboardSubscriptions } from '@/hooks/dashboardSubscriptions'
import * as toastHook from '@/hooks/use-toast'
import { deferred } from '@/test/render'
import { useLogViewerViewModel } from './useLogViewerViewModel'

function setup() {
  const client = new QueryClient({ defaultOptions: { queries: {
    retry: false, gcTime: Infinity, refetchInterval: 1000, refetchIntervalInBackground: true,
  } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
  return { client, wrapper }
}

const info: LogEntry = {
  timestamp: '2026-10-01T00:00:00Z', level: 'INFO', target: 'llmlb::api', message: 'listening',
}
const error: LogEntry = {
  timestamp: '2026-10-01T00:00:05Z', level: 'ERROR', message: 'timed out',
}
const warn: LogEntry = {
  timestamp: '2026-10-01T00:00:09Z', level: 'warn', target: 'llmlb::balancer',
}
const logs = (entries: LogEntry[]): LogResponse => ({ source: 'lb', entries })

function stubDownload() {
  const blobs: Blob[] = []
  const filenames: string[] = []
  const createObjectURL = vi.fn((blob: Blob | MediaSource) => {
    blobs.push(blob as Blob)
    return 'blob:logs'
  })
  const revokeObjectURL = vi.fn()
  vi.stubGlobal('URL', class extends URL {
    static createObjectURL = createObjectURL
    static revokeObjectURL = revokeObjectURL
  })
  vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(function (this: HTMLAnchorElement) {
    filenames.push(this.download)
  })
  return { blobs, filenames, createObjectURL, revokeObjectURL }
}

afterEach(() => vi.useRealTimers())

describe('log viewer ViewModel', () => {
  it('owns the original query key, level filter and formatted log rows', async () => {
    const response = deferred<LogResponse>()
    const getLogs = vi.spyOn(dashboardApi, 'getRouterLogs').mockReturnValue(response.promise)
    const { client, wrapper } = setup()
    const { result } = renderHook(() => useLogViewerViewModel(), { wrapper })
    expect(result.current.filteredLogs).toBeUndefined()
    expect(result.current.logCount).toBeUndefined()
    expect(result.current.levelFilter).toBe('all')
    expect(result.current.autoScroll).toBe(true)

    await act(async () => response.resolve(logs([info, error, warn])))
    await waitFor(() => expect(result.current.logCount).toBe(3))
    expect(getLogs).toHaveBeenCalledExactlyOnceWith({ limit: 200 })
    expect(client.getQueryData(queryKeys.routerLogs())).toEqual(logs([info, error, warn]))
    expect(result.current.filteredLogs).toEqual([
      { timeLabel: new Date(info.timestamp).toLocaleTimeString(), level: 'INFO', targetLabel: '[llmlb::api]', message: 'listening' },
      { timeLabel: new Date(error.timestamp).toLocaleTimeString(), level: 'ERROR', targetLabel: undefined, message: 'timed out' },
      { timeLabel: new Date(warn.timestamp).toLocaleTimeString(), level: 'warn', targetLabel: '[llmlb::balancer]', message: '' },
    ])

    act(() => result.current.setLevelFilter('error'))
    expect(result.current.logCount).toBe(1)
    expect(result.current.filteredLogs?.map((row) => row.message)).toEqual(['timed out'])
    act(() => result.current.setLevelFilter('debug'))
    expect(result.current.filteredLogs).toEqual([])
    expect(result.current.logCount).toBe(0)
    act(() => result.current.setLevelFilter('all'))
    expect(result.current.logCount).toBe(3)
  })

  it('keeps five-second polling independent of provider defaults and WS notifications, and stops on unmount', async () => {
    vi.useFakeTimers()
    const getLogs = vi.spyOn(dashboardApi, 'getRouterLogs').mockResolvedValue(logs([info]))
    const { client, wrapper } = setup()
    const { unmount } = renderHook(() => useLogViewerViewModel(), { wrapper })
    await act(() => vi.advanceTimersByTimeAsync(0))
    expect(getLogs).toHaveBeenCalledTimes(1)
    for (const changed of DASHBOARD_RESOURCES) {
      await act(() => invalidateDashboardSubscriptions(client, { changed }))
    }
    await act(() => vi.advanceTimersByTimeAsync(4999))
    expect(getLogs).toHaveBeenCalledTimes(1)
    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(getLogs).toHaveBeenCalledTimes(2)
    unmount()
    await act(() => vi.advanceTimersByTimeAsync(10000))
    expect(getLogs).toHaveBeenCalledTimes(2)
  })

  it('exposes refresh pending state while retaining the previous rows', async () => {
    const refreshed = deferred<LogResponse>()
    const getLogs = vi.spyOn(dashboardApi, 'getRouterLogs')
      .mockResolvedValueOnce(logs([info])).mockReturnValueOnce(refreshed.promise)
    const { wrapper } = setup()
    const { result } = renderHook(() => useLogViewerViewModel(), { wrapper })
    await waitFor(() => expect(result.current.logCount).toBe(1))
    act(() => result.current.handleRefresh())
    await waitFor(() => expect(result.current.isRefetching).toBe(true))
    expect(result.current.filteredLogs?.[0].message).toBe('listening')
    expect(getLogs).toHaveBeenCalledTimes(2)
    await act(async () => refreshed.resolve(logs([info, error])))
    await waitFor(() => expect(result.current.isRefetching).toBe(false))
    expect(result.current.logCount).toBe(2)
  })

  it('owns the viewport ref and scroll effect, respecting the auto-scroll toggle', async () => {
    const initial = deferred<LogResponse>()
    vi.spyOn(dashboardApi, 'getRouterLogs')
      .mockReturnValueOnce(initial.promise).mockResolvedValue(logs([info, error]))
    const { wrapper } = setup()
    const { result } = renderHook(() => useLogViewerViewModel(), { wrapper })
    const root = document.createElement('div')
    const viewport = document.createElement('div')
    viewport.setAttribute('data-radix-scroll-area-viewport', '')
    Object.defineProperty(viewport, 'scrollHeight', { configurable: true, value: 240 })
    root.append(viewport)
    result.current.scrollRef.current = root
    await act(async () => initial.resolve(logs([info])))
    await waitFor(() => expect(viewport.scrollTop).toBe(240))

    act(() => result.current.setAutoScroll(false))
    Object.defineProperty(viewport, 'scrollHeight', { configurable: true, value: 480 })
    act(() => result.current.handleRefresh())
    await waitFor(() => expect(result.current.logCount).toBe(2))
    expect(viewport.scrollTop).toBe(240)
    act(() => result.current.setAutoScroll(true))
    expect(viewport.scrollTop).toBe(480)
  })

  it('downloads only filtered raw log content with the original filename and revokes the object URL', async () => {
    const download = stubDownload()
    const toast = vi.spyOn(toastHook, 'toast')
    vi.spyOn(dashboardApi, 'getRouterLogs').mockResolvedValue(logs([info, error, warn]))
    const { wrapper } = setup()
    const { result } = renderHook(() => useLogViewerViewModel(), { wrapper })
    await waitFor(() => expect(result.current.logCount).toBe(3))
    act(() => result.current.setLevelFilter('warn'))
    const date = new Date().toISOString().slice(0, 10)
    act(() => result.current.handleDownload())
    expect(download.filenames).toEqual([`logs-llmlb-${date}.txt`])
    expect(download.blobs[0].type).toBe('text/plain')
    expect(await download.blobs[0].text()).toBe('[2026-10-01T00:00:09Z] [warn] [llmlb::balancer] ')
    expect(download.revokeObjectURL).toHaveBeenCalledExactlyOnceWith('blob:logs')
    expect(toast).toHaveBeenCalledWith({ title: 'Logs downloaded' })

    act(() => result.current.setLevelFilter('all'))
    act(() => result.current.handleDownload())
    expect(await download.blobs[1].text()).toBe([
      '[2026-10-01T00:00:00Z] [INFO] [llmlb::api] listening',
      '[2026-10-01T00:00:05Z] [ERROR] timed out',
      '[2026-10-01T00:00:09Z] [warn] [llmlb::balancer] ',
    ].join('\n'))
  })

  it('retains the empty-download and server-side-clear toasts without mutating cached logs', async () => {
    const download = stubDownload()
    const toast = vi.spyOn(toastHook, 'toast')
    const getLogs = vi.spyOn(dashboardApi, 'getRouterLogs').mockResolvedValue(logs([]))
    const { client, wrapper } = setup()
    const { result } = renderHook(() => useLogViewerViewModel(), { wrapper })
    await waitFor(() => expect(result.current.logCount).toBe(0))
    act(() => result.current.handleDownload())
    expect(toast).toHaveBeenCalledWith({ title: 'No logs to download', variant: 'destructive' })
    expect(download.createObjectURL).not.toHaveBeenCalled()
    act(() => result.current.handleClear())
    expect(toast).toHaveBeenCalledWith({ title: 'Log clearing is handled on the server side' })
    expect(getLogs).toHaveBeenCalledTimes(1)
    expect(client.getQueryData(queryKeys.routerLogs())).toEqual(logs([]))
  })
})
