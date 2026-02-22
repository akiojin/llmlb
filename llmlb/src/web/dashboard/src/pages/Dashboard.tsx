import { useState, useRef, useCallback, useMemo } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import {
  dashboardApi,
  modelsApi,
  systemApi,
  type SystemInfo,
  type UpdateState,
  type DashboardOverview,
  type DashboardEndpoint,
  type RequestHistoryItem,
  type RequestResponsesPage,
  type RegisteredModelView,
} from '@/lib/api'
import { useAuth } from '@/hooks/useAuth'
import { useDashboardWebSocket } from '@/hooks/useWebSocket'
import { toast } from '@/hooks/use-toast'
import { Header } from '@/components/dashboard/Header'
import { StatsCards } from '@/components/dashboard/StatsCards'
import { EndpointTable } from '@/components/dashboard/EndpointTable'
import { ModelsTable } from '@/components/dashboard/ModelsTable'
import { RequestHistoryTable } from '@/components/dashboard/RequestHistoryTable'
import { LogViewer } from '@/components/dashboard/LogViewer'
import { TokenStatsSection } from '@/components/dashboard/TokenStatsSection'
import { ClientsTab } from '@/components/dashboard/ClientsTab'
import { Button } from '@/components/ui/button'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  AlertCircle,
  AlertTriangle,
  Globe,
  History,
  FileText,
  BarChart3,
  ArrowUpCircle,
  ExternalLink,
  Loader2,
  RefreshCcw,
  Users,
} from 'lucide-react'

const SYSTEM_INFO_QUERY_KEY = ['system-info'] as const

