import { describe, expect, it } from 'vitest'
import { queryKeysToInvalidate } from './dashboardEventInvalidation'
import type { DashboardChange } from '@/lib/dashboardResources'

const ENDPOINT_ID = '11111111-2222-3333-4444-555555555555'
const matrix: Array<[string, DashboardChange, unknown[]]> = [
  ['endpoints', { changed: 'endpoints', id: ENDPOINT_ID }, [
    ['dashboard-overview'], ['dashboard-endpoints'], ['request-responses'], ['endpoint', ENDPOINT_ID],
  ]],
  ['endpoints broadcast', { changed: 'endpoints' }, [
    ['dashboard-overview'], ['dashboard-endpoints'], ['request-responses'], ['endpoint'],
  ]],
  ['metrics', { changed: 'metrics', id: ENDPOINT_ID }, [['dashboard-overview']]],
  ['tps', { changed: 'tps', id: ENDPOINT_ID }, [['endpoint-model-tps', ENDPOINT_ID]]],
  ['tps broadcast', { changed: 'tps' }, [['endpoint-model-tps']]],
  ['system', { changed: 'system' }, [['system-info']]],
]

describe('queryKeysToInvalidate', () => {
  it.each(matrix)('%s', (_name, change, expected) => {
    expect(queryKeysToInvalidate(change)).toEqual(expected)
  })

  it.each(['unknown', 'constructor', 'toString', '__proto__'])('ignores unknown resource %s', (changed) => {
    expect(queryKeysToInvalidate({ changed } as DashboardChange)).toEqual([])
  })
})
