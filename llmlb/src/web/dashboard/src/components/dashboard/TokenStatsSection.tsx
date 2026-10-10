import {
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Skeleton } from '@/components/ui/skeleton'
import { EmptyState } from '@/components/ui/empty-state'
import { MessageSquare, TrendingUp, Calendar } from 'lucide-react'
import { useTokenStatsViewModel, type TokenChartDatum, type TokenStatsViewModel } from '@/viewmodels/useTokenStatsViewModel'

/** 入力/出力トークンを期間ごとに積み上げ表示するバーチャート。 */
function TokenBarChart({ data, formatChartNumber }: {
  data: TokenChartDatum[]
  formatChartNumber: TokenStatsViewModel['formatChartNumber']
}) {
  return (
    <div
      role="img"
      aria-label="Token Statistics"
      className="mb-4 h-64 w-full"
    >
      <ResponsiveContainer width="100%" height="100%">
        <BarChart data={data} margin={{ top: 4, right: 8, left: -8, bottom: 0 }}>
          <CartesianGrid strokeDasharray="3 3" className="stroke-border" vertical={false} />
          <XAxis
            dataKey="label"
            tick={{ fontSize: 11 }}
            className="fill-muted-foreground"
            tickLine={false}
            axisLine={false}
          />
          <YAxis
            tick={{ fontSize: 11 }}
            className="fill-muted-foreground"
            tickLine={false}
            axisLine={false}
            width={48}
            tickFormatter={(v) => formatChartNumber(Number(v))}
          />
          <Tooltip
            contentStyle={{
              backgroundColor: 'hsl(var(--popover))',
              border: '1px solid hsl(var(--border))',
              borderRadius: '6px',
              fontSize: '12px',
            }}
            labelStyle={{ color: 'hsl(var(--popover-foreground))' }}
            formatter={(value, name) => [
              formatChartNumber(Number(value ?? 0)),
              name === 'input' ? 'Input' : 'Output',
            ]}
          />
          <Legend
            formatter={(value) => (value === 'input' ? 'Input' : 'Output')}
            wrapperStyle={{ fontSize: '12px' }}
          />
          <Bar dataKey="input" stackId="tokens" fill="hsl(var(--chart-1))" radius={[0, 0, 0, 0]} />
          <Bar dataKey="output" stackId="tokens" fill="hsl(var(--chart-2))" radius={[4, 4, 0, 0]} />
        </BarChart>
      </ResponsiveContainer>
    </div>
  )
}

/** ローディング中のアクセシブルなスケルトン表示（スクリーンリーダーへ通知）。 */
function StatsLoading({ rows }: { rows: number }) {
  return (
    <div role="status" aria-label="Loading" aria-busy="true" className="space-y-2">
      {[...Array(rows)].map((_, i) => (
        <Skeleton key={i} className="h-10" />
      ))}
    </div>
  )
}

export function TokenStatsSection() {
  const {
    dailyRows, monthlyRows, dailyChart, monthlyChart,
    loadingDaily, loadingMonthly, formatChartNumber,
  } = useTokenStatsViewModel()

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <MessageSquare className="h-5 w-5" />
          Token Statistics
        </CardTitle>
      </CardHeader>
      <CardContent>
        <Tabs defaultValue="daily" className="space-y-4">
          <TabsList>
            <TabsTrigger value="daily" className="gap-2">
              <TrendingUp className="h-4 w-4" />
              Daily
            </TabsTrigger>
            <TabsTrigger value="monthly" className="gap-2">
              <Calendar className="h-4 w-4" />
              Monthly
            </TabsTrigger>
          </TabsList>

          <TabsContent value="daily">
            {loadingDaily ? (
              <StatsLoading rows={5} />
            ) : dailyRows.length > 0 ? (
              <div className="space-y-2">
                <TokenBarChart data={dailyChart} formatChartNumber={formatChartNumber} />
                <div className="grid grid-cols-5 gap-2 text-sm font-medium text-muted-foreground border-b pb-2">
                  <div>Date</div>
                  <div className="text-right">Requests</div>
                  <div className="text-right">Input</div>
                  <div className="text-right">Output</div>
                  <div className="text-right">Total</div>
                </div>
                {dailyRows.map((row) => (
                  <div key={row.label} className="grid grid-cols-5 gap-2 text-sm py-2 border-b border-border/50">
                    <div className="font-medium">{row.label}</div>
                    <div className="text-right">{row.requestsLabel}</div>
                    <div className="text-right text-muted-foreground">{row.inputTokensLabel}</div>
                    <div className="text-right text-muted-foreground">{row.outputTokensLabel}</div>
                    <div className="text-right font-medium">{row.totalTokensLabel}</div>
                  </div>
                ))}
              </div>
            ) : (
              <EmptyState
                icon={<MessageSquare className="h-10 w-10" />}
                title="No daily statistics available"
              />
            )}
          </TabsContent>

          <TabsContent value="monthly">
            {loadingMonthly ? (
              <StatsLoading rows={3} />
            ) : monthlyRows.length > 0 ? (
              <div className="space-y-2">
                <TokenBarChart data={monthlyChart} formatChartNumber={formatChartNumber} />
                <div className="grid grid-cols-5 gap-2 text-sm font-medium text-muted-foreground border-b pb-2">
                  <div>Month</div>
                  <div className="text-right">Requests</div>
                  <div className="text-right">Input</div>
                  <div className="text-right">Output</div>
                  <div className="text-right">Total</div>
                </div>
                {monthlyRows.map((row) => (
                  <div key={row.label} className="grid grid-cols-5 gap-2 text-sm py-2 border-b border-border/50">
                    <div className="font-medium">{row.label}</div>
                    <div className="text-right">{row.requestsLabel}</div>
                    <div className="text-right text-muted-foreground">{row.inputTokensLabel}</div>
                    <div className="text-right text-muted-foreground">{row.outputTokensLabel}</div>
                    <div className="text-right font-medium">{row.totalTokensLabel}</div>
                  </div>
                ))}
              </div>
            ) : (
              <EmptyState
                icon={<MessageSquare className="h-10 w-10" />}
                title="No monthly statistics available"
              />
            )}
          </TabsContent>
        </Tabs>
      </CardContent>
    </Card>
  )
}
