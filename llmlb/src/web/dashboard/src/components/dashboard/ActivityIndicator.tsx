import { memo, useState } from 'react'
import type { RequestActivity } from '@/lib/api/dashboard'
import { cn } from '@/lib/utils'

interface ActivityIndicatorProps {
  activity?: RequestActivity
  isStale?: boolean
  compact?: boolean
  label?: string
  capacity?: number | null
  timestamp?: string
}

function samplesFor(activity?: RequestActivity) {
  const samples = activity?.requests_per_second.slice(-60) ?? []
  return [...Array<number>(60 - samples.length).fill(0), ...samples]
}

/** A separate, fixed sixty-second scale for each endpoint's arrivals. */
export const RequestRateSparkline = memo(function RequestRateSparkline({ activity }: { activity?: RequestActivity }) {
  const values = samplesFor(activity)
  const peak = Math.max(1, ...values)
  const points = values.map((value, index) => `${(index * 148 / 59 + 1).toFixed(1)},${(29 - value * 26 / peak).toFixed(1)}`).join(' ')
  return (
    <svg role="img" aria-label="Requests per second over the last 60 seconds" data-sample-count={activity ? 60 : 0} viewBox="0 0 150 32" preserveAspectRatio="none" className="h-8 w-36 text-accent-foreground">
      <title>{activity ? `${Math.max(...values)} req/s peak, last 60 seconds` : 'No activity sample available'}</title>
      <line x1="0" y1="30" x2="150" y2="30" className="stroke-border" />
      {activity ? <polyline points={points} fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" vectorEffect="non-scaling-stroke" /> : null}
    </svg>
  )
})

function RequestRateArea({ activity, timestamp }: { activity?: RequestActivity; timestamp?: string }) {
  const [inspectedIndex, setInspectedIndex] = useState<number | null>(null)
  const values = samplesFor(activity)
  const peak = Math.max(1, ...values)
  const x = (index: number) => 36 + index * 552 / 59
  const y = (value: number) => 140 - value * 124 / peak
  const points = values.map((value, index) => `${x(index).toFixed(1)},${y(value).toFixed(1)}`).join(' ')
  const selected = inspectedIndex ?? 59
  const selectedTime = timestamp ? new Date(new Date(timestamp).getTime() + (selected - 59) * 1000) : null
  const timeLabel = selectedTime && Number.isFinite(selectedTime.getTime())
    ? selectedTime.toLocaleTimeString([], { hour12: false })
    : selected === 59 ? 'now' : `${selected - 59}s`

  return (
    <div className="space-y-2">
      <div
        role="slider" aria-label="Inspect request rate" aria-valuemin={0} aria-valuemax={59} aria-valuenow={selected}
        aria-valuetext={`${timeLabel}: ${values[selected]} requests per second`} tabIndex={activity ? 0 : -1}
        className="relative rounded-sm focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-ring"
        onFocus={() => setInspectedIndex(59)} onBlur={() => setInspectedIndex(null)}
        onPointerMove={(event) => {
          const bounds = event.currentTarget.getBoundingClientRect()
          if (bounds.width > 0) setInspectedIndex(Math.max(0, Math.min(59, Math.round(((event.clientX - bounds.left) / bounds.width * 600 - 36) / 552 * 59))))
        }}
        onPointerLeave={() => setInspectedIndex(null)}
        onKeyDown={(event) => {
          if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return
          event.preventDefault()
          setInspectedIndex(event.key === 'Home' ? 0 : event.key === 'End' ? 59 : Math.max(0, Math.min(59, selected + (event.key === 'ArrowLeft' ? -1 : 1))))
        }}
      >
        <svg role="img" aria-label="Requests per second over the last 60 seconds" data-sample-count={activity ? 60 : 0} viewBox="0 0 600 170" preserveAspectRatio="none" className="h-40 w-full text-accent-foreground sm:h-44">
          <title>{activity ? `${Math.max(...values)} requests per second peak, last 60 seconds` : 'No activity sample available'}</title>
          {[0, peak / 2, peak].map((tick) => (
            <g key={tick}>
              <line x1="36" y1={y(tick)} x2="588" y2={y(tick)} className="stroke-border" strokeDasharray={tick === 0 ? undefined : '3 5'} />
              <text x="26" y={y(tick) + 4} textAnchor="end" className="fill-muted-foreground text-[10px]">{Number.isInteger(tick) ? tick : tick.toFixed(1)}</text>
            </g>
          ))}
          {activity ? <>
            <polygon data-testid="activity-area" points={`36,140 ${points} 588,140`} fill="currentColor" fillOpacity="0.1" />
            <polyline points={points} fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round" vectorEffect="non-scaling-stroke" />
            <circle data-testid="activity-latest-point" cx="588" cy={y(values[59])} r="4" fill="currentColor" />
            {inspectedIndex !== null ? <>
              <line x1={x(selected)} y1="10" x2={x(selected)} y2="140" stroke="currentColor" strokeOpacity="0.4" strokeDasharray="3 3" />
              <circle cx={x(selected)} cy={y(values[selected])} r="4" fill="currentColor" />
            </> : null}
          </> : null}
          <g className="fill-muted-foreground text-[10px]">
            <text x="36" y="163">-60s</text><text x="312" y="163" textAnchor="middle">-30s</text><text x="588" y="163" textAnchor="end">now</text>
          </g>
        </svg>
        {activity && inspectedIndex !== null ? <div role="tooltip" className="pointer-events-none absolute top-1 z-10 -translate-x-1/2 whitespace-nowrap rounded-md border border-border bg-popover px-2.5 py-1.5 text-xs tabular-nums text-popover-foreground shadow-md" style={{ left: `${Math.max(15, Math.min(85, x(selected) / 6))}%` }}>
          <span className="text-muted-foreground">{timeLabel}</span><span className="ml-2 font-semibold">{values[selected]} req/s</span>
        </div> : null}
      </div>
      <p className="text-right text-[10px] text-muted-foreground">{activity ? `${Math.max(...values)} req/s peak` : 'Requests / s'}</p>
    </div>
  )
}

export const ActivityIndicator = memo(function ActivityIndicator({
  activity, isStale = false, compact = false, label = 'Request activity', capacity, timestamp,
}: ActivityIndicatorProps) {
  const active = activity?.active_requests ?? 0
  const queued = activity?.queued_requests ?? 0
  const limit = capacity != null && capacity > 0 ? capacity : null
  const scale = Math.max(1, limit ?? 0, active + queued)
  return (
    <div role="group" aria-label={label} data-testid="request-activity" data-state={activity ? (active + queued > 0 ? 'busy' : 'idle') : 'unavailable'} data-active-requests={activity ? active : undefined} data-queued-requests={activity ? queued : undefined} data-stale={isStale} className={cn('space-y-2', compact ? 'min-w-52' : 'w-full')}>
      {compact ? activity ? <>
        <div className="flex items-center gap-3">
          <div className="flex h-2 w-[150px] shrink-0 overflow-hidden rounded-full bg-muted" aria-hidden="true">
            <div data-testid="activity-active-bar" className={cn('h-full bg-accent-foreground transition-[width] duration-300 motion-reduce:transition-none', active > 0 && !isStale ? 'motion-safe:animate-pulse' : '')} style={{ width: `${active / scale * 100}%` }} />
            <div data-testid="activity-queued-bar" className="activity-queue-stripes h-full bg-warning transition-[width] duration-300 motion-reduce:transition-none" style={{ width: `${queued / scale * 100}%` }} />
          </div>
          <span className="whitespace-nowrap text-xs tabular-nums" title={limit ? 'Active / concurrency limit' : 'No configured concurrency limit'}>{active} / {limit ?? '∞'}</span>
        </div>
        <div className="flex items-center justify-between gap-2 text-[11px] text-muted-foreground">
          <span>{active} active</span>{queued > 0 ? <span className="font-medium text-foreground">+{queued} 待ち</span> : <span>{queued} queued</span>}
        </div>
      </> : <span className="text-xs text-muted-foreground">Waiting for activity</span> : <>
        <div className="flex gap-3 text-xs tabular-nums text-muted-foreground">{activity ? <><span>{active} active</span><span>{queued} queued</span>{active + queued === 0 ? <span>Idle</span> : null}</> : <span>Waiting for activity</span>}</div>
        <RequestRateArea activity={activity} timestamp={timestamp} />
      </>}
      {isStale ? <span className="block text-xs text-muted-foreground">Last known activity</span> : null}
    </div>
  )
})
