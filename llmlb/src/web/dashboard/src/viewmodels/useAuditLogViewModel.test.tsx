import type { ReactNode } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { describe, expect, it, vi } from 'vitest'
import { AuthProvider } from '@/hooks/useAuth'
import { auditLogApi, authApi, type AuditLogEntry } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { adminUser, deferred, viewerUser } from '@/test/render'
import { useAuditLogViewModel } from './useAuditLogViewModel'

function renderViewModel(user = adminUser) {
  expect(Object.prototype.hasOwnProperty.call(globalThis.setTimeout, 'clock'), 'a previous test must not leak a fake timer').toBe(false)
  vi.spyOn(authApi, 'me').mockResolvedValue(user)
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <AuthProvider>{children}</AuthProvider>
    </QueryClientProvider>
  )
  return { ...renderHook(() => useAuditLogViewModel(), { wrapper }), queryClient }
}

function entry(overrides: Partial<AuditLogEntry> = {}): AuditLogEntry {
  return {
    id: 1, timestamp: '2026-10-01T00:00:00Z', http_method: 'POST',
    request_path: '/api/endpoints', status_code: 201, actor_type: 'user',
    actor_id: 'user-admin', actor_username: 'admin', api_key_owner_id: null,
    client_ip: '192.0.2.10', duration_ms: 0, input_tokens: null,
    output_tokens: null, total_tokens: 0, model_name: null, endpoint_id: null,
    detail: null, is_migrated: false, ...overrides,
  }
}

function mockList(items: AuditLogEntry[] = [], total = items.length) {
  return vi.spyOn(auditLogApi, 'list').mockResolvedValue({ items, total, page: 1, per_page: 50 })
}

describe('useAuditLogViewModel', () => {
  it('gates both list acquisition and the verification command by admin role', async () => {
    const list = mockList()
    const verify = vi.spyOn(auditLogApi, 'verify')
    const { result } = renderViewModel(viewerUser)
    await waitFor(() => expect(authApi.me).toHaveBeenCalled())
    await act(async () => { await result.current.verification.verify() })
    expect(result.current.canView).toBe(false)
    expect(list).not.toHaveBeenCalled()
    expect(verify).not.toHaveBeenCalled()
  })

  it('keeps filter values when paging and resets the page when a filter changes', async () => {
    const list = mockList([entry()], 120)
    const { result, queryClient } = renderViewModel()
    await waitFor(() => expect(result.current.totalPages).toBe(3))
    expect(result.current.currentPage).toBe(1)
    act(() => result.current.changePage(2))
    await waitFor(() => expect(list).toHaveBeenLastCalledWith({ page: 2, per_page: 50 }))
    act(() => result.current.changeFilter('status_code', '201'))
    await waitFor(() => expect(list).toHaveBeenLastCalledWith({ page: 1, per_page: 50, status_code: 201 }))
    expect(queryClient.getQueryData(queryKeys.auditLogs(result.current.filters))).toBeDefined()
    act(() => result.current.changePage(3))
    await waitFor(() => expect(list).toHaveBeenLastCalledWith({ page: 3, per_page: 50, status_code: 201 }))
    act(() => result.current.changeFilter('status_code', 'all'))
    expect(result.current.filters.status_code).toBeUndefined()
    expect(result.current.currentPage).toBe(1)
  })

  it('debounces the latest search for 300ms and clears an empty search', async () => {
    const list = mockList([entry()], 120)
    const { result } = renderViewModel()
    await waitFor(() => {
      expect(result.current.canView).toBe(true)
      expect(result.current.isLoading).toBe(false)
    })
    act(() => result.current.changePage(2))
    vi.useFakeTimers()
    try {
      act(() => result.current.changeSearch('log'))
      act(() => { vi.advanceTimersByTime(200) })
      act(() => result.current.changeSearch('login'))
      act(() => { vi.advanceTimersByTime(299) })
      expect(result.current.searchText).toBe('login')
      expect(result.current.filters.search).toBeUndefined()
      act(() => { vi.advanceTimersByTime(1) })
      expect(result.current.filters).toEqual({ page: 1, per_page: 50, search: 'login' })
      act(() => result.current.changeSearch(''))
      act(() => { vi.advanceTimersByTime(300) })
      expect(result.current.filters.search).toBeUndefined()
    } finally {
      vi.useRealTimers()
    }
    await waitFor(() => expect(list).toHaveBeenCalledWith({ page: 1, per_page: 50, search: 'login' }))
  })

  it('cancels a pending search when the screen unmounts', async () => {
    mockList()
    const { result, unmount } = renderViewModel()
    await waitFor(() => expect(result.current.canView).toBe(true))
    vi.useFakeTimers()
    const schedule = vi.spyOn(globalThis, 'setTimeout')
    const clear = vi.spyOn(globalThis, 'clearTimeout')
    try {
      act(() => result.current.changeSearch('pending'))
      const searchTimerIndex = schedule.mock.calls.findIndex(([, delay]) => delay === 300)
      expect(searchTimerIndex).toBeGreaterThanOrEqual(0)
      const searchTimer = schedule.mock.results[searchTimerIndex].value
      clear.mockClear()
      unmount()
      expect(clear).toHaveBeenCalledWith(searchTimer)
    } finally {
      // Clear Vitest's restore registry before switching timers; its next
      // automatic restore would otherwise reinstall the spies' fake originals.
      vi.restoreAllMocks()
      vi.useRealTimers()
    }
  })

  it('derives display values while distinguishing zero from missing data', async () => {
    mockList([entry(), entry({ id: 2, actor_username: null, client_ip: null, duration_ms: null, total_tokens: null })])
    const { result } = renderViewModel()
    await waitFor(() => expect(result.current.entries).toHaveLength(2))
    expect(result.current.entries[0]).toMatchObject({
      actorLabel: 'admin', clientHref: '?tab=clients&ip=192.0.2.10', durationLabel: '0ms', tokenLabel: '0',
    })
    expect(result.current.entries[0].timestampLabel).toBeTruthy()
    expect(result.current.entries[1]).toMatchObject({
      actorLabel: 'user-admin', clientHref: null, durationLabel: '-', tokenLabel: '-',
    })
  })

  it('preserves the previous verification result during a retry and a failure', async () => {
    mockList()
    const previous = { valid: true, batches_checked: 3, tampered_batch: null, message: null }
    const retry = deferred<typeof previous>()
    const verify = vi.spyOn(auditLogApi, 'verify').mockResolvedValueOnce(previous).mockReturnValueOnce(retry.promise)
    const { result } = renderViewModel()
    await waitFor(() => expect(result.current.canView).toBe(true))
    await act(async () => { await result.current.verification.verify() })
    let pending!: Promise<void>
    act(() => { pending = result.current.verification.verify() })
    expect(result.current.verification.isVerifying).toBe(true)
    expect(result.current.verification.result).toEqual(previous)
    await act(async () => {
      retry.reject(new Error('service unavailable'))
      await pending
    })
    expect(result.current.verification).toMatchObject({ isVerifying: false, result: previous, error: 'service unavailable' })
    verify.mockRejectedValueOnce('unknown')
    await act(async () => { await result.current.verification.verify() })
    expect(result.current.verification.error).toBe('Verification failed')
    verify.mockResolvedValueOnce(previous)
    await act(async () => { await result.current.verification.verify() })
    expect(result.current.verification.error).toBeNull()
  })
})
