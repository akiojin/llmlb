import type { ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, renderHook } from '@testing-library/react'
import type { ProxyOptions } from 'vite'
import { describe, expect, it, vi } from 'vitest'
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
