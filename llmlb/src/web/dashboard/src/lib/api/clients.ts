// Clients API

import { fetchWithAuth } from './client'

export interface ClientIpRanking {
  ip: string
  request_count: number
  last_seen: string
  is_alert: boolean
  api_key_count: number
}

export interface ClientRankingResponse {
  rankings: ClientIpRanking[]
  total_count: number
  page: number
  per_page: number
}

export interface UniqueIpTimelinePoint {
  hour: string
  unique_ips: number
}

export interface ModelDistribution {
  model: string
  request_count: number
  percentage: number
}

export interface HeatmapCell {
  day_of_week: number
  hour: number
  count: number
}

export interface ClientDetailResponse {
  total_requests: number
  first_seen: string | null
  last_seen: string | null
  recent_requests: ClientRecentRequest[]
  model_distribution: ModelDistribution[]
  hourly_pattern: HourlyPattern[]
}

export interface ClientRecentRequest {
  id: string
  timestamp: string
  model: string
  status: string
  duration_ms: number | null
}

export interface HourlyPattern {
  hour: number
  count: number
}

export interface ClientApiKeyUsage {
  api_key_id: string
  name: string | null
  request_count: number
}

export const clientsApi = {
  getClientRanking: (params?: { page?: number; per_page?: number }) =>
    fetchWithAuth<ClientRankingResponse>('/api/dashboard/clients', {
      params: params as Record<string, string | number | boolean | undefined>,
    }),
  getTimeline: () =>
    fetchWithAuth<UniqueIpTimelinePoint[]>('/api/dashboard/clients/timeline'),
  getModels: () =>
    fetchWithAuth<ModelDistribution[]>('/api/dashboard/clients/models'),
  getHeatmap: () =>
    fetchWithAuth<HeatmapCell[]>('/api/dashboard/clients/heatmap'),
  getClientDetail: (ip: string) =>
    fetchWithAuth<ClientDetailResponse>(`/api/dashboard/clients/${encodeURIComponent(ip)}/detail`),
  getClientApiKeys: (ip: string) =>
    fetchWithAuth<ClientApiKeyUsage[]>(`/api/dashboard/clients/${encodeURIComponent(ip)}/api-keys`),
  getAlertThreshold: () =>
    fetchWithAuth<{ key: string; value: string }>('/api/dashboard/settings/ip_alert_threshold'),
  updateAlertThreshold: (value: string) =>
    fetchWithAuth<{ key: string; value: string }>('/api/dashboard/settings/ip_alert_threshold', {
      method: 'PUT',
      body: JSON.stringify({ value }),
    }),
}
