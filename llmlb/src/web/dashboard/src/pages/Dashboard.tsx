import { useState, useRef, useCallback, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import {
  dashboardApi,
  systemApi,
  type SystemInfo,
  type UpdateState,
  type DashboardOverview,
  type DashboardEndpoint,
  type RequestHistoryItem,
  type RequestResponsesPage,
} from '@/lib/api'
import { useAuth } from '@/hooks/useAuth'
import { useDashboardWebSocket } from '@/hooks/useWebSocket'
import { toast } from '@/hooks/use-toast'
import { Header } from '@/components/dashboard/Header'
import { StatsCards } from '@/components/dashboard/StatsCards'
import { EndpointTable } from '@/components/dashboard/EndpointTable'
import { RequestHistoryTable } from '@/components/dashboard/RequestHistoryTable'
import { LogViewer } from '@/components/dashboard/LogViewer'
import { TokenStatsSection } from '@/components/dashboard/TokenStatsSection'
import { Button } from '@/components/ui/button'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { AlertCircle, Globe, History, FileText, BarChart3, ArrowUpCircle, ExternalLink, Loader2 } from 'lucide-react'

export default function Dashboard() {
  const { user } = useAuth()
  const { isConnected: wsConnected } = useDashboardWebSocket()
  const [lastRefreshed, setLastRefreshed] = useState<Date | null>(null)
  const [fetchTimeMs, setFetchTimeMs] = useState<number | null>(null)
  const fetchStartRef = useRef<number | null>(null)
  const [isApplyingUpdate, setIsApplyingUpdate] = useState(false)

  // When WebSocket is connected, reduce polling frequency
  const pollingInterval = wsConnected ? 10000 : 5000

  const fetchWithTiming = useCallback(async () => {
    fetchStartRef.current = performance.now()
    const result = await dashboardApi.getOverview()
    const endTime = performance.now()
    setFetchTimeMs(Math.round(endTime - (fetchStartRef.current || endTime)))
    setLastRefreshed(new Date())
    return result
  }, [])

  const { data, isLoading, error, refetch } = useQuery<DashboardOverview>({
    queryKey: ['dashboard-overview'],
    queryFn: fetchWithTiming,
    refetchInterval: pollingInterval,
  })

  const {
    data: systemInfo,
    refetch: refetchSystemInfo,
  } = useQuery<SystemInfo>({
    queryKey: ['system-info'],
    queryFn: () => systemApi.getSystem(),
    refetchInterval: pollingInterval,
  })

  // Fetch request history (individual request details)
  const { data: requestResponsesData, isLoading: isLoadingHistory } =
    useQuery<RequestResponsesPage>({
      queryKey: ['request-responses'],
      queryFn: () => dashboardApi.getRequestResponses({ limit: 100 }),
      refetchInterval: pollingInterval,
    })

  // SPEC-e8e9326e: Fetch endpoints list
  const { data: endpointsData, isLoading: isLoadingEndpoints } = useQuery<DashboardEndpoint[]>({
    queryKey: ['dashboard-endpoints'],
    queryFn: () => dashboardApi.getEndpoints(),
    refetchInterval: pollingInterval,
  })

  // Map RequestResponseRecord to RequestHistoryItem
  const historyItems: RequestHistoryItem[] = useMemo(() => {
    if (!requestResponsesData?.records) return []
    return requestResponsesData.records.map((record) => ({
      request_id: record.id,
      timestamp: record.timestamp,
      model: record.model,
      node_id: record.node_id,
      node_name: record.node_machine_name,
      status: record.status.type,
      duration_ms: record.duration_ms,
      error: record.status.type === 'error' ? record.status.message : undefined,
      request_body: record.request_body,
      response_body: record.response_body,
    }))
  }, [requestResponsesData])

  const updateBanner = useMemo(() => {
    const update = systemInfo?.update as UpdateState | undefined
    if (!update || update.state === 'up_to_date') return null

    const isAdmin = user?.role === 'admin'
    const canApply = isAdmin && (update.state === 'available' || update.state === 'failed')
    const applying = update.state === 'draining' || update.state === 'applying'

    let title = 'Update'
    let description = ''
    let link: string | null = null
    let payloadHint: string | null = null

    if (update.state === 'available') {
      title = `Update available: v${update.latest}`
      description = `Current: v${update.current}`
      link = update.release_url
      if (update.payload?.payload === 'downloading') {
        payloadHint = 'Downloading...'
      } else if (update.payload?.payload === 'ready') {
        payloadHint = 'Ready'
      } else if (update.payload?.payload === 'error') {
        payloadHint = 'Download failed'
      } else {
        payloadHint = 'Preparing...'
      }
    } else if (update.state === 'draining') {
      title = `Updating to v${update.latest}`
      description = `Waiting for in-flight requests: ${update.in_flight}`
    } else if (update.state === 'applying') {
      title = `Applying update: v${update.latest}`
      description = 'Restarting...'
    } else if (update.state === 'failed') {
      title = 'Update failed'
      description = update.message
      link = update.release_url || null
    }

    const onApply = async () => {
      setIsApplyingUpdate(true)
      try {
        await systemApi.applyUpdate()
        toast({
          title: 'Update queued',
          description:
            'llmlb will restart after in-flight requests complete.',
        })
        await refetchSystemInfo()
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

    return (
      <section className="mb-6">
        <div className="rounded-2xl border border-border/60 bg-card/60 backdrop-blur-xl px-5 py-4 shadow-sm">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex items-start gap-3">
              <div className="mt-0.5 flex h-9 w-9 items-center justify-center rounded-xl bg-primary/10">
                {applying ? (
                  <Loader2 className="h-5 w-5 animate-spin text-primary" />
                ) : (
                  <ArrowUpCircle className="h-5 w-5 text-primary" />
                )}
              </div>
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <p className="font-medium leading-6">{title}</p>
                  {payloadHint && (
                    <span className="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
                      {payloadHint}
                    </span>
                  )}
                </div>
                {description && (
                  <p className="mt-0.5 text-sm text-muted-foreground">
                    {description}
                  </p>
                )}
              </div>
            </div>

            <div className="flex flex-wrap items-center gap-2">
              {link && (
                <a
                  href={link}
                  target="_blank"
                  rel="noreferrer"
                  className="inline-flex items-center gap-1 rounded-lg border border-border/60 bg-background/60 px-3 py-2 text-sm hover:bg-background"
                >
                  <ExternalLink className="h-4 w-4" />
                  Release
                </a>
              )}
              <Button
                onClick={onApply}
                disabled={!canApply || isApplyingUpdate || applying}
                title={
                  !isAdmin
                    ? 'Admin role is required'
                    : applying
                    ? 'Update is in progress'
                    : undefined
                }
              >
                {isApplyingUpdate ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <ArrowUpCircle className="h-4 w-4" />
                )}
                Restart to update
              </Button>
            </div>
          </div>
        </div>
      </section>
    )
  }, [systemInfo?.update, user?.role, isApplyingUpdate, refetchSystemInfo])

  if (error) {
    return (
      <div className="flex h-screen w-full items-center justify-center bg-background">
        <div className="flex flex-col items-center gap-4 text-center">
          <div className="flex h-16 w-16 items-center justify-center rounded-full bg-destructive/10">
            <AlertCircle className="h-8 w-8 text-destructive" />
          </div>
          <div>
            <h2 className="text-lg font-semibold">Failed to load dashboard</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              {error instanceof Error ? error.message : 'An error occurred'}
            </p>
          </div>
          <Button variant="link" onClick={() => refetch()}>
            Try again
          </Button>
        </div>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-background">
      {/* Background Grid */}
      <div className="fixed inset-0 bg-grid opacity-20 pointer-events-none" />

      {/* Header */}
      <Header
        user={user}
        isConnected={!error}
        lastRefreshed={lastRefreshed}
        fetchTimeMs={fetchTimeMs}
        systemVersion={systemInfo?.version ?? null}
      />

      {/* Main Content */}
      <main className="relative mx-auto max-w-[1600px] px-4 py-6 sm:px-6 lg:px-8">
        {updateBanner}
        {/* Stats Cards */}
        <section className="mb-8">
          <StatsCards stats={data?.stats} endpoints={endpointsData} isLoading={isLoading} />
        </section>

        {/* Tabs */}
        <Tabs defaultValue="endpoints" className="space-y-6">
          <TabsList className="grid w-full grid-cols-4 lg:w-auto lg:inline-grid">
            <TabsTrigger value="endpoints" className="gap-2">
              <Globe className="h-4 w-4" />
              <span className="hidden sm:inline">Endpoints</span>
            </TabsTrigger>
            <TabsTrigger value="statistics" className="gap-2">
              <BarChart3 className="h-4 w-4" />
              <span className="hidden sm:inline">Statistics</span>
            </TabsTrigger>
            <TabsTrigger value="history" className="gap-2">
              <History className="h-4 w-4" />
              <span className="hidden sm:inline">History</span>
            </TabsTrigger>
            <TabsTrigger value="logs" className="gap-2">
              <FileText className="h-4 w-4" />
              <span className="hidden sm:inline">Logs</span>
            </TabsTrigger>
          </TabsList>

          <TabsContent value="endpoints" className="animate-fade-in">
            <EndpointTable endpoints={endpointsData || []} isLoading={isLoadingEndpoints} />
          </TabsContent>

          <TabsContent value="statistics" className="animate-fade-in">
            <TokenStatsSection />
          </TabsContent>

          <TabsContent value="history" className="animate-fade-in">
            <RequestHistoryTable
              history={historyItems}
              isLoading={isLoadingHistory}
            />
          </TabsContent>

          <TabsContent value="logs" className="animate-fade-in">
            <LogViewer />
          </TabsContent>
        </Tabs>
      </main>
    </div>
  )
}
