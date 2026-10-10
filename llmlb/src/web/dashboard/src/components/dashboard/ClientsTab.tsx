import { useClientsTabViewModel } from '@/viewmodels/useClientsTabViewModel'
import { ClientBarChart } from './ClientBarChart'
import { ClientRankingTable } from './ClientRankingTable'
import { UniqueIpTimeline } from './UniqueIpTimeline'
import { ModelDistributionPie } from './ModelDistributionPie'
import { RequestHeatmap } from './RequestHeatmap'
import { AlertThresholdSettings } from './AlertThresholdSettings'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Users, TrendingUp, PieChart, Grid3X3, Loader2, X } from 'lucide-react'

export function ClientsTab() {
  const {
    isLoading, page, perPage, ipFilterLabel, rankings, totalCount,
    timelineData, modelsData, heatmapData, setPage, clearIpFilter,
  } = useClientsTabViewModel()

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-16">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
        <span className="ml-2 text-muted-foreground">Loading client data...</span>
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <Users className="h-4 w-4" />
            Top Clients by Request Count
          </CardTitle>
        </CardHeader>
        <CardContent>
          <ClientBarChart rankings={rankings} />
        </CardContent>
      </Card>

      <div className="grid gap-6 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <TrendingUp className="h-4 w-4" />
              Unique IPs (24h)
            </CardTitle>
          </CardHeader>
          <CardContent>
            <UniqueIpTimeline data={timelineData} />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <PieChart className="h-4 w-4" />
              Model Distribution
            </CardTitle>
          </CardHeader>
          <CardContent>
            <ModelDistributionPie data={modelsData} />
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle className="flex items-center gap-2 text-base">
            <Grid3X3 className="h-4 w-4" />
            Request Heatmap (Hour x Day)
          </CardTitle>
          {ipFilterLabel && (
            <div className="flex items-center gap-2">
              <Badge variant="secondary" className="text-xs">
                {ipFilterLabel}
              </Badge>
              <button
                onClick={clearIpFilter}
                className="text-muted-foreground hover:text-foreground transition-colors"
                title="Clear IP filter"
              >
                <X className="h-4 w-4" />
              </button>
            </div>
          )}
        </CardHeader>
        <CardContent>
          <RequestHeatmap data={heatmapData} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-col items-start gap-3">
          <div className="flex w-full items-center justify-between">
            <CardTitle className="flex items-center gap-2 text-base">
              <Users className="h-4 w-4" />
              Client IP Ranking
            </CardTitle>
            <AlertThresholdSettings />
          </div>
          {ipFilterLabel && (
            <div className="flex items-center gap-2">
              <Badge variant="secondary" className="text-xs">
                {ipFilterLabel}
              </Badge>
              <button
                onClick={clearIpFilter}
                className="text-muted-foreground hover:text-foreground transition-colors"
                title="Clear IP filter"
              >
                <X className="h-4 w-4" />
              </button>
            </div>
          )}
        </CardHeader>
        <CardContent>
          <ClientRankingTable
            rankings={rankings}
            totalCount={totalCount}
            page={page}
            perPage={perPage}
            onPageChange={setPage}
          />
        </CardContent>
      </Card>
    </div>
  )
}
