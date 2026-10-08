import type { AuditLogFilters } from './api/audit-log'
import type { ModelsView } from './api/models'

/** Shared cache keys. Preserve names, parameter order and prefix boundaries. */
export const queryKeys = {
  dashboardOverview: () => ['dashboard-overview'] as const,
  dashboardEndpoints: () => ['dashboard-endpoints'] as const,
  systemInfo: () => ['system-info'] as const,
  version: () => ['version'] as const,
  requestResponses: () => ['request-responses'] as const,
  viewerModels: (view: ModelsView) => ['viewer-models', view] as const,
  loadBalancerPlaygroundModels: () => ['lb-playground-models'] as const,
  endpointPrefix: () => ['endpoint'] as const,
  endpoint: (endpointId: string | undefined) => ['endpoint', endpointId] as const,
  endpointModels: (endpointId: string) => ['endpoint-models', endpointId] as const,
  endpointModelTpsPrefix: () => ['endpoint-model-tps'] as const,
  endpointModelTps: (endpointId: string) => ['endpoint-model-tps', endpointId] as const,
  endpointModelStats: (endpointId: string) => ['endpoint-model-stats', endpointId] as const,
  endpointTodayStats: (endpointId: string | undefined) => ['endpoint-today-stats', endpointId] as const,
  endpointDailyStats: (endpointId: string, days: '7' | '30' | '90') =>
    ['endpoint-daily-stats', endpointId, days] as const,
  allModelStats: () => ['all-model-stats'] as const,
  models: () => ['models'] as const,
  catalogSearch: (query: string) => ['catalog-search', query] as const,
  catalogModel: (repoId: string | undefined) => ['catalog-model', repoId] as const,
  catalogRecommend: (repoId: string | undefined) => ['catalog-recommend', repoId] as const,
  invitations: () => ['invitations'] as const,
  apiKeys: () => ['api-keys'] as const,
  users: () => ['users'] as const,
  notificationSettings: () => ['notification-settings'] as const,
  alertThreshold: () => ['alert-threshold'] as const,
  // No arguments means prefix invalidation; keep the IP slot even when undefined.
  clientRanking: (...args: [] | [page: number, perPage: number, ip: string | undefined]) =>
    ['client-ranking', ...args] as const,
  clientTimeline: () => ['client-timeline'] as const,
  clientModels: () => ['client-models'] as const,
  clientHeatmap: (ip: string | undefined) => ['client-heatmap', ip] as const,
  clientDetail: (ip: string) => ['client-detail', ip] as const,
  clientApiKeys: (ip: string) => ['client-api-keys', ip] as const,
  tokenStatsDaily: () => ['token-stats-daily'] as const,
  tokenStatsMonthly: () => ['token-stats-monthly'] as const,
  routerLogs: () => ['router-logs'] as const,
  auditLogs: (filters: AuditLogFilters) => ['audit-logs', filters] as const,
}

export type DashboardQueryKey = ReturnType<(typeof queryKeys)[keyof typeof queryKeys]>
