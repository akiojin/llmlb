import type { ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, renderHook } from '@testing-library/react'
import type { ProxyOptions } from 'vite'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { FakeWebSocket } from '@/test/fake-websocket'
import viteConfig from '../../vite.config'
import { useWebSocket } from './useWebSocket'
import { useInvalidateOn } from './useInvalidateOn'
import { queryKeys } from '@/lib/queryKeys'

const ENDPOINT_ID = '11111111-2222-3333-4444-555555555555'

function setup(options: Parameters<typeof useWebSocket>[0] = {}) {
  const queryClient = new QueryClient()
  const invalidateQueries = vi.spyOn(queryClient, 'invalidateQueries')
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  )
  const hook = renderHook(() => useWebSocket(options), { wrapper })
  const invalidatedKeys = () => invalidateQueries.mock.calls.map(([filters]) => filters?.queryKey)
  return { ...hook, invalidatedKeys }
}

function receive(payload: unknown) {
  act(() => {
    FakeWebSocket.latest().receive(typeof payload === 'string' ? payload : JSON.stringify(payload))
  })
}

/**
 * The `server.proxy` rule the vite dev server applies to `pathname`.
 *
 * Mirrors Vite's own matching: a key starting with `^` is a RegExp, any other
 * key is a path prefix, and the first matching key wins.
 */
function devProxyRule(pathname: string): ProxyOptions | undefined {
  const rules = Object.entries(viteConfig.server?.proxy ?? {})
  const rule = rules.find(([context]) =>
    context.startsWith('^') ? new RegExp(context).test(pathname) : pathname.startsWith(context),
  )?.[1]
  return typeof rule === 'string' ? { target: rule } : rule
}

describe('useWebSocket subscription dispatch', () => {
  it('does not invalidate any query without a mounted subscriber', () => {
    const { invalidatedKeys } = setup()
    receive({ changed: 'endpoints', id: ENDPOINT_ID })
    expect(invalidatedKeys()).toEqual([])
  })

  it('invalidates only the keys declared by the mounted owner', () => {
    const queryClient = new QueryClient()
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    )
    renderHook(() => {
      useInvalidateOn(['endpoints'], queryKeys.auditLogs({ page: 1 }))
      useWebSocket()
    }, { wrapper })
    receive({ changed: 'endpoints', id: ENDPOINT_ID })
    expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: queryKeys.auditLogs({ page: 1 }) })
  })

  it('an unknown resource invalidates nothing', () => {
    const { invalidatedKeys } = setup()
    receive({ changed: 'SomethingTheClientDoesNotKnow', id: ENDPOINT_ID })
    expect(invalidatedKeys()).toEqual([])
  })

  // These names resolve on any plain object; they must not be taken for a rule.
  it.each(['constructor', 'toString', '__proto__'])('a resource named %s invalidates nothing', (changed) => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
    const { invalidatedKeys } = setup()

    receive({ changed })

    expect(invalidatedKeys()).toEqual([])
    expect(consoleError).not.toHaveBeenCalled()
  })

  it('a malformed message is reported and invalidates nothing', () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
    const { invalidatedKeys } = setup()

    receive('{not json')

    expect(invalidatedKeys()).toEqual([])
    expect(consoleError).toHaveBeenCalledWith('Failed to parse WebSocket message:', expect.anything())
  })
})

describe('useWebSocket connection', () => {
  it('connects to /ws/dashboard on the current host', () => {
    setup()

    expect(FakeWebSocket.latest().url).toBe(`ws://${window.location.host}/ws/dashboard`)
  })

  // Issue #805: the dev server runs on its own port, so a WebSocket path that
  // `vite.config.ts` does not relay never reaches the backend and the page
  // silently stops updating. The path is read from the socket the hook opens,
  // so changing either side alone fails here.
  it('the vite dev server relays the WebSocket path to the backend that serves /api', () => {
    setup()
    const { pathname } = new URL(FakeWebSocket.latest().url)

    const backend = devProxyRule('/api/dashboard/overview')?.target

    expect(backend).toBeDefined()
    expect(devProxyRule(pathname)).toMatchObject({ ws: true, target: backend })
  })

  it('does not connect while disabled', () => {
    setup({ enabled: false })

    expect(FakeWebSocket.instances).toHaveLength(0)
  })

  it('opens without any welcome frame and reports disconnect', () => {
    const onConnect = vi.fn()
    const onDisconnect = vi.fn()
    const onMessage = vi.fn()
    const { result } = setup({ onConnect, onDisconnect, onMessage })
    expect(result.current.isConnected).toBe(false)
    act(() => FakeWebSocket.latest().onopen?.())
    expect(result.current.isConnected).toBe(true)
    expect(result.current.lastEvent).toBeNull()
    expect(onConnect).toHaveBeenCalledTimes(1)
    expect(onMessage).not.toHaveBeenCalled()

    const change = { changed: 'endpoints', id: ENDPOINT_ID }
    receive(change)
    expect(result.current.lastEvent).toEqual(change)
    expect(onMessage).toHaveBeenCalledExactlyOnceWith(change)
    expect(onConnect).toHaveBeenCalledTimes(1)

    act(() => result.current.disconnect())
    expect(result.current.isConnected).toBe(false)
    expect(onDisconnect).toHaveBeenCalledTimes(1)
  })
})

