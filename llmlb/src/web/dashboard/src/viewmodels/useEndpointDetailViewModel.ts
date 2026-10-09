import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { endpointsApi, type DashboardEndpoint, type EndpointType,
  getRecommendedInferenceTimeout, getRecommendedInferenceTimeoutLabel } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { classifyEndpointLastError } from '@/lib/endpoint-errors'
import { formatRelativeTime } from '@/lib/utils'
import { toast } from '@/hooks/use-toast'
import { useInvalidateOn } from '@/hooks/useInvalidateOn'
import { invalidateDashboardSubscriptions } from '@/hooks/dashboardSubscriptions'

export interface EndpointDetailViewModel {
  name: string
  setName: (value: string) => void
  notes: string
  setNotes: (value: string) => void
  healthCheckInterval: string
  setHealthCheckInterval: (value: string) => void
  inferenceTimeout: string
  setInferenceTimeout: (value: string) => void
  downloadDialogOpen: boolean
  setDownloadDialogOpen: (open: boolean) => void
  recommendedInferenceTimeoutLabel: string
  differsFromRecommendedTimeout: boolean
  isLoadingTodayStats: boolean
  todayRequestsLabel: string
  modelCountLabel: string
  totalRequestsLabel: string
  successRateLabel: string
  requestHealth: 'normal' | 'warning' | 'error'
  statusLabel: string
  typeLabel: string
  latencyLabel: string
  registeredLabel: string
  lastSeenLabel: string
  errorLabel: string | undefined
  isSaving: boolean
  isTesting: boolean
  isSyncing: boolean
  handleSave: () => void
  testConnection: () => void
  syncModels: () => void
  openPlayground: () => void
}

function getStatusLabel(
  status: DashboardEndpoint['status']
): string {
  switch (status) {
    case 'online':
      return 'Online'
    case 'pending':
      return 'Pending'
    case 'offline':
      return 'Offline'
    case 'error':
      return 'Error'
    default:
      return status
  }
}

function getTypeLabel(
  type: EndpointType | undefined
): string {
  switch (type) {
    case 'xllm':
      return 'xLLM'
    case 'ollama':
      return 'Ollama'
    case 'vllm':
      return 'vLLM'
    case 'lm_studio':
      return 'LM Studio'
    case 'llamacpp':
      return 'llama.cpp'
    case 'openai_compatible':
      return 'OpenAI Compatible'
    case 'unknown':
      return 'Unknown'
    default:
      return '-'
  }
}

