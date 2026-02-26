export interface EndpointSortRow {
  id: string
  name: string
  status: 'online' | 'pending' | 'offline' | 'error'
  total_requests: number
  latency_ms?: number | null
  model_count: number
  registered_at: string
}

export type EndpointSortField =
  | 'name'
  | 'status'
  | 'total_requests'
  | 'latency_ms'
  | 'model_count'
  | 'registered_at'

export type EndpointSortDirection = 'asc' | 'desc'

const STATUS_ORDER: Record<EndpointSortRow['status'], number> = {
  online: 0,
  pending: 1,
  offline: 2,
  error: 3,
}

function compareEndpoints(
  a: EndpointSortRow,
  b: EndpointSortRow,
  sortField: EndpointSortField,
  sortDirection: EndpointSortDirection
): number {
  let comparison = 0

  switch (sortField) {
    case 'name':
      comparison = a.name.localeCompare(b.name)
      break
    case 'status':
      comparison = STATUS_ORDER[a.status] - STATUS_ORDER[b.status]
      break
    case 'total_requests':
      comparison = a.total_requests - b.total_requests
      break
    case 'latency_ms':
      comparison = (a.latency_ms ?? Infinity) - (b.latency_ms ?? Infinity)
      break
    case 'model_count':
      comparison = a.model_count - b.model_count
      break
    case 'registered_at':
      comparison =
        new Date(a.registered_at).getTime() - new Date(b.registered_at).getTime()
      break
  }

  return sortDirection === 'asc' ? comparison : -comparison
}

export function sortEndpoints<T extends EndpointSortRow>(
  endpoints: T[],
  sortField: EndpointSortField,
  sortDirection: EndpointSortDirection
): T[] {
  return [...endpoints].sort((a, b) => compareEndpoints(a, b, sortField, sortDirection))
}
