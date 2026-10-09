import type { ReactNode } from 'react'
import { act, renderHook } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { endpointsApi } from '@/lib/api/endpoints'
import { useEndpointModelTpsViewModel } from './useEndpointModelTpsViewModel'

afterEach(() => vi.useRealTimers())

describe('endpoint TPS polling', () => {
  it('inherits provider polling when the owner does not supply an interval', async () => {
    vi.useFakeTimers()
    const getTps = vi.spyOn(endpointsApi, 'getModelTps').mockResolvedValue([])
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false, gcTime: Infinity, refetchInterval: 5000, refetchIntervalInBackground: true } },
    })
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    )
    const { unmount } = renderHook(() => useEndpointModelTpsViewModel('A'), { wrapper })
    await act(async () => {})
    expect(getTps).toHaveBeenCalledTimes(1)
    await act(() => vi.advanceTimersByTimeAsync(5000))
    expect(getTps).toHaveBeenCalledTimes(2)
    unmount()
  })
})
