import { act, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { dashboardApi, type LogEntry, type LogResponse } from '@/lib/api'
import { deferred, renderWithProviders } from '@/test/render'
import { LogViewer } from './LogViewer'

function entry(overrides: Partial<LogEntry> = {}): LogEntry {
  return {
    timestamp: '2026-10-01T00:00:00Z',
    level: 'INFO',
    target: 'llmlb::api',
    message: 'listening on 0.0.0.0:32768',
    ...overrides,
  }
}

function logs(entries: LogEntry[]): LogResponse {
  return { source: 'lb', entries }
}

const startup = entry()
const timeout = entry({
  timestamp: '2026-10-01T00:00:05Z',
  level: 'ERROR',
  target: undefined,
  message: 'upstream timed out',
})
const slow = entry({
  timestamp: '2026-10-01T00:00:09Z',
  level: 'WARN',
  target: 'llmlb::balancer',
  message: 'endpoint is slow',
})

function entryCount() {
  return within(screen.getByText('Log Viewer')).getByText(/^\d+$/).textContent
}

/**
 * jsdom implements neither object URLs nor downloads through anchor clicks.
 * Records what the component hands to the browser instead.
 */
function stubDownload() {
  const blobs: Blob[] = []
  const fileNames: string[] = []
  const createObjectURL = vi.fn((blob: Blob | MediaSource) => {
    blobs.push(blob as Blob)
    return 'blob:logs'
  })
  const revokeObjectURL = vi.fn()
  vi.stubGlobal(
    'URL',
    class extends URL {
      static createObjectURL = createObjectURL
      static revokeObjectURL = revokeObjectURL
    },
  )
  vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(function (
    this: HTMLAnchorElement,
  ) {
    fileNames.push(this.download)
  })
  return { blobs, fileNames, createObjectURL, revokeObjectURL }
}

afterEach(() => {
  vi.useRealTimers()
})

describe('LogViewer', () => {
  it('renders the fetched log entries', async () => {
    const getRouterLogs = vi
      .spyOn(dashboardApi, 'getRouterLogs')
      .mockResolvedValue(logs([startup, timeout]))
    renderWithProviders(<LogViewer />)

    expect(await screen.findByText('listening on 0.0.0.0:32768')).toBeInTheDocument()
    expect(getRouterLogs).toHaveBeenCalledExactlyOnceWith({ limit: 200 })
    expect(screen.getByText('INFO')).toBeInTheDocument()
    expect(screen.getByText('[llmlb::api]')).toBeInTheDocument()
    expect(screen.getByText('upstream timed out')).toBeInTheDocument()
    expect(screen.getByText('ERROR')).toBeInTheDocument()
    expect(entryCount()).toBe('2')
    expect(screen.queryByText('No logs available')).not.toBeInTheDocument()
  })

  it('shows the empty state when there are no logs', async () => {
    vi.spyOn(dashboardApi, 'getRouterLogs').mockResolvedValue(logs([]))
    renderWithProviders(<LogViewer />)

    await waitFor(() => expect(entryCount()).toBe('0'))
    expect(screen.getByText('No logs available')).toBeInTheDocument()
  })

  it('refetches the logs on refresh', async () => {
    const refreshed = deferred<LogResponse>()
    const getRouterLogs = vi
      .spyOn(dashboardApi, 'getRouterLogs')
      .mockResolvedValueOnce(logs([startup]))
      .mockReturnValueOnce(refreshed.promise)
    renderWithProviders(<LogViewer />)
    await screen.findByText('listening on 0.0.0.0:32768')

    await userEvent.setup().click(screen.getByRole('button', { name: 'Refresh' }))

    expect(getRouterLogs).toHaveBeenCalledTimes(2)
    expect(getRouterLogs).toHaveBeenLastCalledWith({ limit: 200 })
    expect(screen.getByRole('button', { name: 'Refresh' })).toBeDisabled()

    refreshed.resolve(logs([startup, timeout]))

    expect(await screen.findByText('upstream timed out')).toBeInTheDocument()
    expect(entryCount()).toBe('2')
    expect(screen.getByRole('button', { name: 'Refresh' })).toBeEnabled()
  })

  it('polls for new logs every five seconds', async () => {
    vi.useFakeTimers()
    const getRouterLogs = vi
      .spyOn(dashboardApi, 'getRouterLogs')
      .mockResolvedValueOnce(logs([startup]))
      .mockResolvedValue(logs([startup, timeout]))
    renderWithProviders(<LogViewer />)

    await act(() => vi.advanceTimersByTimeAsync(0))
    expect(screen.getByText('listening on 0.0.0.0:32768')).toBeInTheDocument()
    expect(screen.queryByText('upstream timed out')).not.toBeInTheDocument()

    await act(() => vi.advanceTimersByTimeAsync(4900))
    expect(getRouterLogs).toHaveBeenCalledTimes(1)

    await act(() => vi.advanceTimersByTimeAsync(200))
    expect(getRouterLogs).toHaveBeenCalledTimes(2)
    expect(screen.getByText('upstream timed out')).toBeInTheDocument()
  })

  it('filters the entries by level', async () => {
    vi.spyOn(dashboardApi, 'getRouterLogs').mockResolvedValue(logs([startup, timeout, slow]))
    renderWithProviders(<LogViewer />)
    await screen.findByText('listening on 0.0.0.0:32768')
    const user = userEvent.setup()

    await user.click(screen.getByRole('combobox'))
    await user.click(await screen.findByRole('option', { name: 'Error' }))

    expect(screen.getByText('upstream timed out')).toBeInTheDocument()
    expect(screen.queryByText('listening on 0.0.0.0:32768')).not.toBeInTheDocument()
    expect(screen.queryByText('endpoint is slow')).not.toBeInTheDocument()
    expect(entryCount()).toBe('1')

    await user.click(screen.getByRole('combobox'))
    await user.click(await screen.findByRole('option', { name: 'Debug' }))

    expect(screen.getByText('No logs available')).toBeInTheDocument()
    expect(entryCount()).toBe('0')

    await user.click(screen.getByRole('combobox'))
    await user.click(await screen.findByRole('option', { name: 'All' }))

    expect(screen.getByText('listening on 0.0.0.0:32768')).toBeInTheDocument()
    expect(screen.getByText('endpoint is slow')).toBeInTheDocument()
    expect(entryCount()).toBe('3')
  })

  it('downloads the entries as a text file', async () => {
    const download = stubDownload()
    vi.spyOn(dashboardApi, 'getRouterLogs').mockResolvedValue(logs([startup, timeout]))
    renderWithProviders(<LogViewer />)
    await screen.findByText('listening on 0.0.0.0:32768')

    await userEvent.setup().click(screen.getByRole('button', { name: 'Download' }))

    expect(await screen.findByText('Logs downloaded')).toBeInTheDocument()
    expect(download.fileNames).toHaveLength(1)
    expect(download.fileNames[0]).toMatch(/^logs-llmlb-\d{4}-\d{2}-\d{2}\.txt$/)
    expect(download.blobs).toHaveLength(1)
    expect(await download.blobs[0].text()).toBe(
      [
        '[2026-10-01T00:00:00Z] [INFO] [llmlb::api] listening on 0.0.0.0:32768',
        '[2026-10-01T00:00:05Z] [ERROR] upstream timed out',
      ].join('\n'),
    )
    expect(download.revokeObjectURL).toHaveBeenCalledExactlyOnceWith('blob:logs')
  })

  it('downloads only the entries that match the level filter', async () => {
    const download = stubDownload()
    vi.spyOn(dashboardApi, 'getRouterLogs').mockResolvedValue(logs([startup, timeout, slow]))
    renderWithProviders(<LogViewer />)
    await screen.findByText('listening on 0.0.0.0:32768')
    const user = userEvent.setup()

    await user.click(screen.getByRole('combobox'))
    await user.click(await screen.findByRole('option', { name: 'Warn' }))
    await user.click(screen.getByRole('button', { name: 'Download' }))

    expect(download.blobs).toHaveLength(1)
    expect(await download.blobs[0].text()).toBe(
      '[2026-10-01T00:00:09Z] [WARN] [llmlb::balancer] endpoint is slow',
    )
  })

  it('reports that there is nothing to download', async () => {
    const download = stubDownload()
    vi.spyOn(dashboardApi, 'getRouterLogs').mockResolvedValue(logs([]))
    renderWithProviders(<LogViewer />)
    await waitFor(() => expect(entryCount()).toBe('0'))

    await userEvent.setup().click(screen.getByRole('button', { name: 'Download' }))

    expect(await screen.findByText('No logs to download')).toBeInTheDocument()
    expect(download.createObjectURL).not.toHaveBeenCalled()
    expect(download.fileNames).toHaveLength(0)
  })
})
