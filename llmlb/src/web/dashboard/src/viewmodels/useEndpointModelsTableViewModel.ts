import { useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { endpointsApi, type ModelTpsEntry } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { useEndpointModelTpsViewModel } from './useEndpointModelTpsViewModel'

export interface EndpointModelsTableRow {
  model_id: string
  canonical_name?: string | null
  contextLabel: string
  tpsLabel: string
  requestsLabel: string
  successRateLabel: string
  requestHealth: 'normal' | 'warning' | 'error'
  durationLabel: string
}

export interface EndpointModelsTableViewModel {
  rows: EndpointModelsTableRow[]
  isLoading: boolean
  modelsLabel: string
}

interface EndpointModel {
  model_id: string
  capabilities?: string[]
  max_tokens?: number | null
  last_checked?: string
  canonical_name?: string | null
}

interface ModelRow {
  model_id: string
  max_tokens?: number | null
  tps: number | null
  request_count: number
  total_output_tokens: number
  average_duration_ms: number | null
  successful_requests: number
  total_requests: number
  canonical_name?: string | null
}

function formatTps(tps: number | null): string {
  if (tps === null || tps === undefined) return '—'
  return `${tps.toFixed(1)} tok/s`
}

function formatDuration(ms: number | null): string {
  if (ms === null || ms === undefined) return '—'
  if (ms < 1000) return `${ms.toFixed(0)} ms`
  return `${(ms / 1000).toFixed(1)} s`
}

function formatSuccessRate(entry: ModelRow): string {
  if (entry.total_requests === 0) return '-'
  const rate = (entry.successful_requests / entry.total_requests) * 100
  return `${rate.toFixed(1)}%`
}

/** Preserve model/stats provider polling and compose the existing scoped TPS owner. */
export function useEndpointModelsTableViewModel(endpointId: string, enabled = true): EndpointModelsTableViewModel {
  const { data: modelsData, isLoading: modelsLoading } = useQuery({
    queryKey: queryKeys.endpointModels(endpointId),
    queryFn: () => endpointsApi.getModels(endpointId),
    enabled,
  })

  const { tpsEntries: modelTps, isLoading: tpsLoading } = useEndpointModelTpsViewModel(endpointId, {
    enabled,
    refetchInterval: 10000,
  })

  const { data: modelStats, isLoading: statsLoading } = useQuery({
    queryKey: queryKeys.endpointModelStats(endpointId),
    queryFn: () => endpointsApi.getModelStats(endpointId),
    enabled,
  })

  // Consolidate data from three sources using useMemo
  const consolidatedRows = useMemo(() => {
    const rows: Map<string, ModelRow> = new Map()

    // Start with models from getModels as the base
    const models: EndpointModel[] = modelsData?.models || []
    for (const model of models) {
      rows.set(model.model_id, {
        model_id: model.model_id,
        max_tokens: model.max_tokens,
        tps: null,
        request_count: 0,
        total_output_tokens: 0,
        average_duration_ms: null,
        successful_requests: 0,
        total_requests: 0,
        canonical_name: model.canonical_name,
      })
    }

    // Merge TPS data: find max TPS per model when multiple api_kind entries exist
    const tpsMap = new Map<string, ModelTpsEntry>()
    if (modelTps) {
      for (const entry of modelTps) {
        const existing = tpsMap.get(entry.model_id)
        if (!existing || (entry.tps ?? -1) > (existing.tps ?? -1)) {
          tpsMap.set(entry.model_id, entry)
        }
      }
    }

    for (const [modelId, tpsEntry] of tpsMap) {
      const row = rows.get(modelId) || {
        model_id: modelId,
        max_tokens: undefined,
        tps: null,
        request_count: 0,
        total_output_tokens: 0,
        average_duration_ms: null,
        successful_requests: 0,
        total_requests: 0,
      }
      row.tps = tpsEntry.tps
      row.request_count = tpsEntry.request_count
      row.total_output_tokens = tpsEntry.total_output_tokens
      row.average_duration_ms = tpsEntry.average_duration_ms
      rows.set(modelId, row)
    }

    // Merge stats data
    if (modelStats) {
      for (const stat of modelStats) {
        const row = rows.get(stat.model_id) || {
          model_id: stat.model_id,
          max_tokens: undefined,
          tps: null,
          request_count: 0,
          total_output_tokens: 0,
          average_duration_ms: null,
          successful_requests: 0,
          total_requests: 0,
        }
        row.successful_requests = stat.successful_requests
        row.total_requests = stat.total_requests
        rows.set(stat.model_id, row)
      }
    }

    return Array.from(rows.values())
  }, [modelsData, modelTps, modelStats])

  const isLoading = modelsLoading || tpsLoading || statsLoading

  const rows = consolidatedRows.map((entry): EndpointModelsTableRow => {
    const rate = entry.total_requests === 0 ? null : (entry.successful_requests / entry.total_requests) * 100
    return {
      model_id: entry.model_id, canonical_name: entry.canonical_name,
      contextLabel: entry.max_tokens ? `${(entry.max_tokens / 1024).toFixed(0)}K` : '—',
      tpsLabel: formatTps(entry.tps), requestsLabel: entry.request_count.toLocaleString(),
      successRateLabel: formatSuccessRate(entry),
      requestHealth: rate !== null && rate < 80 ? 'error' : rate !== null && rate < 95 ? 'warning' : 'normal',
      durationLabel: formatDuration(entry.average_duration_ms),
    }
  })
  return { rows, isLoading, modelsLabel: `Models (${rows.length})` }
}
