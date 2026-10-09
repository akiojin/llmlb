import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { endpointsApi, type EndpointDailyStatEntry } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'

export interface EndpointRequestChartViewModel {
  days: '7' | '30' | '90'
  setDays: (value: string) => void
  chartData: { date: string; successful: number; failed: number }[]
  isLoading: boolean
}

type DaysPeriod = '7' | '30' | '90'

function formatDateLabel(dateStr: string): string {
  // YYYY-MM-DD -> MM/DD
  const parts = dateStr.split('-')
  if (parts.length === 3) {
    return `${parts[1]}/${parts[2]}`
  }
  return dateStr
}

export function useEndpointRequestChartViewModel(endpointId: string): EndpointRequestChartViewModel {
  const [days, setDays] = useState<DaysPeriod>('7')

  const { data, isLoading } = useQuery<EndpointDailyStatEntry[]>({
    queryKey: queryKeys.endpointDailyStats(endpointId, days),
    queryFn: () => endpointsApi.getDailyStats(endpointId, Number(days)),
    enabled: !!endpointId,
  })

  const chartData = (data ?? []).map((entry) => ({
    date: formatDateLabel(entry.date),
    successful: entry.successful_requests,
    failed: entry.failed_requests,
  }))

  return { days, setDays: (value) => setDays(value as DaysPeriod), chartData, isLoading }
}
