import type { DashboardResource } from './dashboardResources'
import type { useInvalidateOn } from '@/hooks/useInvalidateOn'

// This file is compiled by dashboard-checks but never mounted or executed.
const valid: Parameters<typeof useInvalidateOn>[0] = ['endpoints', 'metrics', 'tps', 'system']
// @ts-expect-error resource spelling errors must fail the compiler
const invalid: Parameters<typeof useInvalidateOn>[0] = ['metric']
// @ts-expect-error the union cannot accept undeclared resources
const undeclared: DashboardResource = 'connected'
void [valid, invalid, undeclared]
