import { test, expect } from '@playwright/test'
import {
  buildEndpointAggregateTpsMap,
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

test.describe('Endpoint TPS sort logic', () => {
  test('sorts endpoints by TPS ascending and descending', async () => {
    const endpoints = [
      createEndpoint('ep-a', 'A', '2026-02-20T00:00:00.000Z'),
      createEndpoint('ep-b', 'B', '2026-02-21T00:00:00.000Z'),
      createEndpoint('ep-c', 'C', '2026-02-22T00:00:00.000Z'),
    ]
    const endpointTpsByEndpointId = buildEndpointAggregateTpsMap([
      { endpoint_id: 'ep-a', aggregate_tps: 5.0 },
      { endpoint_id: 'ep-b', aggregate_tps: 1.5 },
      { endpoint_id: 'ep-c', aggregate_tps: 12.4 },
    ])

    const sortedAsc = sortEndpoints(
      endpoints,
      'tps',
      'asc',
      endpointTpsByEndpointId
    )
    const sortedDesc = sortEndpoints(
      endpoints,
      'tps',
      'desc',
      endpointTpsByEndpointId
    )

    expect(sortedAsc.map((endpoint) => endpoint.id)).toEqual([
      'ep-b',
      'ep-a',
      'ep-c',
    ])
    expect(sortedDesc.map((endpoint) => endpoint.id)).toEqual([
      'ep-c',
      'ep-a',
      'ep-b',
    ])
  })

  test('keeps null and missing TPS rows at the end for both directions', async () => {
    const endpoints = [
      createEndpoint('ep-null', 'Null', '2026-02-20T00:00:00.000Z'),
      createEndpoint('ep-low', 'Low', '2026-02-21T00:00:00.000Z'),
      createEndpoint('ep-missing', 'Missing', '2026-02-22T00:00:00.000Z'),
      createEndpoint('ep-high', 'High', '2026-02-23T00:00:00.000Z'),
    ]
    const endpointTpsByEndpointId = buildEndpointAggregateTpsMap([
      { endpoint_id: 'ep-null', aggregate_tps: null },
      { endpoint_id: 'ep-low', aggregate_tps: 2.5 },
      // ep-missing intentionally omitted
      { endpoint_id: 'ep-high', aggregate_tps: 9.0 },
    ])

    const sortedAsc = sortEndpoints(
      endpoints,
      'tps',
      'asc',
      endpointTpsByEndpointId
    )
    const sortedDesc = sortEndpoints(
      endpoints,
      'tps',
      'desc',
      endpointTpsByEndpointId
    )

    expect(sortedAsc.map((endpoint) => endpoint.id)).toEqual([
      'ep-low',
      'ep-high',
      'ep-null',
      'ep-missing',
    ])
    expect(sortedDesc.map((endpoint) => endpoint.id)).toEqual([
      'ep-high',
      'ep-low',
      'ep-null',
      'ep-missing',
    ])
  })
})
