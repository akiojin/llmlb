import { useState } from 'react'
import { useMutation, useQuery } from '@tanstack/react-query'
import { clientsApi } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { useInvalidateOn } from '@/hooks/useInvalidateOn'

export interface AlertThresholdSettingsViewModel {
  editing: boolean
  inputValue: string
  thresholdLabel: string
  isPending: boolean
  startEditing: () => void
  setInputValue: (value: string) => void
  save: () => void
  cancelEditing: () => void
  handleKeyDown: (key: string) => void
}

export function useAlertThresholdSettingsViewModel(): AlertThresholdSettingsViewModel {
  const [editing, setEditing] = useState(false)
  const [inputValue, setInputValue] = useState('')
  const refreshThreshold = useInvalidateOn([], queryKeys.alertThreshold())
  const refreshRanking = useInvalidateOn([], queryKeys.clientRanking())
  const { data } = useQuery({
    queryKey: queryKeys.alertThreshold(),
    queryFn: () => clientsApi.getAlertThreshold(),
  })
  const mutation = useMutation({
    mutationFn: (value: string) => clientsApi.updateAlertThreshold(value),
    onSuccess: () => {
      void refreshThreshold()
      void refreshRanking()
      setEditing(false)
    },
  })
  const threshold = data?.value ?? '100'

  function startEditing() {
    setInputValue(threshold)
    setEditing(true)
  }

  function save() {
    const num = parseInt(inputValue, 10)
    if (!isNaN(num) && num > 0) mutation.mutate(String(num))
  }

  function cancelEditing() {
    setEditing(false)
  }

  function handleKeyDown(key: string) {
    if (key === 'Enter') save()
    if (key === 'Escape') cancelEditing()
  }

  return {
    editing, inputValue, thresholdLabel: `${threshold} requests`, isPending: mutation.isPending,
    startEditing, setInputValue, save, cancelEditing, handleKeyDown,
  }
}
