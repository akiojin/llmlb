import { queryKeys } from '@/lib/queryKeys'
import { useState, useEffect, useCallback, useRef } from 'react'
import { useQuery, useMutation } from '@tanstack/react-query'
import { type CatalogSearchResult, type CatalogSearchResponse, type CatalogModelDetail,
  type RecommendedEndpoint, catalogApi, endpointsApi } from '@/lib/api'
import { toast } from '@/hooks/use-toast'
import { useInvalidateOn } from '@/hooks/useInvalidateOn'

type WizardStep = 'search' | 'detail' | 'endpoints' | 'download'
export type ModelDownloadStatus = 'pending' | 'downloading' | 'completed' | 'failed'

export interface ModelAddWizardViewModel {
  step: WizardStep
  stepTitle: string
  searchQuery: string
  debouncedQuery: string
  selectedModel: CatalogSearchResult | null
  selectedEndpointIds: Set<string>
  downloadStatuses: Record<string, ModelDownloadStatus>
  searchResults: CatalogSearchResponse | undefined
  modelDetail: CatalogModelDetail | undefined
  isSearching: boolean
  isLoadingDetail: boolean
  isLoadingEndpoints: boolean
  hasRecommendations: boolean
  downloadableEndpoints: RecommendedEndpoint[]
  downloadRows: Array<RecommendedEndpoint & { status: ModelDownloadStatus }>
  compatibleEngineEntries: Array<[string, string]>
  allDownloadsFinished: boolean
  downloadButtonLabel: string
  handleSearchChange: (value: string) => void
  handleSelectModel: (model: CatalogSearchResult) => void
  handleProceedToEndpoints: () => void
  toggleEndpoint: (id: string) => void
  handleStartDownload: () => Promise<void>
  handleBack: () => void
}

/** Catalog conditions, dialog-session state and sequential download commands. */
export function useModelAddWizardViewModel(): ModelAddWizardViewModel {
  const refreshEndpoints = useInvalidateOn(['endpoints'], queryKeys.dashboardEndpoints())
  const refreshModels = useInvalidateOn([], queryKeys.models())
  const [step, setStep] = useState<WizardStep>('search')
  const [searchQuery, setSearchQuery] = useState('')
  const [debouncedQuery, setDebouncedQuery] = useState('')
  const [selectedModel, setSelectedModel] = useState<CatalogSearchResult | null>(null)
  const [selectedEndpointIds, setSelectedEndpointIds] = useState<Set<string>>(new Set())
  const [downloadStatuses, setDownloadStatuses] = useState<
    Record<string, ModelDownloadStatus>
  >({})
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Cancel pending search debounce when this dialog session is discarded.
  useEffect(() => {
    return () => {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current)
      }
    }
  }, [])

  // Debounce search input
  const handleSearchChange = useCallback((value: string) => {
    setSearchQuery(value)
    if (debounceRef.current) clearTimeout(debounceRef.current)
    debounceRef.current = setTimeout(() => {
      setDebouncedQuery(value)
    }, 300)
  }, [])

  // Search query
  const { data: searchResults, isLoading: isSearching } = useQuery({
    queryKey: queryKeys.catalogSearch(debouncedQuery),
    queryFn: () => catalogApi.search(debouncedQuery, 20),
    enabled: debouncedQuery.length >= 2,
  })

  // Model detail query
  const { data: modelDetail, isLoading: isLoadingDetail } = useQuery({
    queryKey: queryKeys.catalogModel(selectedModel?.repo_id),
    queryFn: () => catalogApi.getModel(selectedModel!.repo_id),
    enabled: !!selectedModel && (step === 'detail' || step === 'endpoints'),
  })

  // Endpoint recommendations
  const { data: recommendations, isLoading: isLoadingEndpoints } = useQuery({
    queryKey: queryKeys.catalogRecommend(selectedModel?.repo_id),
    queryFn: () => catalogApi.recommendEndpoints(selectedModel!.repo_id),
    enabled: !!selectedModel && step === 'endpoints',
  })

  // Download mutation
  const downloadMutation = useMutation({
    mutationFn: async ({
      endpointId,
      model,
    }: {
      endpointId: string
      model: string
    }) => {
      return endpointsApi.downloadModel(endpointId, {
        model,
        hf_repo: selectedModel?.repo_id,
      })
    },
  })

  const handleSelectModel = (model: CatalogSearchResult) => {
    setSelectedModel(model)
    setStep('detail')
  }

  const handleProceedToEndpoints = () => {
    setSelectedEndpointIds(new Set())
    setStep('endpoints')
  }

  const toggleEndpoint = (id: string) => {
    setSelectedEndpointIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
      }
      return next
    })
  }

  const handleStartDownload = async () => {
    if (!selectedModel || selectedEndpointIds.size === 0) return

    setStep('download')
    const initialStatuses: Record<string, ModelDownloadStatus> = {}
    for (const id of selectedEndpointIds) {
      initialStatuses[id] = 'pending'
    }
    setDownloadStatuses(initialStatuses)

    for (const endpointId of selectedEndpointIds) {
      setDownloadStatuses((prev) => ({ ...prev, [endpointId]: 'downloading' }))
      try {
        await downloadMutation.mutateAsync({
          endpointId,
          model: selectedModel.repo_id,
        })
        setDownloadStatuses((prev) => ({ ...prev, [endpointId]: 'completed' }))
      } catch {
        setDownloadStatuses((prev) => ({ ...prev, [endpointId]: 'failed' }))
      }
    }

    // Preserve the batch boundary and do not extend progress for cache refetches.
    void refreshEndpoints()
    void refreshModels()
    toast({
      title: 'Download requests sent',
      description: `Initiated download of ${selectedModel.repo_id} to ${selectedEndpointIds.size} endpoint(s)`,
    })
  }

  const handleBack = () => {
    switch (step) {
      case 'detail':
        setStep('search')
        break
      case 'endpoints':
        setStep('detail')
        break
      default:
        break
    }
  }

  const allDownloadsFinished =
    step === 'download' &&
    Object.values(downloadStatuses).every((s) => s === 'completed' || s === 'failed')

  const stepTitle: Record<WizardStep, string> = {
    search: 'Search HuggingFace Models',
    detail: 'Model Details',
    endpoints: 'Select Endpoints',
    download: 'Download Progress',
  }

  const downloadableEndpoints = recommendations?.endpoints.filter((ep) => ep.can_download) ?? []
  const compatibleEngineEntries = modelDetail
    ? Object.entries(modelDetail.engine_names).filter(
        (entry): entry is [string, string] => entry[1] != null && entry[1] !== ''
      )
    : []

  const downloadRows = recommendations?.endpoints
    .filter((ep) => selectedEndpointIds.has(ep.id))
    .map((ep) => ({ ...ep, status: downloadStatuses[ep.id] ?? 'pending' as const })) ?? []

  return {
    step, stepTitle: stepTitle[step], searchQuery, debouncedQuery, selectedModel,
    selectedEndpointIds, downloadStatuses, searchResults, modelDetail, isSearching,
    isLoadingDetail, isLoadingEndpoints, hasRecommendations: !!recommendations, downloadableEndpoints, downloadRows,
    compatibleEngineEntries, allDownloadsFinished,
    downloadButtonLabel: `Download to ${selectedEndpointIds.size} ${selectedEndpointIds.size === 1 ? 'Endpoint' : 'Endpoints'}`,
    handleSearchChange, handleSelectModel, handleProceedToEndpoints, toggleEndpoint,
    handleStartDownload, handleBack,
  }
}
