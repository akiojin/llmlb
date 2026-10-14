import { useEffect, useRef, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { endpointsApi, type DashboardEndpoint, type DownloadTask } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { toast } from '@/hooks/use-toast'
import { useInvalidateOn } from '@/hooks/useInvalidateOn'

interface ModelDownloadDialogOptions {
  endpoint: DashboardEndpoint | null
  open: boolean
  onOpenChange: (open: boolean) => void
}
export interface ModelDownloadDialogViewModel {
  supportsDownload: boolean
  modelName: string
  status: 'idle' | 'downloading' | 'completed' | 'error'
  progress: number
  progressMessage: string
  errorMessage: string
  isDownloadPending: boolean
  canDownload: boolean
  setModelName: (value: string) => void
  handleDownload: () => void
  handleClose: () => void
  handleOpenChange: (open: boolean) => void
}

/** Owns one keyed dialog session; catalog/batch downloads and model lists stay in their existing VMs. */
export function useModelDownloadDialogViewModel({
  endpoint, open, onOpenChange,
}: ModelDownloadDialogOptions): ModelDownloadDialogViewModel {
  const queryClient = useQueryClient()
  const [modelName, setModelName] = useState('')
  const [download, setDownload] = useState<{ taskId: string; model: string } | null>(null)
  const active = useRef(false)
  const completedTask = useRef<string | null>(null)
  const supportsDownload = endpoint?.endpoint_type === 'xllm'
  const refreshEndpointModels = useInvalidateOn([], queryKeys.endpointModels(endpoint?.id ?? ''), { id: endpoint?.id })
  const refreshEndpoints = useInvalidateOn(['endpoints'], queryKeys.dashboardEndpoints())

  useEffect(() => {
    active.current = open && supportsDownload
    return () => { active.current = false }
  }, [open, supportsDownload])

  const downloadMutation = useMutation({
    mutationFn: (data: { model: string }) => endpointsApi.downloadModel(endpoint!.id, data),
    onSuccess: (data) => {
      if (active.current) setDownload({ taskId: data.task_id, model: modelName })
    },
    onError: (error) => {
      if (!active.current) return
      toast({ title: 'Download Failed', description: error instanceof Error ? error.message : 'Unknown error', variant: 'destructive' })
    },
  })

  const progressQuery = useQuery({
    queryKey: queryKeys.endpointDownloadProgress(endpoint?.id, download?.taskId),
    queryFn: async ({ signal, queryKey }) => {
      const result = await endpointsApi.getDownloadProgress(endpoint!.id, signal)
      const task = findTask(result.tasks, download)
      const previous = queryClient.getQueryData<{ progress: number }>(queryKey)
      // An absent task changes the message, but the previous progress bar stays visible.
      const progress = task && (task.status === 'pending' || task.status === 'downloading')
        ? task.progress || 0 : previous?.progress ?? 0
      return { ...result, progress }
    },
    enabled: (query) => open && supportsDownload && !!download && !isTerminal(findTask(query.state.data?.tasks, download)),
    // The old polling loop ran independently of focus and retried only on its next tick.
    refetchInterval: (query) => isTerminal(findTask(query.state.data?.tasks, download)) ? false : 2000,
    refetchIntervalInBackground: true,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
    retry: false,
    gcTime: 0,
  })
  useEffect(() => {
    if (!open) void queryClient.cancelQueries({
      queryKey: queryKeys.endpointDownloadProgress(endpoint?.id, download?.taskId), exact: true,
    })
  }, [open, queryClient, endpoint?.id, download?.taskId])

  const currentTask = findTask(progressQuery.data?.tasks, download)
  const terminal = isTerminal(currentTask)
  // A terminal task also disables the observer, including explicit cache invalidations.
  const status = downloadMutation.isError ? 'error'
    : currentTask?.status === 'completed' ? 'completed'
      : terminal ? 'error' : download ? 'downloading' : 'idle'

  useEffect(() => {
    if (!open || !active.current || currentTask?.status !== 'completed' || !download || completedTask.current === download.taskId) return
    completedTask.current = download.taskId
    void refreshEndpointModels()
    void refreshEndpoints()
    toast({ title: 'Download Completed', description: `Model ${download.model} has been downloaded successfully` })
  }, [open, currentTask?.status, download, refreshEndpointModels, refreshEndpoints])

  const handleDownload = () => {
    if (!open || !supportsDownload || downloadMutation.isPending || status === 'downloading') return
    if (!modelName.trim()) {
      toast({ title: 'Model name required', description: 'Please enter a model name', variant: 'destructive' })
      return
    }
    setDownload(null)
    completedTask.current = null
    downloadMutation.mutate({ model: modelName.trim() })
  }
  const handleClose = () => {
    if (status === 'downloading') {
      toast({ title: 'Download in progress', description: 'Please wait for the download to complete' })
      return
    }
    onOpenChange(false)
  }
  return {
    supportsDownload, modelName, status,
    progress: status === 'completed' ? 100 : progressQuery.data?.progress ?? 0,
    progressMessage: status === 'completed' ? 'Download completed'
      : status !== 'downloading' ? '' : currentTask ? buildProgressMessage(currentTask)
        : progressQuery.data ? 'Waiting for download to start...' : 'Starting download...',
    errorMessage: downloadMutation.isError
      ? downloadMutation.error instanceof Error ? downloadMutation.error.message : 'Download failed'
      : status === 'error' ? currentTask?.error || 'Download failed' : '',
    isDownloadPending: downloadMutation.isPending,
    canDownload: !!modelName.trim() && !downloadMutation.isPending,
    setModelName, handleDownload, handleClose,
    handleOpenChange: (nextOpen) => { if (nextOpen) onOpenChange(true); else handleClose() },
  }
}

function findTask(tasks: DownloadTask[] | undefined, download: { taskId: string; model: string } | null) {
  return download ? tasks?.find((task) => task.task_id === download.taskId) || tasks?.find((task) => task.model === download.model) : undefined
}
function isTerminal(task: DownloadTask | undefined) {
  return task?.status === 'completed' || task?.status === 'failed' || task?.status === 'cancelled'
}
function buildProgressMessage(task: DownloadTask): string {
  const parts: string[] = []
  if (task.speed_mbps != null) parts.push(`${task.speed_mbps.toFixed(1)} Mbps`)
  if (task.eta_seconds != null) parts.push(`ETA ${formatEta(task.eta_seconds)}`)
  return parts.join(' / ') || 'Downloading...'
}
function formatEta(seconds: number): string {
  if (!Number.isFinite(seconds)) return '-'
  const s = Math.max(0, Math.floor(seconds))
  const m = Math.floor(s / 60)
  const r = s % 60
  return m <= 0 ? `${r}s` : `${m}m ${r}s`
}
