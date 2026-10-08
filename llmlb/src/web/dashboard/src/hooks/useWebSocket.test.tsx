import type { ReactNode } from 'react'
import { QueryClient, QueryClientProvider, type QueryKey } from '@tanstack/react-query'
import { act, renderHook } from '@testing-library/react'
import type { ProxyOptions } from 'vite'
import { describe, expect, it, vi } from 'vitest'
import { FakeWebSocket } from '@/test/fake-websocket'
import viteConfig from '../../vite.config'
import { DASHBOARD_EVENT_INVALIDATIONS } from './dashboardEventInvalidation'
import { useWebSocket } from './useWebSocket'
import { DASHBOARD_RESOURCES, type DashboardChange, type DashboardResource } from '@/lib/dashboardResources'

const ENDPOINT_ID = '11111111-2222-3333-4444-555555555555'

// Endpoint list/detail views, including the endpoint playground (['endpoint', id])
const ENDPOINT_LIFECYCLE_KEYS: QueryKey[] = [
  ['dashboard-overview'],
  ['dashboard-endpoints'],
  ['request-responses'],
  ['endpoint', ENDPOINT_ID],
]

/** Exhaustive over the wire resource union; new resources require a tested rule. */
const INVALIDATION_MATRIX: {
  [T in DashboardResource]: { event: DashboardChange & { changed: T }; invalidates: QueryKey[] }
} = {
  endpoints: { event: { changed: 'endpoints', id: ENDPOINT_ID }, invalidates: ENDPOINT_LIFECYCLE_KEYS },
  metrics: { event: { changed: 'metrics', id: ENDPOINT_ID }, invalidates: [['dashboard-overview']] },
  tps: { event: { changed: 'tps', id: ENDPOINT_ID }, invalidates: [['endpoint-model-tps', ENDPOINT_ID]] },
  system: { event: { changed: 'system' }, invalidates: [['system-info']] },
}

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

// Invalidation order carries no meaning; compare as sets.
const sorted = (keys: unknown[]) => keys.map((key) => JSON.stringify(key)).sort()

describe('useWebSocket query invalidation matrix', () => {
  it.each(Object.entries(INVALIDATION_MATRIX))('%s', (_type, { event, invalidates }) => {
    const { invalidatedKeys } = setup()

    receive(event)

    expect(sorted(invalidatedKeys())).toEqual(sorted(invalidates))
  })

  // SPEC #582 FR-048f: the hook invalidates from this table, so a rule added
  // to it without a row above is a rule that no test verifies.
  it('has a row for every resource in the table the hook invalidates from', () => {
    expect(Object.keys(INVALIDATION_MATRIX).sort()).toEqual(Object.keys(DASHBOARD_EVENT_INVALIDATIONS).sort())
  })

  it('tests every resource declared by the wire contract', () => {
    expect(Object.keys(INVALIDATION_MATRIX).sort()).toEqual([...DASHBOARD_RESOURCES].sort())
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

    receive(INVALIDATION_MATRIX.endpoints.event)
    expect(result.current.lastEvent).toEqual(INVALIDATION_MATRIX.endpoints.event)
    expect(onMessage).toHaveBeenCalledExactlyOnceWith(INVALIDATION_MATRIX.endpoints.event)
    expect(onConnect).toHaveBeenCalledTimes(1)

    act(() => result.current.disconnect())
    expect(result.current.isConnected).toBe(false)
    expect(onDisconnect).toHaveBeenCalledTimes(1)
  })
})
