import type { ReactNode } from 'react'
import { Activity, AlertTriangle, CheckCircle2, Clock, Cpu, Gauge, ListChecks, Server } from 'lucide-react'
import { type DashboardOverview } from '@/lib/api'
import type { DashboardActivity } from '@/lib/api/dashboard'
import { cn, formatBytes, formatFullNumber, formatPercentage } from '@/lib/utils'
import { ActivityIndicator } from './ActivityIndicator'

interface OperationsOverviewProps {
  overview?: DashboardOverview
  isLoading: boolean
  activity?: DashboardActivity
  activityStatus?: 'live' | 'polling' | 'stale'
  todayRequests?: number
}

interface MetricCellProps {
  dataStat: string
  label: string
  value: string
  detail?: string
  icon: ReactNode
  activeState?: 'busy' | 'idle' | 'unavailable'
}

function metricValue(value: number | null | undefined): string {
  return value == null ? '-' : formatFullNumber(value)
}

function byteValue(value: number | null | undefined): string {
  return value == null ? '-' : formatBytes(value, 1)
}

function tpsValue(value: number | null | undefined): string {
  if (value == null) return '-'
  return `${value >= 100 ? value.toFixed(0) : value.toFixed(1)} tok/s`
}

function healthLabel(health: string | undefined): string {
  if (health === 'healthy') return 'Healthy'
  if (health === 'attention') return 'Attention'
  if (health === 'empty') return 'No endpoints'
  return 'Unknown'
}

function MetricCell({ dataStat, label, value, detail, icon, activeState }: MetricCellProps) {
  return (
    <div data-stat={dataStat} data-state={activeState} className={cn('min-w-0 rounded-lg border px-3 py-3 sm:px-4', activeState === 'busy' ? 'border-primary/35 bg-primary/10' : 'border-border/70 bg-background/40')}>
      <div className="mb-3 flex items-center justify-between gap-2">
        <p className="text-[10px] font-medium uppercase tracking-wider text-muted-foreground sm:text-xs">{label}</p>
        <div className={activeState === 'busy' ? 'text-accent-foreground' : 'text-muted-foreground'}>{icon}</div>
      </div>
      <p title={value} className={cn('truncate font-mono font-semibold tracking-tight tabular-nums', dataStat === 'output-tps' ? 'text-xl sm:text-2xl' : 'text-2xl sm:text-3xl', activeState === 'busy' ? 'text-accent-foreground' : '')}>{value}</p>
      {activeState ? (
        <span className={cn('mt-2 inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[10px] font-medium', activeState === 'busy' ? 'bg-primary/15 text-foreground motion-safe:animate-pulse' : 'bg-muted text-muted-foreground')}>
          <span className={cn('h-1 w-1 rounded-full', activeState === 'busy' ? 'bg-accent-foreground' : 'bg-muted-foreground')} aria-hidden="true" />
          {activeState === 'busy' ? '処理中' : activeState === 'idle' ? 'アイドル' : '取得待ち'}
        </span>
      ) : detail ? <p className="mt-2 text-[10px] text-muted-foreground sm:text-xs">{detail}</p> : null}
    </div>
  )
}

