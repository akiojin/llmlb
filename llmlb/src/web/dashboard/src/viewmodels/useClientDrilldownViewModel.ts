import { useQuery } from '@tanstack/react-query'
import {
  clientsApi,
  type ClientDetailResponse,
  type ClientApiKeyUsage,
  type ModelDistribution,
  type HourlyPattern,
} from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'

export interface ClientRecentRequestRow {
  id: string
  model: string
  timeLabel: string
  durationLabel: string
}

export interface ClientApiKeyRow {
  id: string
  name: string | null
  requestCountLabel: string
}

export interface ClientDrilldownViewModel {
  isLoading: boolean
  hasData: boolean
  totalRequestsLabel: string
  firstSeenLabel: string | null
  lastSeenLabel: string | null
  recentRequests: ClientRecentRequestRow[]
  modelDistribution: ModelDistribution[]
  hourlyPattern: HourlyPattern[]
  hasHourlyData: boolean
  apiKeys: ClientApiKeyRow[]
  formatHourlyRequests: (value: unknown) => [number, string]
  formatHour: (hour: unknown) => string
}

function formatDate(dateStr: string): string {
  try {
    return new Date(dateStr).toLocaleString()
  } catch {
    return dateStr
  }
}

function formatTime(dateStr: string): string {
  try {
    return new Date(dateStr).toLocaleTimeString()
  } catch {
    return dateStr
  }
}

function formatHourlyRequests(value: unknown): [number, string] {
  return [Number(value ?? 0), 'Requests']
}

function formatHour(hour: unknown): string {
  return `${String(hour ?? '')}:00`
}

export function useClientDrilldownViewModel(ip: string): ClientDrilldownViewModel {
  // The ranking table mounts this owner only while a client is expanded.
  const { data, isLoading } = useQuery<ClientDetailResponse>({
    queryKey: queryKeys.clientDetail(ip),
    queryFn: () => clientsApi.getClientDetail(ip),
  })
  const { data: apiKeysData } = useQuery<ClientApiKeyUsage[]>({
    queryKey: queryKeys.clientApiKeys(ip),
    queryFn: () => clientsApi.getClientApiKeys(ip),
  })

  const hourlyPattern = data?.hourly_pattern ?? []
  return {
    isLoading,
    hasData: !!data && data.total_requests !== 0,
    totalRequestsLabel: `${(data?.total_requests ?? 0).toLocaleString()} requests`,
    firstSeenLabel: data?.first_seen ? formatDate(data.first_seen) : null,
    lastSeenLabel: data?.last_seen ? formatDate(data.last_seen) : null,
    recentRequests: (data?.recent_requests ?? []).map((request) => ({
      id: request.id, model: request.model,
      timeLabel: formatTime(request.timestamp),
      durationLabel: request.duration_ms != null ? `${request.duration_ms}ms` : '-',
    })),
    modelDistribution: data?.model_distribution ?? [],
    hourlyPattern,
    hasHourlyData: hourlyPattern.some((point) => point.count !== 0),
    apiKeys: (apiKeysData ?? []).map((key) => ({
      id: key.api_key_id, name: key.name,
      requestCountLabel: key.request_count.toLocaleString(),
    })),
    formatHourlyRequests, formatHour,
  }
}
