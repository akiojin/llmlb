import { useQuery } from '@tanstack/react-query'
import { type ModelTpsEntry, endpointsApi } from '@/lib/api'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Label } from '@/components/ui/label'
import { Loader2, Zap } from 'lucide-react'

/**
 * SPEC-4bb5b55f: Model-level TPS (tokens per second) table
 * Displays per-model throughput metrics for a given endpoint.
 */

interface EndpointModelTpsTableProps {
  endpointId: string
  enabled?: boolean
}

function formatTps(tps: number | null): string {
  if (tps === null || tps === undefined) return '—'
  return `${tps.toFixed(1)} tok/s`
}

function formatDuration(ms: number | null): string {
  if (ms === null || ms === undefined) return '—'
  if (ms < 1000) return `${ms.toFixed(0)} ms`
  return `${(ms / 1000).toFixed(1)} s`
}

export function EndpointModelTpsTable({ endpointId, enabled = true }: EndpointModelTpsTableProps) {
  const { data: modelTps, isLoading } = useQuery({
    queryKey: ['endpoint-model-tps', endpointId],
    queryFn: () => endpointsApi.getModelTps(endpointId),
    enabled,
    refetchInterval: 10000,
  })

  return (
    <div className="space-y-3">
      <Label className="flex items-center gap-2">
        <Zap className="h-4 w-4" />
        Production Throughput by Model/API (TPS)
      </Label>

      {isLoading ? (
        <div className="flex items-center justify-center py-4">
          <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
          <span className="ml-2 text-sm text-muted-foreground">Loading TPS data...</span>
        </div>
      ) : !modelTps || modelTps.length === 0 ? (
        <p className="text-sm text-muted-foreground text-center py-4">
          No TPS data available yet
        </p>
      ) : (
        <div className="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Model</TableHead>
                <TableHead>API</TableHead>
                <TableHead className="text-right">TPS</TableHead>
                <TableHead className="text-right">Requests</TableHead>
                <TableHead className="text-right">Output Tokens</TableHead>
                <TableHead className="text-right">Avg Duration</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {modelTps.map((entry: ModelTpsEntry) => (
                <TableRow key={`${entry.model_id}-${entry.api_kind}`}>
                  <TableCell className="font-mono text-xs">{entry.model_id}</TableCell>
                  <TableCell className="text-xs">{entry.api_kind}</TableCell>
                  <TableCell className="text-right font-medium">
                    {formatTps(entry.tps)}
                  </TableCell>
                  <TableCell className="text-right">
                    {entry.request_count.toLocaleString()}
                  </TableCell>
                  <TableCell className="text-right">
                    {entry.total_output_tokens.toLocaleString()}
                  </TableCell>
                  <TableCell className="text-right">
                    {formatDuration(entry.average_duration_ms)}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}
    </div>
  )
}
