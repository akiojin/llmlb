import { queryKeys } from '@/lib/queryKeys'
import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  notificationsApi,
  type NotificationSettings,
  type NotificationSettingsResponse,
} from '@/lib/api'
import { toast } from '@/hooks/use-toast'

const QUERY_KEY = queryKeys.notificationSettings()

// The port is edited as text so that an empty or partial entry can be shown as invalid.
type FormState = Omit<NotificationSettings, 'smtp_port'> & { smtp_port: string }

const FORM_FIELDS = [
  'enabled',
  'smtp_host',
  'smtp_port',
  'smtp_from',
  'daily_digest_time',
  'language',
] as const satisfies readonly (keyof FormState)[]

function toForm(settings: NotificationSettings): FormState {
  return { ...settings, smtp_port: String(settings.smtp_port) }
}

function isValidPort(port: string) {
  return /^\d+$/.test(port) && Number(port) >= 1 && Number(port) <= 65535
}

function isValidTime(time: string) {
  return /^([01]\d|2[0-3]):[0-5]\d$/.test(time)
}

interface UseNotificationSettingsViewModelOptions {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function useNotificationSettingsViewModel({
  open,
  onOpenChange,
}: UseNotificationSettingsViewModelOptions) {
  const queryClient = useQueryClient()
  // Unsaved edits, by field. A field that was not edited keeps following the
  // stored settings, so saving never writes back a value this dialog only read.
  const [draft, setDraft] = useState<Partial<FormState>>({})

  const { data, isLoading, error, refetch } = useQuery({
    queryKey: QUERY_KEY,
    queryFn: () => notificationsApi.get(),
    enabled: open,
  })

  const saveMutation = useMutation({
    mutationFn: (settings: NotificationSettings) => notificationsApi.update(settings),
    onSuccess: async (saved: NotificationSettingsResponse) => {
      // A refresh that started before the save would bring the old settings back.
      await queryClient.cancelQueries({ queryKey: QUERY_KEY })
      queryClient.setQueryData(QUERY_KEY, saved)
      setDraft({})
      // The status panel can be scrolled out of view next to Save, so repeat
      // the reason when the saved settings still cannot send.
      toast({
        title: 'Notification settings saved',
        description:
          saved.status.state === 'unavailable' && saved.status.reason
            ? `Notifications cannot be sent yet: ${saved.status.reason}`
            : undefined,
      })
    },
    onError: (err) => {
      toast({
        title: 'Failed to save notification settings',
        description: err instanceof Error ? err.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  const stored = data ? toForm(data.settings) : null
  const form = stored ? { ...stored, ...draft } : null
  const isDirty =
    form !== null && stored !== null && FORM_FIELDS.some((field) => form[field] !== stored[field])
  const portValid = form !== null && isValidPort(form.smtp_port)
  const timeValid = form !== null && isValidTime(form.daily_digest_time)
  const isSaving = saveMutation.isPending

  const edit = (changes: Partial<FormState>) => {
    setDraft((current) => ({ ...current, ...changes }))
  }

  const handleOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) setDraft({})
    onOpenChange(nextOpen)
  }

  const handleSave = () => {
    if (!form || !portValid || !timeValid) return
    saveMutation.mutate({
      ...form,
      smtp_host: form.smtp_host.trim(),
      smtp_from: form.smtp_from.trim(),
      smtp_port: Number(form.smtp_port),
    })
  }

  return {
    data,
    isLoading,
    error,
    refetch,
    form,
    isDirty,
    portValid,
    timeValid,
    isSaving,
    edit,
    handleOpenChange,
    handleSave,
  }
}
