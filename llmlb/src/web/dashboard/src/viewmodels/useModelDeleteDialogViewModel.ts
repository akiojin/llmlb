import { queryKeys } from '@/lib/queryKeys'
import { useMutation } from '@tanstack/react-query'
import { endpointsApi } from '@/lib/api'
import { toast } from '@/hooks/use-toast'
import { useInvalidateOn } from '@/hooks/useInvalidateOn'

interface ModelDeleteDialogOptions {
  modelId: string
  endpointId: string
  endpointName: string
  endpointType: string
  onOpenChange: (open: boolean) => void
  onDeleted?: () => void
}

export interface ModelDeleteDialogViewModel {
  supportsDelete: boolean
  isDeleting: boolean
  handleDelete: () => void
}

const DELETABLE_TYPES = new Set(['xllm', 'ollama'])

/** Model deletion, scoped cache refresh and the existing close/error contract. */
export function useModelDeleteDialogViewModel({
  modelId, endpointId, endpointName, endpointType, onOpenChange, onDeleted,
}: ModelDeleteDialogOptions): ModelDeleteDialogViewModel {
  const refreshEndpoints = useInvalidateOn(['endpoints'], queryKeys.dashboardEndpoints())
  const refreshModels = useInvalidateOn([], queryKeys.models())
  const refreshEndpointModels = useInvalidateOn([], queryKeys.endpointModels(endpointId), { id: endpointId })
  const supportsDelete = DELETABLE_TYPES.has(endpointType)

  const deleteMutation = useMutation({
    mutationFn: () => endpointsApi.deleteModel(endpointId, modelId),
    onSuccess: () => {
      toast({
        title: 'Model deleted',
        description: `${modelId} has been removed from ${endpointName}`,
      })
      // Keep the original three-query refresh without broadcasting a WS change.
      void refreshEndpointModels()
      void refreshEndpoints()
      void refreshModels()
      onOpenChange(false)
      onDeleted?.()
    },
    onError: (error) => {
      toast({
        title: 'Delete failed',
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  return { supportsDelete, isDeleting: deleteMutation.isPending, handleDelete: () => deleteMutation.mutate() }
}
