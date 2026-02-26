import { test, expect } from '@playwright/test'
import {
  sortEndpoints,
  type EndpointSortRow,
} from '../../../../src/web/dashboard/src/components/dashboard/endpointSorting'

function createEndpoint(
  id: string,
  name: string,
  registeredAt: string
): EndpointSortRow {
  return {
    id,
    name,
    status: 'online',
    total_requests: 0,
    latency_ms: null,
    model_count: 0,
    registered_at: registeredAt,
  }
}

test.describe('Endpoint sort logic', () => {
  test('sorts endpoints by model count ascending and descending', async () => {
    const endpoints = [
      createEndpoint('ep-a', 'A', '2026-02-20T00:00:00.000Z'),
      createEndpoint('ep-b', 'B', '2026-02-21T00:00:00.000Z'),
      createEndpoint('ep-c', 'C', '2026-02-22T00:00:00.000Z'),
    ].map((endpoint, index) => ({
      ...endpoint,
      model_count: index + 1,
    }))

    const sortedAsc = sortEndpoints(endpoints, 'model_count', 'asc')
    const sortedDesc = sortEndpoints(endpoints, 'model_count', 'desc')

    expect(sortedAsc.map((endpoint) => endpoint.id)).toEqual([
      'ep-a',
      'ep-b',
      'ep-c',
    ])
    expect(sortedDesc.map((endpoint) => endpoint.id)).toEqual([
      'ep-c',
      'ep-b',
      'ep-a',
    ])
  })

  test('keeps null latency ordering consistent with current comparator', async () => {
    const endpoints = [
      createEndpoint('ep-null', 'Null', '2026-02-20T00:00:00.000Z'),
      createEndpoint('ep-low', 'Low', '2026-02-21T00:00:00.000Z'),
      createEndpoint('ep-missing', 'Missing', '2026-02-22T00:00:00.000Z'),
      createEndpoint('ep-high', 'High', '2026-02-23T00:00:00.000Z'),
    ].map((endpoint) => {
      if (endpoint.id === 'ep-low') return { ...endpoint, latency_ms: 25 }
      if (endpoint.id === 'ep-high') return { ...endpoint, latency_ms: 250 }
      return endpoint
    })

    const sortedAsc = sortEndpoints(endpoints, 'latency_ms', 'asc')
    const sortedDesc = sortEndpoints(endpoints, 'latency_ms', 'desc')

    expect(sortedAsc.map((endpoint) => endpoint.id)).toEqual([
      'ep-low',
      'ep-high',
      'ep-null',
      'ep-missing',
    ])
    expect(sortedDesc.map((endpoint) => endpoint.id)).toEqual([
      'ep-null',
      'ep-missing',
      'ep-high',
      'ep-low',
    ])
  })
})
