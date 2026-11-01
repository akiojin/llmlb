import type { ReactNode } from 'react'
import { act, renderHook } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { dashboardApi, modelsApi, systemApi } from '@/lib/api'
import { useAuth } from '@/hooks/useAuth'
import { FakeWebSocket } from '@/test/fake-websocket'
import { useDashboardViewModel } from './useDashboardViewModel'

vi.mock('@/hooks/useAuth', () => ({ useAuth: vi.fn() }))

function auth(role: string | null, isLoading = false): ReturnType<typeof useAuth> {
  return {
    user: role ? { id: 'user', username: role, role, must_change_password: false } : null,
    isLoading,
    isLoggedIn: role !== null,
    login: vi.fn(), logout: vi.fn(), checkAuth: vi.fn(),
  }
}

beforeEach(() => {
  vi.spyOn(dashboardApi, 'getOverview').mockImplementation(() => new Promise(() => {}))
  vi.spyOn(dashboardApi, 'getActivity').mockImplementation(() => new Promise(() => {}))
  vi.spyOn(dashboardApi, 'getDailyTokenStats').mockImplementation(() => new Promise(() => {}))
  vi.spyOn(dashboardApi, 'getEndpoints').mockImplementation(() => new Promise(() => {}))
  vi.spyOn(dashboardApi, 'getRequestResponses').mockImplementation(() => new Promise(() => {}))
  vi.spyOn(systemApi, 'getSystem').mockImplementation(() => new Promise(() => {}))
  vi.spyOn(systemApi, 'getVersion').mockImplementation(() => new Promise(() => {}))
  vi.spyOn(modelsApi, 'getRegistered').mockImplementation(() => new Promise(() => {}))
})

function setup(initialAuth: ReturnType<typeof useAuth>) {
  vi.mocked(useAuth).mockReturnValue(initialAuth)
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
  const hook = renderHook(() => useDashboardViewModel(), { wrapper })
  const changeAuth = (next: ReturnType<typeof useAuth>) => {
    vi.mocked(useAuth).mockReturnValue(next)
    hook.rerender()
  }
  return { ...hook, changeAuth, client }
}

describe('Dashboard authenticated WebSocket lifecycle (Issue #953)', () => {
  it('does not open an unauthenticated socket when a mounted viewer logs out', () => {
    const { changeAuth } = setup(auth('viewer'))
    expect(FakeWebSocket.instances).toHaveLength(0)
    changeAuth(auth(null))
    expect(FakeWebSocket.instances).toHaveLength(0)
  })

  it.each([
    [null, false], [null, true], ['viewer', false], ['admin', true], ['org_admin', false],
  ] as const)('does not connect for role=%s loading=%s', (role, loading) => {
    setup(auth(role, loading))
    expect(FakeWebSocket.instances).toHaveLength(0)
  })

  it('connects only after admin authentication completes and closes on logout', () => {
    const { changeAuth, result } = setup(auth('admin', true))
    expect(FakeWebSocket.instances).toHaveLength(0)
    changeAuth(auth('admin'))
    const socket = FakeWebSocket.latest()
    const close = vi.spyOn(socket, 'close')
    act(() => socket.onopen?.())
    expect(result.current.activityConnectionStatus).toBe('live')
    changeAuth(auth(null))
    expect(close).toHaveBeenCalledOnce()
    expect(result.current.activityConnectionStatus).toBe('polling')
    expect(FakeWebSocket.instances).toHaveLength(1)
  })

  it('retains the polling fallback when a connected admin socket closes', () => {
    const { result } = setup(auth('admin'))
    act(() => FakeWebSocket.latest().onopen?.())
    expect(result.current.activityConnectionStatus).toBe('live')
    act(() => FakeWebSocket.latest().onclose?.())
    expect(result.current.activityConnectionStatus).toBe('polling')
  })
})
