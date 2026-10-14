import { useState, useMemo, type Dispatch, type SetStateAction } from 'react'
import { useQuery } from '@tanstack/react-query'
import { dashboardApi, type RegisteredModelView, type DashboardEndpoint, type LifecycleStatus,
  type ModelCapabilities, type ModelStatEntry } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { formatBytes } from '@/lib/utils'

export type SortField =
  | 'id'
  | 'bestStatus'
  | 'endpointCount'
  | 'totalRequests'
type SortDirection = 'asc' | 'desc'
export type SupportedApi =
  | 'chat_completions'
  | 'completions'
  | 'responses'
  | 'embeddings'
  | 'fine_tune'
  | 'inference'
  | 'audio_speech'
  | 'audio_transcription'
  | 'image_input'
  | 'image_generation'

export interface AggregatedModel {
  id: string
  bestStatus: LifecycleStatus
  ready: boolean
  supportedApis: SupportedApi[]
  maxTokens?: number | null
  source?: string
  tags: string[]
  description?: string
  repo?: string
  filename?: string
  requiredMemoryBytes?: number
  chatTemplate?: string
  endpointIds: string[]
  endpointCount: number
  endpointCountLabel: string
  endpointSourcesLabel: string
  maxTokensLabel: string
  requiredMemoryLabel: string
  lifecycleLabel: string
  canonicalName?: string
  aliases: string[]
  isCanonical: boolean
}

function emptyCapabilities(): ModelCapabilities {
  return {
    chat_completion: false,
    completion: false,
    embeddings: false,
    fine_tune: false,
    inference: false,
    text_to_speech: false,
    speech_to_text: false,
    image_input: false,
    image_generation: false,
  }
}

function normalizeSupportedApi(api: string): SupportedApi | null {
  switch (api) {
    case 'chat':
    case 'chat_completion':
    case 'chat_completions':
      return 'chat_completions'
    case 'completion':
    case 'completions':
      return 'completions'
    case 'response':
    case 'responses':
      return 'responses'
    case 'embedding':
    case 'embeddings':
      return 'embeddings'
    case 'fine_tune':
    case 'fine_tuning':
      return 'fine_tune'
    case 'inference':
      return 'inference'
    case 'text_to_speech':
    case 'tts':
    case 'audio_speech':
      return 'audio_speech'
    case 'speech_to_text':
    case 'asr':
    case 'audio_transcription':
    case 'audio_transcriptions':
      return 'audio_transcription'
    case 'image':
    case 'images':
    case 'image_input':
    case 'vision':
    case 'visual':
    case 'multimodal':
      return 'image_input'
    case 'image_generation':
    case 'images_generations':
      return 'image_generation'
    default:
      return null
  }
}

function uniqueApis(apis: SupportedApi[]): SupportedApi[] {
  return Array.from(new Set(apis))
}

function supportedApisFromCapabilities(capabilities?: ModelCapabilities): SupportedApi[] {
  const caps = capabilities ?? emptyCapabilities()
  return uniqueApis([
    ...(caps.chat_completion ? ['chat_completions' as const] : []),
    ...(caps.completion ? ['completions' as const] : []),
    ...(caps.embeddings ? ['embeddings' as const] : []),
    ...(caps.fine_tune ? ['fine_tune' as const] : []),
    ...(caps.inference ? ['inference' as const] : []),
    ...(caps.text_to_speech ? ['audio_speech' as const] : []),
    ...(caps.speech_to_text ? ['audio_transcription' as const] : []),
    ...(caps.image_input ? ['image_input' as const] : []),
    ...(caps.image_generation ? ['image_generation' as const] : []),
  ])
}

function normalizeSupportedApis(
  supportedApis?: string[],
  capabilities?: ModelCapabilities
): SupportedApi[] {
  const apis = uniqueApis(
    (supportedApis ?? [])
      .map(normalizeSupportedApi)
      .filter((api): api is SupportedApi => api != null)
  )
  return apis.length > 0 ? apis : supportedApisFromCapabilities(capabilities)
}

function aggregateModels(models: RegisteredModelView[]): AggregatedModel[] {
  return models.map((model) => {
    const endpointIds = model.endpoint_ids ?? []
    const requiredMemoryBytes = typeof model.required_memory_gb === 'number'
      ? Math.round(model.required_memory_gb * 1024 * 1024 * 1024)
      : undefined
    return {
      id: model.name,
      bestStatus: model.lifecycle_status,
      ready: model.ready,
      supportedApis: normalizeSupportedApis(model.supported_apis, model.capabilities),
      maxTokens: undefined,
      source: model.source,
      tags: model.tags ?? [],
      description: model.description,
      repo: model.repo,
      filename: model.filename,
      requiredMemoryBytes,
      requiredMemoryLabel: requiredMemoryBytes ? formatBytes(requiredMemoryBytes) : '-',
      maxTokensLabel: '-',
      lifecycleLabel: LIFECYCLE_LABELS[model.lifecycle_status],
      chatTemplate: model.chat_template,
      endpointIds,
      endpointCount: endpointIds.length,
      endpointCountLabel: endpointIds.length.toLocaleString(),
      endpointSourcesLabel: `Endpoints (${endpointIds.length} ${endpointIds.length === 1 ? 'source' : 'sources'})`,
      canonicalName: model.canonical_name,
      aliases: model.aliases ?? [],
      isCanonical: model.is_canonical ?? false,
    }
  })
}

