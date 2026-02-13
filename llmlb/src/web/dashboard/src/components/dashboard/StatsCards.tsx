import { type DashboardEndpoint, type DashboardStats } from '@/lib/api'
import { formatDuration, formatFullNumber, formatPercentage } from '@/lib/utils'
import { Card, CardContent } from '@/components/ui/card'
import {
  Server,
  Activity,
  Clock,
  Cpu,
  CheckCircle2,
  XCircle,
  Zap,
  HardDrive,
  Hourglass,
  MessageSquare,
} from 'lucide-react'

interface StatsCardsProps {
  stats?: DashboardStats
  endpoints?: DashboardEndpoint[]
  isLoading: boolean
}

interface StatCardProps {
  title: string
  value: string | number
  subtitle?: string
  icon: React.ReactNode
  trend?: 'up' | 'down' | 'neutral'
  accentColor?: string
  isLoading?: boolean
  delay?: number
  dataStat?: string
}

// Tailwind JIT では動的クラス生成が機能しないため、静的マップを使用
const accentColorClasses: Record<string, string> = {
  primary: 'bg-primary/10 group-hover:bg-primary/20',
  'chart-1': 'bg-chart-1/10 group-hover:bg-chart-1/20',
  'chart-2': 'bg-chart-2/10 group-hover:bg-chart-2/20',
  'chart-3': 'bg-chart-3/10 group-hover:bg-chart-3/20',
  'chart-4': 'bg-chart-4/10 group-hover:bg-chart-4/20',
  'chart-5': 'bg-chart-5/10 group-hover:bg-chart-5/20',
  success: 'bg-success/10 group-hover:bg-success/20',
  warning: 'bg-warning/10 group-hover:bg-warning/20',
  destructive: 'bg-destructive/10 group-hover:bg-destructive/20',
}

