import { cn } from '@/lib/utils'

export function ActivityConnectionStatus({ status = 'polling', isStale = false }: { status?: 'live' | 'polling'; isStale?: boolean }) {
  const live = status === 'live'
  return (
    <span role="status" data-testid="activity-connection-status" className={cn('inline-flex items-center gap-2 whitespace-nowrap rounded-full border px-3 py-1.5 text-[11px] font-medium', live ? 'border-success/25 bg-success/10 text-foreground' : 'border-warning/30 bg-warning/10 text-foreground')}>
      <span data-testid="activity-connection-dot" aria-hidden="true" className={cn('h-1.5 w-1.5 shrink-0 rounded-full', live ? 'bg-success motion-safe:animate-pulse' : 'bg-warning')} />
      {live ? 'Live · WebSocket' : 'Polling · 1s · WebSocket 未接続'}
      {isStale ? <span className="border-l border-warning/40 pl-2">Update delayed</span> : null}
    </span>
  )
}