const LIFECYCLE_LABELS: Record<LifecycleStatus, string> = {
  registered: 'Registered', caching: 'Caching', pending: 'Pending', error: 'Error',
}

const LIFECYCLE_PRIORITY: Record<LifecycleStatus, number> = {
  registered: 4,
  caching: 3,
  pending: 2,
  error: 1,
}


export interface ModelTraffic {
  total: number
  totalLabel: string
  successfulLabel: string
  failedLabel: string
}

function formatTraffic(stat?: ModelStatEntry): ModelTraffic {
  const total = stat?.total_requests ?? 0
  return {
    total,
    totalLabel: total.toLocaleString(),
    successfulLabel: (stat?.successful_requests ?? 0).toLocaleString(),
    failedLabel: (stat?.failed_requests ?? 0).toLocaleString(),
  }
}

export interface ModelEndpoint extends DashboardEndpoint {
  canDeleteModel: boolean
}

interface DeleteDialogState {
  open: boolean
  modelId: string
  endpointId: string
  endpointName: string
  endpointType: string
}

export interface ModelsTableViewModel {
  search: string
  setSearch: (value: string) => void
  statusFilter: LifecycleStatus | 'all'
  setStatusFilter: (value: string) => void
  capabilityFilters: Record<string, boolean>
  setCapabilityFilter: (key: string, checked: boolean) => void
  columnVisibility: Record<string, boolean>
  setColumnVisible: (key: string, visible: boolean) => void
  sortField: SortField
  sortDirection: SortDirection
  handleSort: (field: SortField) => void
  expandedModels: Set<string>
  toggleExpand: (id: string) => void
  addWizardOpen: boolean
  setAddWizardOpen: Dispatch<SetStateAction<boolean>>
  deleteDialog: DeleteDialogState
  openDeleteDialog: (modelId: string, endpoint: ModelEndpoint) => void
  setDeleteDialogOpen: (open: boolean) => void
  aggregatedWithStatsFallback: AggregatedModel[]
  getModelTraffic: (modelId: string) => ModelTraffic
  modelEndpoints: Map<string, ModelEndpoint[]>
  activeApiFilters: string[]
  sorted: AggregatedModel[]
  viewerFiltered: AggregatedModel[]
  openPlayground: (modelId: string) => void
}

