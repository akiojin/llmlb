import { useQuery } from '@tanstack/react-query'
import { dashboardApi, type DailyTokenStats, type MonthlyTokenStats, type TokenStats } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { formatNumber } from '@/lib/utils'

export interface TokenChartDatum {
  label: string
  input: number
  output: number
}

export interface TokenStatsRow {
  label: string
  requestsLabel: string
  inputTokensLabel: string
  outputTokensLabel: string
  totalTokensLabel: string
}

export interface TokenStatsViewModel {
  dailyRows: TokenStatsRow[]
  monthlyRows: TokenStatsRow[]
  dailyChart: TokenChartDatum[]
  monthlyChart: TokenChartDatum[]
  loadingDaily: boolean
  loadingMonthly: boolean
  formatChartNumber: (value: number) => string
}

function formatRow(label: string, stats: TokenStats): TokenStatsRow {
  return {
    label,
    requestsLabel: formatNumber(stats.request_count),
    inputTokensLabel: formatNumber(stats.total_input_tokens),
    outputTokensLabel: formatNumber(stats.total_output_tokens),
    totalTokensLabel: formatNumber(stats.total_tokens),
  }
}

/** Both periods retain provider polling independently of the active Radix tab. */
export function useTokenStatsViewModel(): TokenStatsViewModel {
  const { data: dailyStats, isLoading: loadingDaily } = useQuery<DailyTokenStats[]>({
    queryKey: queryKeys.tokenStatsDaily(),
    queryFn: () => dashboardApi.getDailyTokenStats(7),
  })
  const { data: monthlyStats, isLoading: loadingMonthly } = useQuery<MonthlyTokenStats[]>({
    queryKey: queryKeys.tokenStatsMonthly(),
    queryFn: () => dashboardApi.getMonthlyTokenStats(6),
  })

  return {
    dailyRows: (dailyStats ?? []).map((stats) => formatRow(stats.date, stats)),
    monthlyRows: (monthlyStats ?? []).map((stats) => formatRow(stats.month, stats)),
    dailyChart: (dailyStats ?? []).map((stats) => ({
      label: stats.date,
      input: stats.total_input_tokens,
      output: stats.total_output_tokens,
    })),
    monthlyChart: (monthlyStats ?? []).map((stats) => ({
      label: stats.month,
      input: stats.total_input_tokens,
      output: stats.total_output_tokens,
    })),
    loadingDaily,
    loadingMonthly,
    formatChartNumber: formatNumber,
  }
}