/** Modal state, presentation and commands; no extra endpoint acquisition. */
export function useEndpointDetailViewModel(
  endpoint: DashboardEndpoint, open: boolean, onOpenChange: (open: boolean) => void,
): EndpointDetailViewModel {
  const queryClient = useQueryClient()
  // The parent owns endpoint acquisition. Keep the modal's snapshot and keyed form lifetime.
  useInvalidateOn(['endpoints'], queryKeys.dashboardEndpoints())
  const errorDisplay = classifyEndpointLastError(endpoint?.last_error)
  const [name, setName] = useState(endpoint?.name || '')
  const [notes, setNotes] = useState(endpoint?.notes || '')
  const [healthCheckInterval, setHealthCheckInterval] = useState(
    endpoint?.health_check_interval_secs?.toString() || '30'
  )
  const [inferenceTimeout, setInferenceTimeout] = useState(
    endpoint?.inference_timeout_secs?.toString()
      || getRecommendedInferenceTimeout(endpoint?.endpoint_type).toString()
  )
  const [downloadDialogOpen, setDownloadDialogOpen] = useState(false)
  const recommendedInferenceTimeout = getRecommendedInferenceTimeout(endpoint?.endpoint_type)
  const recommendedInferenceTimeoutLabel = getRecommendedInferenceTimeoutLabel(
    endpoint?.endpoint_type
  )

  // SPEC-8c32349f: Fetch today's request statistics
  const { data: todayStats, isLoading: isLoadingTodayStats } = useQuery({
    queryKey: queryKeys.endpointTodayStats(endpoint?.id),
    queryFn: () => endpointsApi.getTodayStats(endpoint.id),
    enabled: !!endpoint?.id && open,
  })

  const openPlayground = () => {
    if (endpoint) {
      window.location.hash = `playground/${endpoint.id}`
      onOpenChange(false)
    }
  }

  // Update mutation
  const updateMutation = useMutation({
    mutationFn: (data: Parameters<typeof endpointsApi.update>[1]) =>
      endpointsApi.update(endpoint.id, data),
    onSuccess: () => {
      void invalidateDashboardSubscriptions(queryClient, { changed: 'endpoints', id: endpoint.id })
      toast({
        title: 'Update Complete',
        description: 'Endpoint settings updated',
      })
    },
    onError: (error) => {
      toast({
        title: 'Update Failed',
        description: String(error),
        variant: 'destructive',
      })
    },
  })

  // Test connection mutation
  const testMutation = useMutation({
    mutationFn: () => endpointsApi.test(endpoint.id),
    onSuccess: (result) => {
      void invalidateDashboardSubscriptions(queryClient, { changed: 'endpoints', id: endpoint.id })
      toast({
        title: result.success
          ? 'Connection Successful'
          : 'Connection Failed',
        description:
          result.message
          || (result.latency_ms
            ? `Latency: ${result.latency_ms}ms`
            : ''),
        variant: result.success ? 'default' : 'destructive',
      })
    },
    onError: (error) => {
      toast({
        title: 'Connection Test Failed',
        description: String(error),
        variant: 'destructive',
      })
    },
  })

  // Sync models mutation
  const syncMutation = useMutation({
    mutationFn: () => endpointsApi.sync(endpoint.id),
    onSuccess: (result) => {
      void invalidateDashboardSubscriptions(queryClient, { changed: 'endpoints', id: endpoint.id })
      toast({
        title: 'Sync Complete',
        description: `Synced ${result.synced_models} models`,
      })
    },
    onError: (error) => {
      toast({
        title: 'Sync Failed',
        description: String(error),
        variant: 'destructive',
      })
    },
  })

  const handleSave = () => {
    updateMutation.mutate({
      name: name !== endpoint?.name ? name : undefined,
      notes: notes !== endpoint?.notes ? notes : undefined,
      health_check_interval_secs:
        parseInt(healthCheckInterval) !== endpoint?.health_check_interval_secs
          ? parseInt(healthCheckInterval)
          : undefined,
      inference_timeout_secs:
        parseInt(inferenceTimeout) !== endpoint?.inference_timeout_secs
          ? parseInt(inferenceTimeout)
          : undefined,
    })
  }

  const successRate = endpoint.total_requests === 0
    ? null : (endpoint.successful_requests / endpoint.total_requests) * 100
  const errorRate = successRate === null ? 0 : 100 - successRate

  return {
    name, setName, notes, setNotes, healthCheckInterval, setHealthCheckInterval,
    inferenceTimeout, setInferenceTimeout, downloadDialogOpen, setDownloadDialogOpen,
    recommendedInferenceTimeoutLabel,
    differsFromRecommendedTimeout: inferenceTimeout !== recommendedInferenceTimeout.toString(),
    isLoadingTodayStats,
    todayRequestsLabel: todayStats && todayStats.total_requests > 0 ? todayStats.total_requests.toLocaleString() : '-',
    modelCountLabel: `Models: ${endpoint.model_count}`,
    totalRequestsLabel: endpoint.total_requests > 0 ? endpoint.total_requests.toLocaleString() : '-',
    successRateLabel: successRate === null ? '-' : `${successRate.toFixed(1)}%`,
    requestHealth: errorRate >= 20 ? 'error' : errorRate >= 5 ? 'warning' : 'normal',
    statusLabel: getStatusLabel(endpoint.status), typeLabel: getTypeLabel(endpoint.endpoint_type),
    latencyLabel: endpoint.latency_ms != null ? `${endpoint.latency_ms}ms` : '-',
    registeredLabel: formatRelativeTime(endpoint.registered_at),
    lastSeenLabel: endpoint.last_seen ? formatRelativeTime(endpoint.last_seen) : '-',
    errorLabel: errorDisplay?.label,
    isSaving: updateMutation.isPending, isTesting: testMutation.isPending, isSyncing: syncMutation.isPending,
    handleSave, testConnection: () => testMutation.mutate(), syncModels: () => syncMutation.mutate(), openPlayground,
  }
}