/** Models data, filters and commands; JSX and column renderers belong to the View. */
export function useModelsTableViewModel({ models, endpoints, viewerMode = false }: {
  models: RegisteredModelView[]
  endpoints: DashboardEndpoint[]
  viewerMode?: boolean
}): ModelsTableViewModel {
  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState<LifecycleStatus | 'all'>('all')
  const [capabilityFilters, setCapabilityFilters] = useState<Record<string, boolean>>({})
  const [sortField, setSortField] = useState<SortField>('id')
  const [sortDirection, setSortDirection] = useState<SortDirection>('asc')
  const [expandedModels, setExpandedModels] = useState<Set<string>>(new Set())
  const [addWizardOpen, setAddWizardOpen] = useState(false)
  const [deleteDialog, setDeleteDialog] = useState<DeleteDialogState>({ open: false, modelId: '', endpointId: '', endpointName: '', endpointType: '' })
  const [columnVisibility, setColumnVisibility] = useState<Record<string, boolean>>({
    id: true,
    bestStatus: true,
    endpointCount: true,
    totalRequests: true,
    supportedApis: true,
    maxTokens: false,
    source: false,
    tags: false,
    description: false,
    repo: false,
    filename: false,
    requiredMemoryBytes: false,
    chatTemplate: false,
  })

  const aggregated = useMemo(() => aggregateModels(models), [models])

  // These stats were never WS-invalidated; retain provider polling and viewer gating.
  const { data: allModelStats } = useQuery({
    queryKey: queryKeys.allModelStats(),
    queryFn: () => dashboardApi.getAllModelStats(),
    enabled: !viewerMode,
  })

  const modelStatsMap = useMemo(() => {
    const map = new Map<string, ModelStatEntry>()
    if (allModelStats) {
      for (const stat of allModelStats) {
        map.set(stat.model_id, stat)
      }
    }
    return map
  }, [allModelStats])

  const aggregatedWithStatsFallback = useMemo(() => {
    if (!allModelStats) return aggregated

    const existingIds = new Set(aggregated.map((m) => m.id))
    const statsOnlyModels: AggregatedModel[] = allModelStats
      .filter((stat) => !existingIds.has(stat.model_id))
      .map((stat) => ({
        id: stat.model_id,
        bestStatus: 'registered',
        ready: false,
        supportedApis: [],
        tags: [],
        endpointIds: [],
        endpointCount: 0,
        endpointCountLabel: '0',
        endpointSourcesLabel: 'Endpoints (0 sources)',
        maxTokensLabel: '-',
        requiredMemoryLabel: '-',
        lifecycleLabel: 'Registered',
        aliases: [],
        isCanonical: false,
      }))

    return [...aggregated, ...statsOnlyModels]
  }, [aggregated, allModelStats])

  const activeApiFilters = useMemo(
    () => Object.entries(capabilityFilters).filter(([, v]) => v).map(([k]) => k),
    [capabilityFilters]
  )

  const filtered = useMemo(() => {
    return aggregatedWithStatsFallback.filter((m) => {
      if (search && !m.id.toLowerCase().includes(search.toLowerCase())) return false
      if (statusFilter !== 'all' && m.bestStatus !== statusFilter) return false
      if (activeApiFilters.length > 0) {
        for (const api of activeApiFilters) {
          if (!m.supportedApis.includes(api as SupportedApi)) return false
        }
      }
      return true
    })
  }, [aggregatedWithStatsFallback, search, statusFilter, activeApiFilters])

  const sorted = useMemo(() => {
    return [...filtered].sort((a, b) => {
      let cmp = 0
      switch (sortField) {
        case 'id':
          cmp = a.id.localeCompare(b.id)
          break
        case 'bestStatus':
          cmp = LIFECYCLE_PRIORITY[a.bestStatus] - LIFECYCLE_PRIORITY[b.bestStatus]
          break
        case 'endpointCount':
          cmp = a.endpointCount - b.endpointCount
          break
        case 'totalRequests':
          cmp = (modelStatsMap.get(a.id)?.total_requests ?? 0) - (modelStatsMap.get(b.id)?.total_requests ?? 0)
          break
      }
      return sortDirection === 'asc' ? cmp : -cmp
    })
  }, [filtered, sortField, sortDirection, modelStatsMap])

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDirection(sortDirection === 'asc' ? 'desc' : 'asc')
    } else {
      setSortField(field)
      setSortDirection('asc')
    }
  }

  const toggleExpand = (id: string) => {
    setExpandedModels((prev) => {
      const next = new Set(prev)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
      }
      return next
    })
  }

  const viewerFiltered = useMemo(() => aggregatedWithStatsFallback.filter((model) =>
    model.id.toLowerCase().includes(search.toLowerCase())
  ), [aggregatedWithStatsFallback, search])
  const modelEndpoints = useMemo(() => {
    const result = new Map<string, ModelEndpoint[]>()
    for (const model of aggregatedWithStatsFallback) {
      const ids = new Set(model.endpointIds)
      result.set(model.id, endpoints.filter((endpoint) => ids.has(endpoint.id)).map((endpoint) => ({
        ...endpoint,
        canDeleteModel: endpoint.endpoint_type === 'xllm' || endpoint.endpoint_type === 'ollama',
      })))
    }
    return result
  }, [aggregatedWithStatsFallback, endpoints])

  return {
    search, setSearch, statusFilter,
    setStatusFilter: (value) => setStatusFilter(value as LifecycleStatus | 'all'),
    capabilityFilters,
    setCapabilityFilter: (key, checked) => setCapabilityFilters((prev) => ({ ...prev, [key]: checked })),
    columnVisibility,
    setColumnVisible: (key, visible) => setColumnVisibility((prev) => ({ ...prev, [key]: visible })),
    sortField, sortDirection, handleSort, expandedModels, toggleExpand, addWizardOpen, setAddWizardOpen,
    deleteDialog,
    openDeleteDialog: (modelId, endpoint) => setDeleteDialog({ open: true, modelId,
      endpointId: endpoint.id, endpointName: endpoint.name, endpointType: endpoint.endpoint_type }),
    setDeleteDialogOpen: (open) => setDeleteDialog((prev) => ({ ...prev, open })),
    aggregatedWithStatsFallback, getModelTraffic: (modelId) => formatTraffic(modelStatsMap.get(modelId)),
    modelEndpoints, activeApiFilters, sorted, viewerFiltered,
    openPlayground: (modelId) => { window.location.hash = 'lb-playground?model=' + encodeURIComponent(modelId) },
  }
}
