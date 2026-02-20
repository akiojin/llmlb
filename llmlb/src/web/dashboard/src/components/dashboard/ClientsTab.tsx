import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import {
  clientsApi,
  type ClientRankingResponse,
  type UniqueIpTimelinePoint,
  type ModelDistribution,
  type HeatmapCell,
} from '@/lib/api'
import { ClientBarChart } from './ClientBarChart'
import { ClientRankingTable } from './ClientRankingTable'
import { UniqueIpTimeline } from './UniqueIpTimeline'
import { ModelDistributionPie } from './ModelDistributionPie'
import { RequestHeatmap } from './RequestHeatmap'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Users, TrendingUp, PieChart, Grid3X3, Loader2 } from 'lucide-react'

export function ClientsTab() {
  const [page, setPage] = useState(1)
  const perPage = 20

  const { data, isLoading } = useQuery<ClientRankingResponse>({
    queryKey: ['client-ranking', page, perPage],
    queryFn: () => clientsApi.getClientRanking({ page, per_page: perPage }),
  })

  const { data: timelineData } = useQuery<UniqueIpTimelinePoint[]>({
    queryKey: ['client-timeline'],
    queryFn: () => clientsApi.getTimeline(),
  })

  const { data: modelsData } = useQuery<ModelDistribution[]>({
    queryKey: ['client-models'],
    queryFn: () => clientsApi.getModels(),
  })

  const { data: heatmapData } = useQuery<HeatmapCell[]>({
    queryKey: ['client-heatmap'],
    queryFn: () => clientsApi.getHeatmap(),
  })

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-16">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
        <span className="ml-2 text-muted-foreground">Loading client data...</span>
      </div>
    )
  }

  const rankings = data?.rankings ?? []
  const totalCount = data?.total_count ?? 0

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
            <UniqueIpTimeline data={timelineData ?? []} />
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
            <ModelDistributionPie data={modelsData ?? []} />
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <Grid3X3 className="h-4 w-4" />
            Request Heatmap (Hour x Day)
          </CardTitle>
        </CardHeader>
        <CardContent>
          <RequestHeatmap data={heatmapData ?? []} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <Users className="h-4 w-4" />
            Client IP Ranking
          </CardTitle>
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
