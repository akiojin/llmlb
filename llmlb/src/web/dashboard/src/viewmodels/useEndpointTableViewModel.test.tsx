import type { ReactNode } from 'react'
import { act, renderHook } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { describe, expect, it } from 'vitest'
import type { DashboardEndpoint, EndpointType } from '@/lib/api/endpoints'
import { useEndpointTableViewModel } from './useEndpointTableViewModel'

const statuses = ['online', 'pending', 'offline', 'error'] as const
const types = [
  'xllm', 'ollama', 'vllm', 'sglang', 'lmdeploy', 'xinference', 'lemonade',
  'tgi', 'koboldcpp', 'lm_studio', 'llamacpp', 'strata', 'project_maya',
  'openai_compatible', 'unknown',
] as const satisfies readonly EndpointType[]

const endpoints: DashboardEndpoint[] = statuses.flatMap((status) => types.map((type) => ({
  id: `${status}-${type}`,
  name: `${status}-${type}`,
  base_url: `http://${status}-${type}.test:8080`,
  status,
  endpoint_type: type,
  health_check_interval_secs: 30,
  inference_timeout_secs: 600,
  error_count: 0,
  registered_at: '2026-10-01T00:00:00Z',
  model_count: 0,
  total_requests: 0,
  successful_requests: 0,
  failed_requests: 0,
})))

function setup() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
  return renderHook(() => useEndpointTableViewModel(endpoints), { wrapper })
}

describe('EndpointTable filter input contracts', () => {
  it.each(statuses)('accepts status %s from Select and resets the page', (status) => {
    const { result } = setup()
    act(() => result.current.nextPage())
    expect(result.current.currentPage).toBe(2)

    const onValueChange: (value: string) => void = result.current.setStatusFilter
    act(() => onValueChange(status))

    expect(result.current.statusFilter).toBe(status)
    expect(result.current.filteredCount).toBe(types.length)
    expect(result.current.currentPage).toBe(1)
    expect(result.current.paginatedEndpoints.every((row) => row.status === status)).toBe(true)
  })

  it.each(types)('accepts type %s from Select and resets the page', (type) => {
    const { result } = setup()
    act(() => result.current.nextPage())

    const onValueChange: (value: string) => void = result.current.setTypeFilter
    act(() => onValueChange(type))

    expect(result.current.typeFilter).toBe(type)
    expect(result.current.filteredCount).toBe(statuses.length)
    expect(result.current.currentPage).toBe(1)
    expect(result.current.paginatedEndpoints.every((row) => row.endpoint_type === type)).toBe(true)
  })

  it('combines status and type and restores all selections independently', () => {
    const { result } = setup()
    act(() => result.current.setStatusFilter('offline'))
    act(() => result.current.setTypeFilter('strata'))
    expect(result.current.paginatedEndpoints.map((row) => row.id)).toEqual(['offline-strata'])

    act(() => result.current.setStatusFilter('all'))
    expect(result.current.filteredCount).toBe(statuses.length)
    expect(result.current.typeFilter).toBe('strata')

    act(() => result.current.setTypeFilter('all'))
    expect(result.current.filteredCount).toBe(endpoints.length)
    expect(result.current.currentPage).toBe(1)
  })

  it.each(['', 'ONLINE', ' online', 'all ', 'unknown', '__proto__']) (
    'ignores invalid status %j without changing the filter or page', (value) => {
      const { result } = setup()
      act(() => result.current.setStatusFilter('online'))
      act(() => result.current.nextPage())
      expect(result.current.currentPage).toBe(2)
      const before = result.current.paginatedEndpoints

      const onValueChange: (value: string) => void = result.current.setStatusFilter
      act(() => onValueChange(value))

      expect(result.current.statusFilter).toBe('online')
      expect(result.current.currentPage).toBe(2)
      expect(result.current.paginatedEndpoints).toEqual(before)
    },
  )

  it.each(['', 'XLLM', ' xllm', 'all ', 'online', '__proto__']) (
    'ignores invalid type %j without changing the filter or page', (value) => {
      const { result } = setup()
      act(() => result.current.setStatusFilter('online'))
      act(() => result.current.nextPage())
      expect(result.current.currentPage).toBe(2)
      const before = result.current.paginatedEndpoints

      const onValueChange: (value: string) => void = result.current.setTypeFilter
      act(() => onValueChange(value))

      expect(result.current.typeFilter).toBe('all')
      expect(result.current.statusFilter).toBe('online')
      expect(result.current.currentPage).toBe(2)
      expect(result.current.paginatedEndpoints).toEqual(before)
    },
  )
})
