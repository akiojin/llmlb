import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Label } from '@/components/ui/label'
import { ModelIdentity } from './ModelIdentity'
import { Loader2, Grid3X3 } from 'lucide-react'
import { useEndpointModelsTableViewModel } from '@/viewmodels/useEndpointModelsTableViewModel'

/**
 * SPEC-8c32349f: Unified endpoint models table
 * Consolidates Models, Production Throughput by Model/API (TPS), and Requests by Model
 * into a single integrated display showing all relevant metrics per model.
 */

interface EndpointModelsTableProps {
  endpointId: string
  enabled?: boolean
  headerActions?: React.ReactNode
}

export function EndpointModelsTable({
  endpointId,
  enabled = true,
  headerActions,
}: EndpointModelsTableProps) {
  const { rows: consolidatedRows, isLoading, modelsLabel } = useEndpointModelsTableViewModel(endpointId, enabled)

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <Label className="flex items-center gap-2">
          <Grid3X3 className="h-4 w-4" />
          {modelsLabel}
        </Label>
        {headerActions && <div className="flex gap-2">{headerActions}</div>}
      </div>

      {isLoading ? (
        <div className="flex items-center justify-center py-4">
          <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
          <span className="ml-2 text-sm text-muted-foreground">Loading models...</span>
        </div>
      ) : consolidatedRows.length === 0 ? (
        <p className="text-sm text-muted-foreground text-center py-4">
          No models available
        </p>
      ) : (
        <div className="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Model</TableHead>
                <TableHead className="text-right">ctx</TableHead>
                <TableHead className="text-right">TPS</TableHead>
                <TableHead className="text-right">Requests</TableHead>
                <TableHead className="text-right">Success%</TableHead>
                <TableHead className="text-right">Avg Duration</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {consolidatedRows.map((entry) => (
                <TableRow key={entry.model_id}>
                  <TableCell>
                    <ModelIdentity
                      id={entry.model_id}
                      canonicalName={entry.canonical_name}
                      isCanonical={entry.canonical_name === entry.model_id}
                    />
                  </TableCell>
                  <TableCell className="text-right">
                    {entry.contextLabel}
                  </TableCell>
                  <TableCell className="text-right font-medium">
                    {entry.tpsLabel}
                  </TableCell>
                  <TableCell className="text-right">
                    {entry.requestsLabel}
                  </TableCell>
                  <TableCell className={`text-right ${entry.requestHealth === 'error' ? 'bg-red-100 text-red-900' : entry.requestHealth === 'warning' ? 'bg-yellow-100 text-yellow-900' : ''}`}>
                    {entry.successRateLabel}
                  </TableCell>
                  <TableCell className="text-right">
                    {entry.durationLabel}
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
