import type { ReactNode } from 'react'
import { QueryClient, QueryClientProvider, type QueryKey } from '@tanstack/react-query'
import { act, renderHook } from '@testing-library/react'
import type { ProxyOptions } from 'vite'
import { describe, expect, it, vi } from 'vitest'
import { FakeWebSocket } from '@/test/fake-websocket'
import viteConfig from '../../vite.config'
import { DASHBOARD_EVENT_INVALIDATIONS } from './dashboardEventInvalidation'
import { useWebSocket, type DashboardEvent, type DashboardEventType } from './useWebSocket'

const ENDPOINT_ID = '11111111-2222-3333-4444-555555555555'

// Endpoint list/detail views, including the endpoint playground (['endpoint', id])
const ENDPOINT_LIFECYCLE_KEYS: QueryKey[] = [
  ['dashboard-overview'],
  ['dashboard-endpoints'],
  ['request-responses'],
  ['endpoint', ENDPOINT_ID],
]

/**
 * SPEC #582 FR-049: event type -> invalidated query keys.
 *
 * The mapped type makes this table exhaustive over `DashboardEventType`:
 * adding or renaming an event type fails `tsc` until the table says which
 * query keys that event invalidates.
 */
const INVALIDATION_MATRIX: {
  [T in DashboardEventType]: { event: DashboardEvent & { type: T }; invalidates: QueryKey[] }
} = {
  connected: {
    event: { type: 'connected', message: 'Dashboard WebSocket connected' },
    invalidates: [],
  },
  NodeRegistered: {
    event: { type: 'NodeRegistered', data: { runtime_id: ENDPOINT_ID, status: 'pending' } },
    invalidates: ENDPOINT_LIFECYCLE_KEYS,
  },
  EndpointStatusChanged: {
    event: {
      type: 'EndpointStatusChanged',
      data: { runtime_id: ENDPOINT_ID, old_status: 'online', new_status: 'offline' },
    },
    invalidates: ENDPOINT_LIFECYCLE_KEYS,
  },
  NodeRemoved: {
    event: { type: 'NodeRemoved', data: { runtime_id: ENDPOINT_ID } },
    invalidates: ENDPOINT_LIFECYCLE_KEYS,
  },
  MetricsUpdated: {
    event: { type: 'MetricsUpdated', data: { runtime_id: ENDPOINT_ID, cpu_usage: 12.5 } },
    invalidates: [['dashboard-overview']],
  },
  TpsUpdated: {
    event: { type: 'TpsUpdated', data: { endpoint_id: ENDPOINT_ID, model_id: 'm', tps: 42 } },
    invalidates: [['endpoint-model-tps', ENDPOINT_ID]],
  },
  UpdateStateChanged: {
    event: { type: 'UpdateStateChanged' },
    invalidates: [['system-info']],
  },
}

/**
 * The canonical events that gained a publisher in #781, with the payload the
 * backend sends (SPEC #582 FR-048a-d). `MetricsUpdated` carries no resource
 * usage: llmlb does not observe it, so those fields arrive as null.
 */
const PUBLISHED_EVENTS = {
  NodeRegistered: {
    type: 'NodeRegistered',
    data: { runtime_id: ENDPOINT_ID, machine_name: 'gpu-1', ip_address: '192.0.2.10', status: 'pending' },
  },
  NodeRemoved: { type: 'NodeRemoved', data: { runtime_id: ENDPOINT_ID } },
  MetricsUpdated: {
    type: 'MetricsUpdated',
    data: { runtime_id: ENDPOINT_ID, cpu_usage: null, memory_usage: null, gpu_usage: null },
  },
} satisfies { [T in DashboardEventType]?: DashboardEvent & { type: T } }

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
  it('has a row for every event type in the table the hook invalidates from', () => {
    expect(Object.keys(INVALIDATION_MATRIX).sort()).toEqual(Object.keys(DASHBOARD_EVENT_INVALIDATIONS).sort())
  })

  it.each(Object.values(PUBLISHED_EVENTS))('$type as published by the backend', (event) => {
    const { invalidatedKeys } = setup()

    receive(event)

    expect(sorted(invalidatedKeys())).toEqual(sorted(INVALIDATION_MATRIX[event.type].invalidates))
  })

  it('TpsUpdated without endpoint_id invalidates nothing', () => {
    const { invalidatedKeys } = setup()

    receive({ type: 'TpsUpdated', data: { model_id: 'm', tps: 42 } })

    expect(invalidatedKeys()).toEqual([])
  })

  it('an event type outside DashboardEventType invalidates nothing', () => {
    const { invalidatedKeys } = setup()

    receive({ type: 'SomethingTheClientDoesNotKnow', data: { runtime_id: ENDPOINT_ID } })

    expect(invalidatedKeys()).toEqual([])
  })

  // These names resolve on any plain object; they must not be taken for a rule.
  it.each(['constructor', 'toString', '__proto__'])('an event type named %s invalidates nothing', (type) => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
    const { invalidatedKeys } = setup()

    receive({ type })

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

  it('exposes the connection state and the last received event', () => {
    const onMessage = vi.fn()
    const { result } = setup({ onMessage })
    expect(result.current.isConnected).toBe(false)

    act(() => FakeWebSocket.latest().onopen?.())
    receive(INVALIDATION_MATRIX.connected.event)

    expect(result.current.isConnected).toBe(true)
    expect(result.current.lastEvent).toEqual(INVALIDATION_MATRIX.connected.event)
    expect(onMessage).toHaveBeenCalledWith(INVALIDATION_MATRIX.connected.event)
  })
})