function StatCard({
  title,
  value,
  subtitle,
  icon,
  accentColor = 'primary',
  isLoading,
  delay = 0,
  dataStat,
}: StatCardProps) {
  return (
    <Card
      className={`stat-card group overflow-hidden animate-fade-up`}
      style={{ animationDelay: `${delay}ms` }}
      data-stat={dataStat}
    >
      <CardContent className="p-6">
        <div className="flex items-start justify-between">
          <div className="space-y-2">
            <p className="text-sm font-medium text-muted-foreground">{title}</p>
            {isLoading ? (
              <div className="h-8 w-24 shimmer rounded" />
            ) : (
              <p className="text-3xl font-bold tracking-tight">{value}</p>
            )}
            {subtitle && !isLoading && (
              <p className="text-xs text-muted-foreground">{subtitle}</p>
            )}
          </div>
          <div
            className={`flex h-10 w-10 items-center justify-center rounded-lg transition-colors ${accentColorClasses[accentColor] || accentColorClasses.primary}`}
          >
            {icon}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

export function StatsCards({ stats, endpoints, isLoading }: StatsCardsProps) {
  const runtimeCounts = stats
    ? {
        total: stats.total_runtimes ?? stats.total_nodes,
        online: stats.online_runtimes ?? stats.online_nodes,
        pending: stats.pending_runtimes ?? stats.pending_nodes,
        registering: stats.registering_runtimes ?? stats.registering_nodes,
        offline: stats.offline_runtimes ?? stats.offline_nodes,
      }
    : undefined

  const endpointCounts = endpoints
    ? {
        total: endpoints.length,
        online: endpoints.filter((e) => e.status === 'online').length,
        pending: endpoints.filter((e) => e.status === 'pending').length,
        offline: endpoints.filter((e) => e.status === 'offline').length,
        error: endpoints.filter((e) => e.status === 'error').length,
      }
    : undefined

  const totalEndpoints = endpointCounts?.total ?? runtimeCounts?.total
  const onlineEndpoints = endpointCounts?.online ?? runtimeCounts?.online
  const pendingEndpoints = endpointCounts?.pending ?? runtimeCounts?.pending
  const offlineEndpoints = endpointCounts?.offline ?? runtimeCounts?.offline
  const errorEndpoints = endpointCounts?.error

  const endpointsSubtitle =
    onlineEndpoints != null &&
    pendingEndpoints != null &&
    offlineEndpoints != null &&
    (errorEndpoints == null || errorEndpoints >= 0)
      ? [
          `${onlineEndpoints} online`,
          `${pendingEndpoints} pending`,
          `${offlineEndpoints} offline`,
          ...(errorEndpoints != null && errorEndpoints > 0
            ? [`${errorEndpoints} error`]
            : []),
        ].join(', ')
      : undefined

  const cards = [
    {
      title: 'Total Endpoints',
      value: totalEndpoints != null ? formatFullNumber(totalEndpoints) : '—',
      subtitle: endpointsSubtitle,
      icon: <Server className="h-5 w-5 text-primary" />,
      accentColor: 'primary',
      dataStat: 'total-endpoints',
    },
    {
      title: 'Total Requests',
      value: stats ? formatFullNumber(stats.total_requests) : '—',
      subtitle: stats
        ? `${formatFullNumber(stats.successful_requests)} successful`
        : undefined,
      icon: <Activity className="h-5 w-5 text-chart-2" />,
      accentColor: 'chart-2',
      dataStat: 'total-requests',
    },
    {
      title: 'Total Tokens',
      value: stats ? formatFullNumber(stats.total_tokens) : '—',
      subtitle: stats
        ? `In: ${formatFullNumber(stats.total_input_tokens)} / Out: ${formatFullNumber(
            stats.total_output_tokens
          )}`
        : undefined,
      icon: <MessageSquare className="h-5 w-5 text-chart-5" />,
      accentColor: 'chart-5',
      dataStat: 'total-tokens',
    },
    {
      title: 'Active Requests',
      value: stats ? formatFullNumber(stats.total_active_requests) : '—',
      icon: <Cpu className="h-5 w-5 text-chart-1" />,
      accentColor: 'chart-1',
      dataStat: 'active-requests',
    },
    {
      title: 'Queued Requests',
      value: stats ? formatFullNumber(stats.queued_requests) : '—',
      icon: <Hourglass className="h-5 w-5 text-chart-5" />,
      accentColor: 'chart-5',
      dataStat: 'queued-requests',
    },
    {
      title: 'Success Rate',
      value:
        stats && stats.total_requests > 0
          ? formatPercentage(
              (stats.successful_requests / stats.total_requests) * 100
            )
          : '—',
      subtitle: stats
        ? `${formatFullNumber(stats.failed_requests)} failed`
        : undefined,
      icon:
        stats && stats.failed_requests > 0 ? (
          <XCircle className="h-5 w-5 text-destructive" />
        ) : (
          <CheckCircle2 className="h-5 w-5 text-success" />
        ),
      accentColor: stats && stats.failed_requests > 0 ? 'destructive' : 'success',
      dataStat: 'success-rate',
    },
    {
      title: 'Avg Response Time',
      value: stats
        ? formatDuration(stats.average_response_time_ms)
        : '—',
      icon: <Clock className="h-5 w-5 text-warning" />,
      accentColor: 'warning',
      dataStat: 'average-response-time-ms',
    },
    {
      title: 'Avg GPU Usage',
      value: stats ? formatPercentage(stats.average_gpu_usage) : '—',
      icon: <Zap className="h-5 w-5 text-chart-3" />,
      accentColor: 'chart-3',
      dataStat: 'average-gpu-usage',
    },
    {
      title: 'Avg GPU Memory',
      value: stats
        ? formatPercentage(stats.average_gpu_memory_usage)
        : '—',
      icon: <HardDrive className="h-5 w-5 text-chart-4" />,
      accentColor: 'chart-4',
      dataStat: 'average-gpu-memory-usage',
    },
  ]

  return (
    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6">
      {cards.map((card, index) => (
        <StatCard
          key={card.title}
          {...card}
          isLoading={isLoading}
          delay={index * 50}
        />
      ))}
    </div>
  )
}
