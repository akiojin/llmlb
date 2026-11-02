import type { ReactNode } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { describe, expect, it, vi } from 'vitest'
import { dashboardApi } from '@/lib/api/dashboard'
import { ApiError } from '@/lib/api/client'
import { useEndpointFailureHistoryViewModel } from './useEndpointFailureHistoryViewModel'

const auth = vi.hoisted(() => ({ user: { role: 'admin' } as { role: string } | null }))
vi.mock('@/hooks/useAuth', () => ({ useAuth: () => auth }))

function setup() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return { wrapper: ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  ) }
}

describe('endpoint failure history (#943)', () => {
  it('requests only the endpoint failures and keys each server page separately', async () => {
    auth.user = { role: 'admin' }
    const fetch = vi.spyOn(dashboardApi, 'getRequestResponses').mockImplementation(async ({ page } = {}) => ({
      records: [{ id: `failure-${page}`, timestamp: '2026-10-25T12:34:56Z', model: 'long-model',
        endpoint_id: 'endpoint-A', endpoint_name: 'A', request_type: 'chat_completions', duration_ms: 120000,
        status: { type: 'error' as const, message: 'Recorded timeout' } }],
      total_count: 26, page: page ?? 1, per_page: 25,
    }))
    const { result } = renderHook(() => useEndpointFailureHistoryViewModel('endpoint-A'), setup())
    await waitFor(() => expect(result.current.history).toHaveLength(1))
    expect(fetch).toHaveBeenLastCalledWith({ endpoint_id: 'endpoint-A', status: 'error', page: 1, per_page: 25 })
    expect(result.current.history[0]).toMatchObject({ request_id: 'failure-1', error: 'Recorded timeout', node_id: 'endpoint-A' })
    expect(result.current.totalPages).toBe(2)
    act(() => result.current.setPage(2))
    await waitFor(() => expect(result.current.history[0]?.request_id).toBe('failure-2'))
    expect(fetch).toHaveBeenLastCalledWith({ endpoint_id: 'endpoint-A', status: 'error', page: 2, per_page: 25 })
  })

  it.each(['viewer', null])('does not fetch history without management access (%s)', role => {
    auth.user = role ? { role } : null
    const fetch = vi.spyOn(dashboardApi, 'getRequestResponses')
    const { result } = renderHook(() => useEndpointFailureHistoryViewModel('endpoint-A'), setup())
    expect(fetch).not.toHaveBeenCalled()
    expect(result.current.unavailableReason).toMatch(/permission/)
    expect(result.current.canRetry).toBe(false)
    act(() => result.current.retry())
    expect(fetch).not.toHaveBeenCalled()
  })

  it('keeps an out-of-range page distinct from an empty endpoint history', async () => {
    auth.user = { role: 'admin' }
    vi.spyOn(dashboardApi, 'getRequestResponses').mockResolvedValue({ records: [], total_count: 1, page: 2, per_page: 25 })
    const { result } = renderHook(() => useEndpointFailureHistoryViewModel('endpoint-A'), setup())
    act(() => result.current.setPage(2))
    await waitFor(() => expect(result.current.isEmptyPage).toBe(true))
    expect(result.current.isEmpty).toBe(false)
  })

  it.each([
    [new ApiError(403, 'Forbidden'), /permission/],
    [new Error('unavailable'), /could not be loaded/],
  ])('distinguishes unavailable history from a successful empty page', async (error, message) => {
    auth.user = { role: 'admin' }
    vi.spyOn(dashboardApi, 'getRequestResponses').mockRejectedValue(error)
    const { result } = renderHook(() => useEndpointFailureHistoryViewModel('endpoint-A'), setup())
    await waitFor(() => expect(result.current.unavailableReason).toMatch(message))
    expect(result.current.isEmpty).toBe(false)
  })

  it('explains that an empty retained history cannot determine the lifetime failure cause', async () => {
    auth.user = { role: 'admin' }
    vi.spyOn(dashboardApi, 'getRequestResponses').mockResolvedValue({ records: [], total_count: 0, page: 1, per_page: 25 })
    const { result } = renderHook(() => useEndpointFailureHistoryViewModel('endpoint-A'), setup())
    await waitFor(() => expect(result.current.isEmpty).toBe(true))
    expect(result.current.unavailableReason).toBeNull()
  })
})