describe('useWebSocket cleanup and reconnect isolation (Issue #953)', () => {
  afterEach(() => vi.useRealTimers())

  function lifecycle() {
    vi.useFakeTimers()
    const queryClient = new QueryClient()
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    const onConnect = vi.fn()
    const onDisconnect = vi.fn()
    const onMessage = vi.fn()
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    )
    const hook = renderHook(({ enabled }) => {
      useInvalidateOn(['endpoints'], queryKeys.dashboardOverview())
      return useWebSocket({ enabled, onConnect, onDisconnect, onMessage })
    }, { wrapper, initialProps: { enabled: true } })
    const socket = FakeWebSocket.latest()
    // Preserve already queued callbacks, even if cleanup detaches the handlers.
    const message = socket.onmessage!
    const late = {
      open: socket.onopen!, close: socket.onclose!, error: socket.onerror!,
      message: () => message({ data: JSON.stringify({ changed: 'endpoints' }) }),
    }
    return { ...hook, socket, late, invalidate, onConnect, onDisconnect, onMessage }
  }

  it.each(['disabled', 'unmounted', 'disconnected'] as const)('ignores old callbacks after %s cleanup', (mode) => {
    const h = lifecycle()
    act(() => h.socket.onopen?.())
    if (mode === 'disabled') h.rerender({ enabled: false })
    else if (mode === 'unmounted') h.unmount()
    else act(() => h.result.current.disconnect())
    h.onConnect.mockClear()
    h.onDisconnect.mockClear()
    h.onMessage.mockClear()
    h.invalidate.mockClear()
    const warning = vi.spyOn(console, 'warn').mockImplementation(() => {})
    act(() => {
      h.late.open(); h.late.message(); h.late.error(); h.late.close()
      vi.advanceTimersByTime(60000)
    })
    expect(FakeWebSocket.instances).toHaveLength(1)
    expect(h.onConnect).not.toHaveBeenCalled()
    expect(h.onDisconnect).not.toHaveBeenCalled()
    expect(h.onMessage).not.toHaveBeenCalled()
    expect(h.invalidate).not.toHaveBeenCalled()
    expect(warning).not.toHaveBeenCalled()
    if (mode !== 'unmounted') expect(h.result.current.isConnected).toBe(false)
  })

  it('cancels a queued reconnect and rejects a captured timer after disable/re-enable', () => {
    const h = lifecycle()
    const timeout = vi.spyOn(globalThis, 'setTimeout')
    act(() => h.socket.onclose?.())
    const reconnectTimer = timeout.mock.calls.find(([, delay]) => Number(delay) >= 1000)![0] as () => void
    h.rerender({ enabled: false })
    expect(vi.getTimerCount()).toBe(0)
    h.rerender({ enabled: true })
    const current = FakeWebSocket.latest()
    act(() => { current.onopen?.(); reconnectTimer() })
    expect(FakeWebSocket.instances).toHaveLength(2)
    expect(h.result.current.isConnected).toBe(true)
  })

  it('keeps a new session connected when the old session delivers late events', () => {
    const h = lifecycle()
    h.rerender({ enabled: false })
    h.rerender({ enabled: true })
    const current = FakeWebSocket.latest()
    act(() => current.onopen?.())
    h.onMessage.mockClear()
    h.invalidate.mockClear()
    act(() => { h.late.close(); h.late.open(); h.late.message() })
    expect(h.result.current.isConnected).toBe(true)
    expect(h.onMessage).not.toHaveBeenCalled()
    expect(h.invalidate).not.toHaveBeenCalled()
    act(() => current.receive(JSON.stringify({ changed: 'endpoints' })))
    expect(h.onMessage).toHaveBeenCalledOnce()
    expect(h.invalidate).toHaveBeenCalledOnce()
    act(() => vi.advanceTimersByTime(60000))
    expect(FakeWebSocket.instances).toHaveLength(2)
  })

  it('uses exponential backoff for ordinary disconnects and resets it after open', () => {
    const h = lifecycle()
    vi.spyOn(Math, 'random').mockReturnValue(0)
    act(() => h.socket.onclose?.())
    act(() => vi.advanceTimersByTime(999))
    expect(FakeWebSocket.instances).toHaveLength(1)
    act(() => vi.advanceTimersByTime(1))
    expect(FakeWebSocket.instances).toHaveLength(2)
    act(() => FakeWebSocket.latest().onclose?.())
    act(() => vi.advanceTimersByTime(1999))
    expect(FakeWebSocket.instances).toHaveLength(2)
    act(() => vi.advanceTimersByTime(1))
    expect(FakeWebSocket.instances).toHaveLength(3)
    act(() => { FakeWebSocket.latest().onopen?.(); FakeWebSocket.latest().onclose?.() })
    act(() => vi.advanceTimersByTime(1000))
    expect(FakeWebSocket.instances).toHaveLength(4)
  })

  it('does not let the exported reconnect bypass disabled or unmounted state', () => {
    const h = lifecycle()
    const reconnect = h.result.current.reconnect
    h.rerender({ enabled: false })
    act(() => reconnect())
    h.unmount()
    act(() => reconnect())
    expect(FakeWebSocket.instances).toHaveLength(1)
  })
})
