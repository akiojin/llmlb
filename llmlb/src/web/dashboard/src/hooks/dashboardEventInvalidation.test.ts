import { describe, expect, it } from 'vitest'
import { queryKeysToInvalidate } from './dashboardEventInvalidation'
import type { DashboardEvent } from './useWebSocket'

const ENDPOINT_ID = '11111111-2222-3333-4444-555555555555'

// Endpoint list/detail views, including the endpoint playground (['endpoint', id])
const ENDPOINT_LIFECYCLE_KEYS = [
  ['dashboard-overview'],
  ['dashboard-endpoints'],
  ['request-responses'],
  ['endpoint', ENDPOINT_ID],
]

// SPEC #582 T010: event type -> invalidated query keys
const matrix: Array<[string, DashboardEvent, unknown[]]> = [
  ['connected', { type: 'connected', message: 'Dashboard WebSocket connected' }, []],
  [
    'NodeRegistered',
    { type: 'NodeRegistered', data: { runtime_id: ENDPOINT_ID, status: 'pending' } },
    ENDPOINT_LIFECYCLE_KEYS,
  ],
  [
    'EndpointStatusChanged',
    {
      type: 'EndpointStatusChanged',
      data: { runtime_id: ENDPOINT_ID, old_status: 'online', new_status: 'offline' },
    },
    ENDPOINT_LIFECYCLE_KEYS,
  ],
  ['NodeRemoved', { type: 'NodeRemoved', data: { runtime_id: ENDPOINT_ID } }, ENDPOINT_LIFECYCLE_KEYS],
  ['MetricsUpdated', { type: 'MetricsUpdated', data: { runtime_id: ENDPOINT_ID } }, [['dashboard-overview']]],
  [
    'TpsUpdated',
    { type: 'TpsUpdated', data: { endpoint_id: ENDPOINT_ID, model_id: 'm', tps: 1 } },
    [['endpoint-model-tps', ENDPOINT_ID]],
  ],
  ['TpsUpdated without endpoint_id', { type: 'TpsUpdated', data: {} }, []],
  ['UpdateStateChanged', { type: 'UpdateStateChanged' }, [['system-info']]],
]

describe('queryKeysToInvalidate', () => {
  it.each(matrix)('%s', (_name, event, expected) => {
    expect(queryKeysToInvalidate(event)).toEqual(expected)
  })

  it('ignores the removed legacy NodeStatusChanged event name', () => {
    const legacy = { type: 'NodeStatusChanged' } as unknown as DashboardEvent
    expect(queryKeysToInvalidate(legacy)).toEqual([])
  })
})
