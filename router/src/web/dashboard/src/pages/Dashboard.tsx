import { useState, useRef, useCallback } from 'react'
import { useQuery } from '@tanstack/react-query'
import { dashboardApi, type DashboardOverview } from '@/lib/api'
import { useAuth } from '@/hooks/useAuth'
import { Header } from '@/components/dashboard/Header'
import { StatsCards } from '@/components/dashboard/StatsCards'
import { NodeTable } from '@/components/dashboard/NodeTable'
import { RequestHistoryTable } from '@/components/dashboard/RequestHistoryTable'
import { LogViewer } from '@/components/dashboard/LogViewer'
import { ModelsSection } from '@/components/models/ModelsSection'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { AlertCircle, Server, History, FileText, Box } from 'lucide-react'

export default function Dashboard() {
  const { user } = useAuth()
  const [lastRefreshed, setLastRefreshed] = useState<Date | null>(null)
  const [fetchTimeMs, setFetchTimeMs] = useState<number | null>(null)
  const fetchStartRef = useRef<number | null>(null)

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
    refetchInterval: 5000,
  })

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
          <TabsList className="grid w-full grid-cols-4 lg:w-auto lg:inline-grid">
            <TabsTrigger value="nodes" className="gap-2">
              <Server className="h-4 w-4" />
              <span className="hidden sm:inline">Nodes</span>
            </TabsTrigger>
            <TabsTrigger value="models" className="gap-2">
              <Box className="h-4 w-4" />
              <span className="hidden sm:inline">Models</span>
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

          <TabsContent value="history" className="animate-fade-in">
            <RequestHistoryTable
              history={data?.history || []}
              isLoading={isLoading}
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
