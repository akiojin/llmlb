import { hashKey, type QueryClient } from '@tanstack/react-query'
import type { DashboardChange, DashboardResource } from '@/lib/dashboardResources'
import type { DashboardQueryKey } from '@/lib/queryKeys'

export interface DashboardSubscriptionScope {
  id?: string
}

interface Subscription extends DashboardSubscriptionScope {
  resources: ReadonlySet<DashboardResource>
  queryKey: DashboardQueryKey
}

// Registrations follow their provider's lifetime and never cross cache clients.
const subscriptions = new WeakMap<QueryClient, Set<Subscription>>()

/** Register one owner; its cleanup removes only that owner's subscription. */
export function registerDashboardSubscription(
  queryClient: QueryClient,
  resources: readonly DashboardResource[],
  queryKey: DashboardQueryKey,
  { id }: DashboardSubscriptionScope = {},
): () => void {
  let entries = subscriptions.get(queryClient)
  if (!entries) {
    entries = new Set()
    subscriptions.set(queryClient, entries)
  }
  const subscription: Subscription = { resources: new Set(resources), queryKey, id }
  entries.add(subscription)
  return () => {
    if (!entries.delete(subscription)) return
    if (entries.size === 0) subscriptions.delete(queryClient)
  }
}

/**
 * SPEC #821 FR-004: id-less changes broadcast; scoped changes reach aggregate
 * registrations and the same explicit id. Query key structure never sets scope.
 * ViewModels own registrations; the WebSocket transport only dispatches changes.
 */
export function invalidateDashboardSubscriptions(queryClient: QueryClient, change: DashboardChange): void {
  const entries = subscriptions.get(queryClient)
  if (!entries) return

  const invalidated = new Set<string>()
  for (const subscription of entries) {
    if (!subscription.resources.has(change.changed)) continue
    if (change.id !== undefined && subscription.id !== undefined && subscription.id !== change.id) continue

    const hash = hashKey(subscription.queryKey)
    if (invalidated.has(hash)) continue
    invalidated.add(hash)
    // Preserve the existing WS path's React Query prefix invalidation contract.
    void queryClient.invalidateQueries({ queryKey: subscription.queryKey })
  }
}
