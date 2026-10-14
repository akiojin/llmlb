import { useState, useMemo, useEffect } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { systemApi, type SystemInfo, type UpdateState, type ScheduleInfo } from '@/lib/api/system'
import { queryKeys } from '@/lib/queryKeys'
import { toast } from '@/hooks/use-toast'

const SYSTEM_INFO_QUERY_KEY = queryKeys.systemInfo()
const CHECK_COOLDOWN_MS = 30_000

type ScheduleMode = 'immediate' | 'idle' | 'scheduled'

export interface SystemUpdateViewModel {
  isApplyingUpdate: boolean
  isApplyingForceUpdate: boolean
  isForceUpdateDialogOpen: boolean
  setIsForceUpdateDialogOpen: (open: boolean) => void
  isCheckingUpdate: boolean
  isCooldown: boolean
  isRollbackDialogOpen: boolean
  setIsRollbackDialogOpen: (open: boolean) => void
  isRollingBack: boolean
  isSettingsOpen: boolean
  setIsSettingsOpen: (open: boolean) => void
  scheduleMode: ScheduleMode
  setScheduleMode: (mode: ScheduleMode) => void
  scheduledAt: string
  setScheduledAt: (value: string) => void
  isScheduling: boolean
  drainCountdown: string | null
  applyTimeoutCountdown: string | null
  updateState: UpdateState['state'] | undefined
  updateLatest: string | null
  restartLabel: string
  applying: boolean
  showRestartButton: boolean
  showForceButton: boolean
  canApply: boolean
  canForceApply: boolean
  canCheck: boolean
  forceUpdateTitle: string | undefined
  rollbackAvailable: boolean
  title: string
  description: string
  link: string | null
  payloadHint: string | null
  downloadProgress: { downloaded_bytes: number; total_bytes: number } | null
  downloadProgressPercent: number | null
  scheduleInfo: ScheduleInfo | null | undefined
  scheduleDescription: string | null
  onCheck: () => Promise<void>
  onApply: () => Promise<void>
  onForceApply: () => Promise<void>
  onRollback: () => Promise<void>
  onSchedule: () => Promise<void>
  onCancelSchedule: () => Promise<void>
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)}KB`
  return `${(bytes / (1024 * 1024)).toFixed(0)}MB`
}

function formatCountdown(targetIso: string): string | null {
  const target = new Date(targetIso).getTime()
  const now = Date.now()
  const diffSec = Math.max(0, Math.floor((target - now) / 1000))
  if (diffSec <= 0) return '0:00'
  const min = Math.floor(diffSec / 60)
  const sec = diffSec % 60
  return `${min}:${sec.toString().padStart(2, '0')}`
}

interface Countdown {
  timeoutAt: string
  text: string | null
}

function countdownFor(state: Countdown | null, timeoutAt: string | null): string | null {
  return timeoutAt && state?.timeoutAt === timeoutAt ? state.text : null
}

/** Update presentation, commands and timers; system changes are pulled by the data ViewModel's subscription. */
export function useSystemUpdateViewModel(systemInfo: SystemInfo | undefined, isAdmin: boolean): SystemUpdateViewModel {
  const queryClient = useQueryClient()
  const [isApplyingUpdate, setIsApplyingUpdate] = useState(false)
  const [isApplyingForceUpdate, setIsApplyingForceUpdate] = useState(false)
  const [isForceUpdateDialogOpen, setIsForceUpdateDialogOpen] = useState(false)
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false)
  const [isCooldown, setIsCooldown] = useState(false)
  const [isRollbackDialogOpen, setIsRollbackDialogOpen] = useState(false)
  const [isRollingBack, setIsRollingBack] = useState(false)
  const [isSettingsOpen, setIsSettingsOpen] = useState(false)
  const [scheduleMode, setScheduleMode] = useState<ScheduleMode>('immediate')
  const [scheduledAt, setScheduledAt] = useState('')
  const [isScheduling, setIsScheduling] = useState(false)
  // カウントダウン表示は対象の timeout_at と組で保持し、別の timeout_at に対する古い表示を出さない
  const [drainCountdownState, setDrainCountdownState] = useState<Countdown | null>(null)
  const [applyTimeoutCountdownState, setApplyTimeoutCountdownState] = useState<Countdown | null>(null)
  // Drain timeout countdown timer
  const drainTimeoutAt =
    systemInfo?.update?.state === 'draining' ? systemInfo.update.timeout_at || null : null
  useEffect(() => {
    if (!drainTimeoutAt) return
    const tick = () =>
      setDrainCountdownState({ timeoutAt: drainTimeoutAt, text: formatCountdown(drainTimeoutAt) })
    tick()
    const timer = setInterval(tick, 1000)
    return () => clearInterval(timer)
  }, [drainTimeoutAt])
  const drainCountdown = countdownFor(drainCountdownState, drainTimeoutAt)

  const applyTimeoutAt =
    systemInfo?.update?.state === 'applying' ? systemInfo.update.timeout_at || null : null
  useEffect(() => {
    if (!applyTimeoutAt) return
    const tick = () =>
      setApplyTimeoutCountdownState({ timeoutAt: applyTimeoutAt, text: formatCountdown(applyTimeoutAt) })
    tick()
    const timer = setInterval(tick, 1000)
    return () => clearInterval(timer)
  }, [applyTimeoutAt])
  const applyTimeoutCountdown = countdownFor(applyTimeoutCountdownState, applyTimeoutAt)

  // Update check cooldown: re-enable the check button CHECK_COOLDOWN_MS after the last check
  useEffect(() => {
    if (!isCooldown) return
    const timer = setTimeout(() => setIsCooldown(false), CHECK_COOLDOWN_MS)
    return () => clearTimeout(timer)
  }, [isCooldown])

  const updateBanner = useMemo(() => {
    const update = systemInfo?.update as UpdateState | undefined
    const updateState = update?.state
    const hasAvailableUpdate = updateState === 'available'
    const isPayloadReady =
      hasAvailableUpdate && update?.payload?.payload === 'ready'
    const failedHasUpdateCandidate = updateState === 'failed' && Boolean(update?.latest)
    const canApply = isAdmin && (updateState === 'available' || failedHasUpdateCandidate)
    const applying = updateState === 'draining' || updateState === 'applying'
    const showRestartButton = updateState === 'available' || failedHasUpdateCandidate || applying
    const showForceButton = hasAvailableUpdate
    const canForceApply = isAdmin && isPayloadReady && !applying
    const canCheck = isAdmin && !applying && !isCooldown
    const forceUpdateTitle = !isAdmin
      ? 'Admin role is required'
      : applying
        ? 'Update is in progress'
        : !hasAvailableUpdate
          ? 'No update is available'
        : isPayloadReady
          ? undefined
          : 'Update payload is still preparing'

    const rollbackAvailable = systemInfo?.rollback_available === true
    const scheduleInfo = systemInfo?.schedule as ScheduleInfo | null | undefined

    let title = 'Update'
    let description = 'Update status unavailable'
    let link: string | null = null
    let payloadHint: string | null = null
    let downloadProgress: { downloaded_bytes: number; total_bytes: number } | null = null

    if (updateState === 'available' && update) {
      title = `Update available: v${update.latest}`
      description = `Current: v${update.current}`
      link = update.release_url
      if (update.payload?.payload === 'downloading') {
        const dl = update.payload
        if (dl.downloaded_bytes != null && dl.total_bytes != null && dl.total_bytes > 0) {
          downloadProgress = {
            downloaded_bytes: dl.downloaded_bytes,
            total_bytes: dl.total_bytes,
          }
          const pct = Math.round((dl.downloaded_bytes / dl.total_bytes) * 100)
          payloadHint = `Downloading: ${formatBytes(dl.downloaded_bytes)} / ${formatBytes(dl.total_bytes)} (${pct}%)`
        } else {
          payloadHint = 'Downloading...'
        }
      } else if (update.payload?.payload === 'ready') {
        payloadHint = 'Ready'
      } else if (update.payload?.payload === 'error') {
        payloadHint = 'Download failed'
      } else {
        payloadHint = 'Preparing...'
      }
    } else if (updateState === 'up_to_date' && update) {
      title = 'Up to date'
      const checkedAt = update.checked_at ?? null
      if (checkedAt) {
        const asDate = new Date(checkedAt)
        description = `Last checked: ${Number.isNaN(asDate.valueOf()) ? checkedAt : asDate.toLocaleString()}`
      } else {
        description = 'Last checked: unknown'
      }
    } else if (updateState === 'draining' && update) {
      title = `Updating to v${update.latest}`
      description = `Waiting for in-flight requests: ${update.in_flight}`
    } else if (updateState === 'applying' && update) {
      title = `Applying update: v${update.latest}`
      description = update.phase_message ?? 'Restarting...'
    } else if (updateState === 'failed' && update) {
      title = 'Update failed'
      description = update.message
      link = update.release_url || null
    }

    const onCheck = async () => {
      setIsCheckingUpdate(true)
      setIsCooldown(true)
      const previousUpdate = queryClient.getQueryData<SystemInfo>(SYSTEM_INFO_QUERY_KEY)?.update
      try {
        const { update } = await systemApi.checkUpdate()
        const currentSystemInfo = queryClient.getQueryData<SystemInfo>(SYSTEM_INFO_QUERY_KEY)
        if (currentSystemInfo) {
          // A system notification may already have fetched newer download progress.
          if (currentSystemInfo.update === previousUpdate) {
            queryClient.setQueryData<SystemInfo>(SYSTEM_INFO_QUERY_KEY, {
              ...currentSystemInfo,
              update,
            })
          }
        } else {
          const freshSystemInfo = await systemApi.getSystem()
          // This GET is newer than the check snapshot; retain any intervening pull too.
          queryClient.setQueryData<SystemInfo>(SYSTEM_INFO_QUERY_KEY,
            (current) => current ?? freshSystemInfo,
          )
        }
        toast({
          title: 'Checked for updates',
        })
      } catch (e) {
        toast({
          title: 'Update check failed',
          description: e instanceof Error ? e.message : String(e),
          variant: 'destructive',
        })
      } finally {
        setIsCheckingUpdate(false)
      }
    }

    const onApply = async () => {
      setIsApplyingUpdate(true)
      try {
        const result = await systemApi.applyUpdate()
        if (result.queued) {
          toast({
            title: 'Update queued',
            description: 'llmlb will restart after in-flight requests complete.',
          })
        } else {
          toast({
            title: 'Applying update',
            description: 'llmlb is restarting now.',
          })
        }
      } catch (e) {
        toast({
          title: 'Failed to apply update',
          description: e instanceof Error ? e.message : String(e),
          variant: 'destructive',
        })
      } finally {
        setIsApplyingUpdate(false)
      }
    }

    const onForceApply = async () => {
      setIsApplyingForceUpdate(true)
      try {
        const result = await systemApi.applyForceUpdate()
        toast({
          title: 'Force update started',
          description:
            result.dropped_in_flight > 0
              ? `${result.dropped_in_flight} in-flight request(s) were terminated.`
              : 'No in-flight requests were active.',
        })
        setIsForceUpdateDialogOpen(false)
      } catch (e) {
        toast({
          title: 'Failed to force update',
          description: e instanceof Error ? e.message : String(e),
          variant: 'destructive',
        })
      } finally {
        setIsApplyingForceUpdate(false)
      }
    }

    const onRollback = async () => {
      setIsRollingBack(true)
      try {
        await systemApi.rollback()
        toast({
          title: 'Rolling back',
          description: 'Restoring previous version and restarting...',
        })
        setIsRollbackDialogOpen(false)
      } catch (e) {
        toast({
          title: 'Rollback failed',
          description: e instanceof Error ? e.message : String(e),
          variant: 'destructive',
        })
      } finally {
        setIsRollingBack(false)
      }
    }

    const onSchedule = async () => {
      setIsScheduling(true)
      try {
        if (scheduleMode === 'immediate') {
          await onApply()
        } else {
          await systemApi.createSchedule({
            mode: scheduleMode,
            scheduled_at: scheduleMode === 'scheduled' ? scheduledAt : undefined,
          })
          toast({ title: 'Schedule created' })
        }
        setIsSettingsOpen(false)
      } catch (e) {
        toast({
          title: 'Failed to create schedule',
          description: e instanceof Error ? e.message : String(e),
          variant: 'destructive',
        })
      } finally {
        setIsScheduling(false)
      }
    }

    const onCancelSchedule = async () => {
      try {
        await systemApi.cancelSchedule()
        toast({ title: 'Schedule cancelled' })
      } catch (e) {
        toast({
          title: 'Failed to cancel schedule',
          description: e instanceof Error ? e.message : String(e),
          variant: 'destructive',
        })
      }
    }

    return {
      updateState, applying, showRestartButton, showForceButton, canApply, canForceApply,
      canCheck, forceUpdateTitle, rollbackAvailable, title, description, link, payloadHint,
      downloadProgress, onCheck, onApply, onForceApply, onRollback, onSchedule, onCancelSchedule,
      scheduleInfo,
      updateLatest: update && 'latest' in update ? update.latest ?? null : null,
      restartLabel: update?.state === 'draining'
        ? `Waiting to update... (${update.in_flight})`
        : update?.state === 'applying' ? 'Applying update...' : 'Restart to update',
      downloadProgressPercent: downloadProgress
        ? Math.round(downloadProgress.downloaded_bytes / downloadProgress.total_bytes * 100)
        : null,
      scheduleDescription: scheduleInfo
        ? `Scheduled by ${scheduleInfo.scheduled_by} (${scheduleInfo.mode})${scheduleInfo.scheduled_at ? ` at ${new Date(scheduleInfo.scheduled_at).toLocaleString()}` : ''}`
        : null,
    }
  }, [
    systemInfo?.update,
    systemInfo?.rollback_available,
    systemInfo?.schedule,
    isCooldown,
    scheduleMode,
    scheduledAt,
    queryClient,
    isAdmin,
  ])

  return {
    ...updateBanner,
    isApplyingUpdate, isApplyingForceUpdate, isForceUpdateDialogOpen, setIsForceUpdateDialogOpen,
    isCheckingUpdate, isCooldown, isRollbackDialogOpen, setIsRollbackDialogOpen, isRollingBack,
    isSettingsOpen, setIsSettingsOpen, scheduleMode, setScheduleMode, scheduledAt, setScheduledAt,
    isScheduling, drainCountdown, applyTimeoutCountdown,
  }
}
