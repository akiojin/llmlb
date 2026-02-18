import { useQuery } from '@tanstack/react-query'
import { type ModelStatEntry, endpointsApi } from '@/lib/api'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Label } from '@/components/ui/label'
import { Loader2, BarChart3 } from 'lucide-react'

/**
 * SPEC-8c32349f: Model-level request statistics table
 * Displays per-model request breakdown for a given endpoint.
 */

interface EndpointModelStatsTableProps {
  endpointId: string
  enabled?: boolean
}

function formatSuccessRate(entry: ModelStatEntry): string {
  if (entry.total_requests === 0) return '-'
  const rate = (entry.successful_requests / entry.total_requests) * 100
  return `${rate.toFixed(1)}%`
}

export function EndpointModelStatsTable({ endpointId, enabled = true }: EndpointModelStatsTableProps) {
  const { data: modelStats, isLoading } = useQuery({
    queryKey: ['endpoint-model-stats', endpointId],
    queryFn: () => endpointsApi.getModelStats(endpointId),
    enabled,
  })

  return (
    <div className="space-y-3">
      <Label className="flex items-center gap-2">
        <BarChart3 className="h-4 w-4" />
        Requests by Model
      </Label>

      {isLoading ? (
        <div className="flex items-center justify-center py-4">
          <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
          <span className="ml-2 text-sm text-muted-foreground">Loading model stats...</span>
        </div>
      ) : !modelStats || modelStats.length === 0 ? (
        <p className="text-sm text-muted-foreground text-center py-4">
          No request data available
        </p>
      ) : (
        <div className="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Model ID</TableHead>
                <TableHead className="text-right">Total</TableHead>
                <TableHead className="text-right">Successful</TableHead>
                <TableHead className="text-right">Failed</TableHead>
                <TableHead className="text-right">Success Rate</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {modelStats.map((entry) => (
                <TableRow key={entry.model_id}>
                  <TableCell className="font-mono text-xs">{entry.model_id}</TableCell>
                  <TableCell className="text-right">
                    {entry.total_requests.toLocaleString()}
                  </TableCell>
                  <TableCell className="text-right">
                    {entry.successful_requests.toLocaleString()}
                  </TableCell>
                  <TableCell className="text-right">
                    {entry.failed_requests.toLocaleString()}
                  </TableCell>
                  <TableCell className="text-right">
                    {formatSuccessRate(entry)}
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
