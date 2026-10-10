import { useEffect, useRef, useState, type RefObject } from 'react'
import { useQuery } from '@tanstack/react-query'
import { dashboardApi } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { toast } from '@/hooks/use-toast'

export type LogLevel = 'all' | 'error' | 'warn' | 'info' | 'debug'

export interface LogViewerRow {
  timeLabel: string
  level: string
  targetLabel: string | undefined
  message: string
}

export interface LogViewerViewModel {
  levelFilter: LogLevel
  autoScroll: boolean
  scrollRef: RefObject<HTMLDivElement | null>
  filteredLogs: LogViewerRow[] | undefined
  logCount: number | undefined
  isRefetching: boolean
  setLevelFilter: (value: string) => void
  setAutoScroll: (value: boolean) => void
  handleRefresh: () => void
  handleClear: () => void
  handleDownload: () => void
}

export function useLogViewerViewModel(): LogViewerViewModel {
  const [levelFilter, setLevelFilter] = useState<LogLevel>('all')
  const [autoScroll, setAutoScroll] = useState(true)
  const scrollRef = useRef<HTMLDivElement>(null)
  const { data: routerLogs, refetch, isRefetching } = useQuery({
    queryKey: queryKeys.routerLogs(),
    queryFn: () => dashboardApi.getRouterLogs({ limit: 200 }),
    refetchInterval: 5000,
  })
  const filteredEntries = routerLogs?.entries.filter((log) =>
    levelFilter === 'all' || log.level.toLowerCase() === levelFilter)
  const filteredLogs = filteredEntries?.map((log) => ({
    timeLabel: new Date(log.timestamp).toLocaleTimeString(),
    level: log.level,
    targetLabel: log.target ? `[${log.target}]` : undefined,
    message: log.message || '',
  }))

  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      const viewport = scrollRef.current.querySelector('[data-radix-scroll-area-viewport]')
      if (viewport) viewport.scrollTop = viewport.scrollHeight
    }
  }, [filteredLogs, autoScroll])

  function handleRefresh() {
    void refetch()
  }

  function handleClear() {
    toast({ title: 'Log clearing is handled on the server side' })
  }

  function handleDownload() {
    if (!filteredEntries || filteredEntries.length === 0) {
      toast({ title: 'No logs to download', variant: 'destructive' })
      return
    }
    const logText = filteredEntries
      .map((log) => `[${log.timestamp}] [${log.level}] ${log.target ? `[${log.target}] ` : ''}${log.message || ''}`)
      .join('\n')
    const blob = new Blob([logText], { type: 'text/plain' })
    const url = URL.createObjectURL(blob)
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = `logs-llmlb-${new Date().toISOString().slice(0, 10)}.txt`
    anchor.click()
    URL.revokeObjectURL(url)
    toast({ title: 'Logs downloaded' })
  }

  return {
    levelFilter, autoScroll, scrollRef, filteredLogs, logCount: filteredLogs?.length, isRefetching,
    setLevelFilter: (value) => setLevelFilter(value as LogLevel), setAutoScroll,
    handleRefresh, handleClear, handleDownload,
  }
}
