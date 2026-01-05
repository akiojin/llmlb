import { useState, useRef, useCallback, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import {
  dashboardApi,
  type DashboardOverview,
  type RequestHistoryItem,
  type RequestResponsesPage,
} from '@/lib/api'
import { useAuth } from '@/hooks/useAuth'
import { useDashboardWebSocket } from '@/hooks/useWebSocket'
import { Header } from '@/components/dashboard/Header'
import { StatsCards } from '@/components/dashboard/StatsCards'
import { NodeTable } from '@/components/dashboard/NodeTable'
import { RequestHistoryTable } from '@/components/dashboard/RequestHistoryTable'
import { LogViewer } from '@/components/dashboard/LogViewer'
import { TokenStatsSection } from '@/components/dashboard/TokenStatsSection'
import { ModelsSection } from '@/components/models/ModelsSection'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { AlertCircle, Server, History, FileText, Box, BarChart3 } from 'lucide-react'

export default function Dashboard() {
  const { user } = useAuth()
  const { isConnected: wsConnected } = useDashboardWebSocket()
  const [lastRefreshed, setLastRefreshed] = useState<Date | null>(null)
  const [fetchTimeMs, setFetchTimeMs] = useState<number | null>(null)
  const fetchStartRef = useRef<number | null>(null)

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

  // リクエスト履歴（個別リクエスト詳細）を取得
  const { data: requestResponsesData, isLoading: isLoadingHistory } =
    useQuery<RequestResponsesPage>({
      queryKey: ['request-responses'],
      queryFn: () => dashboardApi.getRequestResponses({ limit: 100 }),
      refetchInterval: pollingInterval,
    })

  // RequestResponseRecord を RequestHistoryItem にマッピング
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
          <button
            onClick={() => refetch()}
            className="text-sm text-primary hover:underline"
          >
            Try again
          </button>
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
      />

      {/* Main Content */}
      <main className="relative mx-auto max-w-[1600px] px-4 py-6 sm:px-6 lg:px-8">
        {/* Stats Cards */}
        <section className="mb-8">
          <StatsCards stats={data?.stats} isLoading={isLoading} />
        </section>

        {/* Tabs */}
        <Tabs defaultValue="nodes" className="space-y-6">
          <TabsList className="grid w-full grid-cols-5 lg:w-auto lg:inline-grid">
            <TabsTrigger value="nodes" className="gap-2">
              <Server className="h-4 w-4" />
              <span className="hidden sm:inline">Nodes</span>
            </TabsTrigger>
            <TabsTrigger value="models" className="gap-2">
              <Box className="h-4 w-4" />
              <span className="hidden sm:inline">Models</span>
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

          <TabsContent value="nodes" className="animate-fade-in">
            <NodeTable nodes={data?.nodes || []} isLoading={isLoading} />
          </TabsContent>

          <TabsContent value="models" className="animate-fade-in">
            <ModelsSection />
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
            <LogViewer nodes={data?.nodes || []} />
          </TabsContent>
        </Tabs>
      </main>
    </div>
  )
}
