/** Resource names shared by the subscription API (SPEC #821 FR-007). */
export const DASHBOARD_RESOURCES = ['endpoints', 'metrics', 'tps', 'system'] as const

export type DashboardResource = (typeof DASHBOARD_RESOURCES)[number]

export interface DashboardChange {
  changed: DashboardResource
  id?: string
}