export default function Dashboard() {
  const { user } = useAuth()
  const isViewer = user?.role === 'viewer'
  const { isConnected: wsConnected } = useDashboardWebSocket({ enabled: !isViewer })
  const queryClient = useQueryClient()
  const [lastRefreshed, setLastRefreshed] = useState<Date | null>(null)
  const [fetchTimeMs, setFetchTimeMs] = useState<number | null>(null)
  const fetchStartRef = useRef<number | null>(null)
  const [isApplyingUpdate, setIsApplyingUpdate] = useState(false)
  const [isApplyingForceUpdate, setIsApplyingForceUpdate] = useState(false)
  const [isForceUpdateDialogOpen, setIsForceUpdateDialogOpen] = useState(false)
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false)

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
  } = useQuery<SystemInfo>({
    queryKey: SYSTEM_INFO_QUERY_KEY,
    queryFn: () => systemApi.getSystem(),
    refetchInterval: pollingInterval,
    enabled: !isViewer,
  })

  // Fetch request history (individual request details)
  const { data: requestResponsesData, isLoading: isLoadingHistory } =
    useQuery<RequestResponsesPage>({
      queryKey: ['request-responses'],
      queryFn: () => dashboardApi.getRequestResponses({ limit: 100 }),
      refetchInterval: pollingInterval,
      enabled: !isViewer,
    })

  // SPEC-e8e9326e: Fetch endpoints list
  const { data: endpointsData, isLoading: isLoadingEndpoints } = useQuery<DashboardEndpoint[]>({
    queryKey: ['dashboard-endpoints'],
    queryFn: () => dashboardApi.getEndpoints(),
    refetchInterval: pollingInterval,
    enabled: !isViewer,
  })

  const {
    data: viewerModels,
    isLoading: isLoadingViewerModels,
    refetch: refetchViewerModels,
  } = useQuery<RegisteredModelView[]>({
    queryKey: ['viewer-models'],
    queryFn: () => modelsApi.getRegistered(),
    refetchInterval: pollingInterval,
    enabled: isViewer,
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
      client_ip: record.client_ip,
    }))
  }, [requestResponsesData])

  const updateBanner = useMemo(() => {
    const update = systemInfo?.update as UpdateState | undefined
    const updateState = update?.state
    const isAdmin = user?.role === 'admin'
    const hasAvailableUpdate = updateState === 'available'
    const isPayloadReady =
      hasAvailableUpdate && update?.payload?.payload === 'ready'
    const failedHasUpdateCandidate = updateState === 'failed' && Boolean(update?.latest)
    const canApply = isAdmin && (updateState === 'available' || failedHasUpdateCandidate)
    const applying = updateState === 'draining' || updateState === 'applying'
    const showRestartButton = updateState === 'available' || failedHasUpdateCandidate || applying
    const showForceButton = true
    const canForceApply = isAdmin && isPayloadReady && !applying
    const canCheck = isAdmin && !applying
    const forceUpdateTitle = !isAdmin
      ? 'Admin role is required'
      : applying
        ? 'Update is in progress'
        : !hasAvailableUpdate
          ? 'No update is available'
        : isPayloadReady
          ? undefined
          : 'Update payload is still preparing'

    let title = 'Update'
    let description = 'Update status unavailable'
    let link: string | null = null
    let payloadHint: string | null = null

    if (updateState === 'available' && update) {
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
      description = 'Restarting...'
    } else if (updateState === 'failed' && update) {
      title = 'Update failed'
      description = update.message
      link = update.release_url || null
    }

    const onCheck = async () => {
      setIsCheckingUpdate(true)
      try {
        const { update } = await systemApi.checkUpdate()
        const currentSystemInfo = queryClient.getQueryData<SystemInfo>(SYSTEM_INFO_QUERY_KEY)
        if (currentSystemInfo) {
          queryClient.setQueryData<SystemInfo>(
            SYSTEM_INFO_QUERY_KEY,
            {
              ...currentSystemInfo,
              update,
            }
          )
        } else {
          const freshSystemInfo = await systemApi.getSystem()
          queryClient.setQueryData<SystemInfo>(
            SYSTEM_INFO_QUERY_KEY,
            {
              ...freshSystemInfo,
              update,
            }
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
        await queryClient.invalidateQueries({ queryKey: SYSTEM_INFO_QUERY_KEY })
      }
    }

    const onApply = async () => {
      setIsApplyingUpdate(true)
      try {
        const result = await systemApi.applyUpdate()
        if (result.queued) {
          toast({
            title: 'Update queued',
            description:
              'llmlb will restart after in-flight requests complete.',
          })
        } else {
          toast({
            title: 'Applying update',
            description: 'llmlb is restarting now.',
          })
        }
        await queryClient.invalidateQueries({ queryKey: SYSTEM_INFO_QUERY_KEY })
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
        await queryClient.invalidateQueries({ queryKey: SYSTEM_INFO_QUERY_KEY })
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
                variant="outline"
                onClick={onCheck}
                disabled={!canCheck || isCheckingUpdate || isApplyingUpdate || isApplyingForceUpdate}
                title={
                  !isAdmin
                    ? 'Admin role is required'
                    : applying
                      ? 'Update is in progress'
                      : undefined
                }
              >
                {isCheckingUpdate ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <RefreshCcw className="h-4 w-4" />
                )}
                Check for updates
              </Button>
              {showRestartButton && (
                <Button
                  onClick={onApply}
                  disabled={!canApply || isApplyingUpdate || isApplyingForceUpdate || applying}
                  title={
                    !isAdmin
                      ? 'Admin role is required'
                      : applying
                        ? 'Update is in progress'
                        : undefined
                  }
                >
                  {isApplyingUpdate || applying ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <ArrowUpCircle className="h-4 w-4" />
                  )}
                  {update?.state === 'draining'
                    ? `Waiting to update... (${update.in_flight})`
                    : update?.state === 'applying'
                      ? 'Applying update...'
                      : 'Restart to update'}
                </Button>
              )}
              {showForceButton && (
                <AlertDialog
                  open={isForceUpdateDialogOpen}
                  onOpenChange={setIsForceUpdateDialogOpen}
                >
                  <AlertDialogTrigger asChild>
                    <Button
                      variant="destructive"
                      disabled={!canForceApply || isApplyingUpdate || isApplyingForceUpdate}
                      title={forceUpdateTitle}
                    >
                      {isApplyingForceUpdate ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : (
                        <AlertTriangle className="h-4 w-4" />
                      )}
                      Force update now
                    </Button>
                  </AlertDialogTrigger>
                  <AlertDialogContent>
                    <AlertDialogHeader>
                      <AlertDialogTitle>Force update now?</AlertDialogTitle>
                      <AlertDialogDescription>
                        In-flight inference requests will be terminated immediately and llmlb will restart.
                        Use this only for urgent maintenance.
                      </AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                      <AlertDialogCancel disabled={isApplyingForceUpdate}>Cancel</AlertDialogCancel>
                      <AlertDialogAction
                        disabled={isApplyingForceUpdate}
                        onClick={(event) => {
                          event.preventDefault()
                          void onForceApply()
                        }}
                      >
                        {isApplyingForceUpdate ? (
                          <>
                            <Loader2 className="h-4 w-4 animate-spin" />
                            Applying...
                          </>
                        ) : (
                          'Force update'
                        )}
                      </AlertDialogAction>
                    </AlertDialogFooter>
                  </AlertDialogContent>
                </AlertDialog>
              )}
            </div>
          </div>
        </div>
      </section>
    )
  }, [
    systemInfo?.update,
    user?.role,
    isApplyingUpdate,
    isApplyingForceUpdate,
    isForceUpdateDialogOpen,
    isCheckingUpdate,
    queryClient,
  ])

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
        updateState={(systemInfo?.update as UpdateState | undefined)?.state}
        updateLatest={(() => {
          const u = systemInfo?.update as UpdateState | undefined
          if (!u) return null
          if ('latest' in u) return u.latest ?? null
          return null
        })()}
        minimalViewer={isViewer}
      />

      {/* Main Content */}
      <main className="relative mx-auto max-w-[1600px] px-4 py-6 sm:px-6 lg:px-8">
        {!isViewer && updateBanner}
        {/* Stats Cards */}
        <section className="mb-8">
          <StatsCards stats={data?.stats} endpoints={endpointsData} isLoading={isLoading} />
        </section>

        {isViewer ? (
          <section className="mb-8">
            <ModelsTable
              models={viewerModels || []}
              endpoints={endpointsData || []}
              isLoading={isLoadingViewerModels}
              onRefresh={() => {
                void refetchViewerModels()
              }}
              viewerMode
            />
          </section>
        ) : (
          <Tabs defaultValue="endpoints" className="space-y-6">
            <TabsList className="grid w-full grid-cols-5 lg:w-auto lg:inline-grid">
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
              <TabsTrigger value="clients" className="gap-2">
                <Users className="h-4 w-4" />
                <span className="hidden sm:inline">Clients</span>
              </TabsTrigger>
              <TabsTrigger value="logs" className="gap-2">
                <FileText className="h-4 w-4" />
                <span className="hidden sm:inline">Logs</span>
              </TabsTrigger>
            </TabsList>

            <TabsContent value="endpoints" className="animate-fade-in">
              <EndpointTable
                endpoints={endpointsData || []}
                endpointTps={data?.endpoint_tps}
                isLoading={isLoadingEndpoints}
              />
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

            <TabsContent value="clients" className="animate-fade-in">
              <ClientsTab />
            </TabsContent>

            <TabsContent value="logs" className="animate-fade-in">
              <LogViewer />
            </TabsContent>
          </Tabs>
        )}
      </main>
    </div>
  )
}