export function OperationsOverview({ overview, isLoading, activity, activityStatus = 'polling', todayRequests }: OperationsOverviewProps) {
  const operations = overview?.operations
  const activeRequests = activity?.total.active_requests ?? operations?.active_requests
  const queuedRequests = activity?.total.queued_requests ?? operations?.queued_requests
  const capacity = overview?.capacity
  const actionItems = overview?.action_items ?? []
  const loadingValue = isLoading ? '-' : undefined
  const lastFive = activity?.total.requests_per_second.slice(-5)
  const averageRate = lastFive ? (lastFive.reduce((total, count) => total + count, 0) / 5).toFixed(1) : '-'
  const rates = activity?.total.requests_per_second
  const currentRate = rates?.[rates.length - 1]
  const activeState = activeRequests == null ? 'unavailable' : activeRequests > 0 ? 'busy' : 'idle'
  const failedAndImpaired = (operations?.failed_requests ?? 0) + (operations?.offline_endpoints ?? 0) + (queuedRequests ?? 0)

  return (
    <section className="space-y-4" aria-label="Operations overview">
      <div className="grid gap-4 xl:grid-cols-[1.05fr_1fr]">
        <div className="rounded-xl border border-border/70 bg-card p-4 sm:p-5">
          <div className="mb-4 flex items-center justify-between gap-3">
            <h2 className="text-sm font-semibold tracking-tight">Operations Overview</h2>
            <span className="text-[11px] text-muted-foreground">All endpoints</span>
          </div>
          <div data-testid="operations-tiles" className="grid grid-cols-3 gap-2 sm:gap-3">
            <MetricCell dataStat="active-requests" label="Active" value={activity ? metricValue(activeRequests) : loadingValue ?? metricValue(activeRequests)} activeState={activeState} icon={<Cpu className="h-3.5 w-3.5" />} />
            <MetricCell dataStat="queued-requests" label="Queued" value={activity ? metricValue(queuedRequests) : loadingValue ?? metricValue(queuedRequests)} detail="waiting" icon={<Clock className="h-3.5 w-3.5" />} />
            <MetricCell dataStat="requests-per-second" label="Req/s" value={averageRate} detail="直近5秒平均" icon={<Activity className="h-3.5 w-3.5" />} />
            <MetricCell dataStat="total-requests" label="Requests" value={todayRequests == null ? '—' : metricValue(todayRequests)} detail="本日累計 · UTC" icon={<Activity className="h-3.5 w-3.5" />} />
            <MetricCell dataStat="output-tps" label="Output TPS" value={loadingValue ?? tpsValue(operations?.output_tps)} detail={`${metricValue(operations?.total_output_tokens)} output tokens`} icon={<Gauge className="h-3.5 w-3.5" />} />
            <MetricCell dataStat="total-endpoints" label="Endpoints" value={loadingValue ?? `${metricValue(operations?.online_endpoints)} / ${metricValue(operations?.total_endpoints)}`} detail="online / total" icon={<Server className="h-3.5 w-3.5" />} />
          </div>
        </div>

        <section aria-label="Request activity" className="flex min-w-0 flex-col rounded-xl border border-border/70 bg-card p-4 sm:p-5">
          <div className="flex items-center justify-between gap-2">
            <h2 className="text-sm font-semibold tracking-tight">Requests per second</h2>
            <span className="text-[11px] text-muted-foreground">{activityStatus === 'live' ? 'Live · 1s' : activityStatus === 'stale' ? 'Update delayed' : 'Polling · 1s'}</span>
          </div>
          <div className="mb-3 mt-3 flex items-baseline gap-2">
            <p data-testid="activity-current-rate" className="font-mono text-4xl font-semibold tracking-tight tabular-nums">{currentRate ?? '—'}</p>
            <span className="text-xs text-muted-foreground">req/s（現在）</span>
          </div>
          <div className="mt-auto">
            <ActivityIndicator activity={activity?.total} timestamp={activity?.timestamp} isStale={activityStatus === 'stale'} label="All endpoints request activity" />
          </div>
        </section>
      </div>

      <div className="grid gap-4 lg:grid-cols-[1fr_1.4fr]">
        <div data-stat="operational-health" className="flex items-center gap-3 rounded-lg border border-border/70 bg-card px-4 py-3">
          {operations?.health === 'healthy' ? <CheckCircle2 className="h-5 w-5 shrink-0 text-success" /> : <AlertTriangle className="h-5 w-5 shrink-0 text-warning" />}
          <div>
            <p className="text-sm font-medium">{loadingValue ?? healthLabel(operations?.health)}</p>
            <p className="mt-1 text-xs text-muted-foreground">{`${metricValue(operations?.online_endpoints)} online, ${metricValue(failedAndImpaired)} requiring review`}</p>
          </div>
        </div>
        <div data-section="action-items" className="rounded-lg border border-border/70 bg-card px-4 py-3">
          <div className="mb-2 flex items-center gap-2"><ListChecks className="h-4 w-4 text-muted-foreground" /><p className="text-sm font-medium">Action Items</p></div>
          <div className="space-y-2">
            {(isLoading ? [] : actionItems).map((item, index) => <div key={`${item.title}-${index}`} className="flex items-start justify-between gap-3 rounded-md bg-muted/40 px-3 py-2">
              <div className="min-w-0"><p className="truncate text-sm font-medium">{item.title}</p><p className="truncate text-xs text-muted-foreground">{item.detail}</p></div>
              <span className="rounded-md bg-background px-2 py-0.5 text-xs tabular-nums">{item.count}</span>
            </div>)}
            {isLoading ? <div className="h-8 rounded-md bg-muted/50" /> : null}
          </div>
        </div>
      </div>

      <div className="grid gap-3 sm:grid-cols-3 lg:grid-cols-6">
        <div data-stat="success-rate" className="rounded-md border border-border/70 bg-card px-4 py-3"><p className="text-xs text-muted-foreground">Success</p><p className="mt-2 text-xl font-semibold">{loadingValue ?? formatPercentage(operations?.success_rate)}</p><p className="mt-1 text-xs text-muted-foreground">{`${metricValue(operations?.failed_requests)} failed`}</p></div>
        <div data-stat="total-tokens" className="rounded-md border border-border/70 bg-card px-4 py-3"><p className="text-xs text-muted-foreground">Tokens</p><p className="mt-2 text-xl font-semibold">{loadingValue ?? metricValue(operations?.total_tokens)}</p><p className="mt-1 text-xs text-muted-foreground">{`${metricValue(operations?.total_output_tokens)} out`}</p></div>
        <div data-stat="gpu-capacity" className="rounded-md border border-border/70 bg-card px-4 py-3"><p className="text-xs text-muted-foreground">GPU endpoints</p><p className="mt-2 text-xl font-semibold">{loadingValue ?? metricValue(capacity?.gpu_capable_endpoints)}</p><p className="mt-1 text-xs text-muted-foreground">{`${metricValue(capacity?.gpu_telemetry_endpoints)} reporting`}</p></div>
        <div className="rounded-md border border-border/70 bg-card px-4 py-3"><p className="text-xs text-muted-foreground">Model capacity</p><p className="mt-2 text-xl font-semibold">{loadingValue ?? metricValue(capacity?.total_models)}</p></div>
        <div className="rounded-md border border-border/70 bg-card px-4 py-3"><p className="text-xs text-muted-foreground">GPU memory</p><p className="mt-2 text-xl font-semibold">{loadingValue ?? byteValue(capacity?.used_gpu_memory_bytes)}</p><p className="mt-1 text-xs text-muted-foreground">{`${byteValue(capacity?.total_gpu_memory_bytes)} total, ${formatPercentage(capacity?.gpu_memory_usage_percent)}`}</p></div>
        <div className="rounded-md border border-border/70 bg-card px-4 py-3"><p className="text-xs text-muted-foreground">Telemetry</p><p className="mt-2 text-xl font-semibold">{loadingValue ?? capacity?.telemetry_status ?? '-'}</p></div>
      </div>
    </section>
  )
}
