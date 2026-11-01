import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useAuth } from '@/hooks/useAuth'
import { inferenceDefaultsApi, type InferenceDefaults } from '@/lib/api/inference-defaults'
import { modelsApi } from '@/lib/api/models'
import { queryKeys } from '@/lib/queryKeys'

const SETTINGS_KEY = queryKeys.inferenceDefaults()

export function useInferenceDefaultsViewModel() {
  const { user } = useAuth()
  const isAdmin = user?.role === 'admin'
  const queryClient = useQueryClient()
  // Undefined means untouched; an empty string is an intentional clear.
  const [draft, setDraft] = useState<string>()
  const settings = useQuery({
    queryKey: SETTINGS_KEY,
    queryFn: () => inferenceDefaultsApi.get(),
    enabled: isAdmin,
    refetchInterval: false,
  })
  // Reuse the Dashboard catalog cache and fetch independently of settings.
  const catalog = useQuery({
    queryKey: queryKeys.viewerModels('canonical'),
    queryFn: () => modelsApi.getRegistered('canonical'),
    enabled: isAdmin,
  })
  const mutation = useMutation({
    mutationFn: (defaults: InferenceDefaults) => inferenceDefaultsApi.update(defaults),
    onSuccess: async (saved) => {
      // An earlier GET must not replace the committed value with stale data.
      await queryClient.cancelQueries({ queryKey: SETTINGS_KEY })
      queryClient.setQueryData(SETTINGS_KEY, saved)
      setDraft(undefined)
    },
  })

  const savedModel = settings.data?.default_text_model ?? ''
  const selectedModel = draft ?? savedModel
  const candidates = (catalog.data ?? []).filter((model) =>
    !model.capabilities || model.capabilities.chat_completion || model.capabilities.completion,
  )
  const savedModelMissing = Boolean(savedModel) && catalog.isSuccess &&
    !candidates.some((model) => model.name === savedModel)
  const options = candidates.map((model) => ({
    value: model.name,
    label: `${model.name}${model.ready ? '' : ' (offline)'}`,
  }))
  // Preserve the configured ID even when the catalog is unavailable or changed.
  if (savedModel && !options.some((option) => option.value === savedModel)) {
    options.unshift({
      value: savedModel,
      label: `${savedModel}${savedModelMissing ? ' (unavailable)' : ''}`,
    })
  }
  if (selectedModel && !options.some((option) => option.value === selectedModel)) {
    options.unshift({ value: selectedModel, label: `${selectedModel} (unavailable)` })
  }
  const isDirty = settings.data !== undefined && selectedModel !== savedModel
  const canSave = isAdmin && isDirty && !mutation.isPending

  function edit(value: string) {
    if (mutation.isPending) return
    mutation.reset()
    setDraft(value)
  }

  function save() {
    if (canSave) mutation.mutate({ default_text_model: selectedModel || null })
  }

  return {
    isAdmin,
    isLoading: settings.isPending,
    settingsError: settings.error,
    hasSettings: settings.data !== undefined,
    catalogError: catalog.error,
    isLoadingCatalog: catalog.isPending,
    selectedModel,
    savedModelMissing,
    options,
    isDirty,
    isSaving: mutation.isPending,
    saveError: mutation.error,
    isSaved: mutation.isSuccess && !settings.error,
    canSave,
    edit,
    save,
    retrySettings: () => { void settings.refetch() },
    retryCatalog: () => { void catalog.refetch() },
  }
}
