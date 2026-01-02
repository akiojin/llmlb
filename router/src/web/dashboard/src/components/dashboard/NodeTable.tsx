import { useState, useMemo } from 'react'
import { type DashboardNode } from '@/lib/api'
import { formatUptime, formatPercentage, formatRelativeTime, formatBytes, cn } from '@/lib/utils'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { NodeDetailModal } from './NodeDetailModal'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Search,
  ChevronUp,
  ChevronDown,
  Server,
  Info,
  ChevronLeft,
  ChevronRight,
  Download,
  FileJson,
  FileSpreadsheet,
} from 'lucide-react'

interface NodeTableProps {
  nodes: DashboardNode[]
  isLoading: boolean
}

type SortField = 'machine_name' | 'status' | 'uptime_seconds' | 'total_requests' | 'gpu_usage'
type SortDirection = 'asc' | 'desc'

const PAGE_SIZE = 10

export function NodeTable({ nodes, isLoading }: NodeTableProps) {
  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState<
    'all' | 'online' | 'pending' | 'registering' | 'offline'
  >('all')
  const [sortField, setSortField] = useState<SortField>('status')
  const [sortDirection, setSortDirection] = useState<SortDirection>('desc')
  const [currentPage, setCurrentPage] = useState(1)
  const [selectedNode, setSelectedNode] = useState<DashboardNode | null>(null)
  const [selectedNodes, setSelectedNodes] = useState<Set<string>>(new Set())

  const filteredAndSortedNodes = useMemo(() => {
    let result = [...nodes]

    // Filter by search
    if (search) {
      const searchLower = search.toLowerCase()
      result = result.filter(
        (node) =>
          node.machine_name?.toLowerCase().includes(searchLower) ||
          node.custom_name?.toLowerCase().includes(searchLower) ||
          node.ip_address?.toLowerCase().includes(searchLower) ||
          node.node_id?.toLowerCase().includes(searchLower)
      )
    }

    // Filter by status
    if (statusFilter !== 'all') {
      result = result.filter((node) => node.status === statusFilter)
    }

    // Sort
    result.sort((a, b) => {
      let aVal: string | number = a[sortField] as string | number
      let bVal: string | number = b[sortField] as string | number

      // Handle undefined values
      if (aVal === undefined) aVal = sortField === 'gpu_usage' ? -1 : ''
      if (bVal === undefined) bVal = sortField === 'gpu_usage' ? -1 : ''

      if (typeof aVal === 'string') {
        return sortDirection === 'asc'
          ? aVal.localeCompare(bVal as string)
          : (bVal as string).localeCompare(aVal)
      }

      return sortDirection === 'asc'
        ? (aVal as number) - (bVal as number)
        : (bVal as number) - (aVal as number)
    })

    return result
  }, [nodes, search, statusFilter, sortField, sortDirection])

  const totalPages = Math.ceil(filteredAndSortedNodes.length / PAGE_SIZE)
  const paginatedNodes = filteredAndSortedNodes.slice(
    (currentPage - 1) * PAGE_SIZE,
    currentPage * PAGE_SIZE
  )

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDirection((prev) => (prev === 'asc' ? 'desc' : 'asc'))
    } else {
      setSortField(field)
      setSortDirection('desc')
    }
  }

  const handleSelectAll = (checked: boolean) => {
    if (checked) {
      setSelectedNodes(new Set(filteredAndSortedNodes.map((n) => n.node_id)))
    } else {
      setSelectedNodes(new Set())
    }
  }

  const handleSelectNode = (nodeId: string, checked: boolean) => {
    const newSelected = new Set(selectedNodes)
    if (checked) {
      newSelected.add(nodeId)
    } else {
      newSelected.delete(nodeId)
    }
    setSelectedNodes(newSelected)
  }

  const exportToJson = () => {
    const dataToExport =
      selectedNodes.size > 0
        ? filteredAndSortedNodes.filter((n) => selectedNodes.has(n.node_id))
        : filteredAndSortedNodes
    const json = JSON.stringify(dataToExport, null, 2)
    const blob = new Blob([json], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = 'nodes.json'
    a.click()
    URL.revokeObjectURL(url)
  }

  const exportToCsv = () => {
    const dataToExport =
      selectedNodes.size > 0
        ? filteredAndSortedNodes.filter((n) => selectedNodes.has(n.node_id))
        : filteredAndSortedNodes
    const headers = ['node_id', 'machine_name', 'ip_address', 'port', 'status', 'gpu_model', 'gpu_usage', 'uptime_seconds', 'total_requests']
    const rows = dataToExport.map((node) =>
      headers.map((h) => {
        const value = node[h as keyof DashboardNode]
        return value !== undefined && value !== null ? String(value) : ''
      }).join(',')
    )
    const csv = [headers.join(','), ...rows].join('\n')
    const blob = new Blob([csv], { type: 'text/csv' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = 'nodes.csv'
    a.click()
    URL.revokeObjectURL(url)
  }

  const isAllSelected =
    filteredAndSortedNodes.length > 0 &&
    filteredAndSortedNodes.every((n) => selectedNodes.has(n.node_id))

  const SortIcon = ({ field }: { field: SortField }) => {
    if (sortField !== field) return null
    return sortDirection === 'asc' ? (
      <ChevronUp className="h-4 w-4" />
    ) : (
      <ChevronDown className="h-4 w-4" />
    )
  }

  const syncBadgeVariant = (state?: DashboardNode['sync_state']) => {
    switch (state) {
      case 'running':
        return 'warning'
      case 'success':
        return 'success'
      case 'failed':
        return 'destructive'
      case 'idle':
        return 'secondary'
      default:
        return 'outline'
    }
  }

  if (isLoading) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Server className="h-5 w-5" />
            Nodes
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            {[...Array(5)].map((_, i) => (
              <div key={i} className="h-12 shimmer rounded" />
            ))}
          </div>
        </CardContent>
      </Card>
    )
  }

  return (
    <>
      <Card>
        <CardHeader className="pb-4">
          <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
            <CardTitle className="flex items-center gap-2">
              <Server className="h-5 w-5" />
              Nodes
              <Badge variant="secondary" className="ml-2">
                {filteredAndSortedNodes.length}
              </Badge>
            </CardTitle>

            <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
              {/* Search */}
              <div className="relative">
                <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  id="filter-query"
                  placeholder="Search nodes..."
                  value={search}
                  onChange={(e) => {
                    setSearch(e.target.value)
                    setCurrentPage(1)
                  }}
                  className="pl-9 w-full sm:w-64"
                />
              </div>

              {/* Status Filter */}
              <Select
                value={statusFilter}
                onValueChange={(value) => {
                  setStatusFilter(
                    value as 'all' | 'online' | 'pending' | 'registering' | 'offline'
                  )
                  setCurrentPage(1)
                }}
              >
                <SelectTrigger id="filter-status" className="w-full sm:w-32">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All</SelectItem>
                  <SelectItem value="online">Online</SelectItem>
                  <SelectItem value="pending">Pending</SelectItem>
                  <SelectItem value="registering">Registering</SelectItem>
                  <SelectItem value="offline">Offline</SelectItem>
                </SelectContent>
              </Select>

              {/* Export Buttons */}
              <div className="flex gap-1">
                <Button
                  id="export-json"
                  variant="outline"
                  size="sm"
                  onClick={exportToJson}
                  title="Export to JSON"
                >
                  <FileJson className="h-4 w-4" />
                </Button>
                <Button
                  id="export-csv"
                  variant="outline"
                  size="sm"
                  onClick={exportToCsv}
                  title="Export to CSV"
                >
                  <FileSpreadsheet className="h-4 w-4" />
                </Button>
              </div>
            </div>
          </div>
        </CardHeader>

        <CardContent className="px-0">
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-12">
                    <Checkbox
                      id="select-all"
                      checked={isAllSelected}
                      onCheckedChange={handleSelectAll}
                      aria-label="Select all nodes"
                    />
                  </TableHead>
                  <TableHead
                    className="cursor-pointer hover:bg-muted/50"
                    onClick={() => handleSort('machine_name')}
                  >
                    <div className="flex items-center gap-1">
                      Name
                      <SortIcon field="machine_name" />
                    </div>
                  </TableHead>
                  <TableHead>IP Address</TableHead>
                  <TableHead
                    className="cursor-pointer hover:bg-muted/50"
                    onClick={() => handleSort('status')}
                  >
                    <div className="flex items-center gap-1">
                      Status
                      <SortIcon field="status" />
                    </div>
                  </TableHead>
                  <TableHead>Sync</TableHead>
                  <TableHead>GPU</TableHead>
                  <TableHead
                    className="cursor-pointer hover:bg-muted/50"
                    onClick={() => handleSort('gpu_usage')}
                  >
                    <div className="flex items-center gap-1">
                      GPU Usage
                      <SortIcon field="gpu_usage" />
                    </div>
                  </TableHead>
                  <TableHead
                    className="cursor-pointer hover:bg-muted/50"
                    onClick={() => handleSort('uptime_seconds')}
                  >
                    <div className="flex items-center gap-1">
                      Uptime
                      <SortIcon field="uptime_seconds" />
                    </div>
                  </TableHead>
                  <TableHead
                    className="cursor-pointer hover:bg-muted/50"
                    onClick={() => handleSort('total_requests')}
                  >
                    <div className="flex items-center gap-1">
                      Requests
                      <SortIcon field="total_requests" />
                    </div>
                  </TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody id="nodes-body">
                {paginatedNodes.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={10} className="h-32 text-center">
                      <div className="flex flex-col items-center gap-2 text-muted-foreground">
                        <Server className="h-8 w-8" />
                        <p>No nodes found</p>
                      </div>
                    </TableCell>
                  </TableRow>
                ) : (
                  paginatedNodes.map((node) => (
                    <TableRow
                      key={node.node_id}
                      className="cursor-pointer hover:bg-muted/50"
                      onClick={() => setSelectedNode(node)}
                    >
                      <TableCell onClick={(e) => e.stopPropagation()}>
                        <Checkbox
                          checked={selectedNodes.has(node.node_id)}
                          onCheckedChange={(checked) =>
                            handleSelectNode(node.node_id, checked === true)
                          }
                          aria-label={`Select ${node.machine_name}`}
                        />
                      </TableCell>
                      <TableCell>
                        <div>
                          <p className="font-medium">
                            {node.custom_name || node.machine_name}
                          </p>
                          {node.custom_name && (
                            <p className="text-xs text-muted-foreground">
                              {node.machine_name}
                            </p>
                          )}
                        </div>
                      </TableCell>
                      <TableCell className="font-mono text-sm">
                        {node.ip_address}:{node.port}
                      </TableCell>
                      <TableCell>
                        <Badge
                          variant={
                            node.status === 'online'
                              ? 'online'
                              : node.status === 'offline'
                              ? 'offline'
                              : 'pending'
                          }
                          className="gap-1"
                        >
                          <span
                            className={cn(
                              'h-1.5 w-1.5 rounded-full',
                              node.status === 'online'
                                ? 'bg-success animate-pulse'
                                : node.status === 'offline'
                                ? 'bg-destructive'
                                : 'bg-warning'
                            )}
                          />
                          {node.status}
                        </Badge>
                      </TableCell>
                      <TableCell>
                        {node.sync_state || node.sync_progress ? (
                          <div className="flex flex-col gap-1">
                            <Badge
                              variant={syncBadgeVariant(node.sync_state)}
                              className="w-fit capitalize"
                            >
                              {node.sync_state ?? 'running'}
                            </Badge>
                            {node.sync_progress ? (
                              <div className="text-xs text-muted-foreground">
                                {node.sync_progress.total_bytes > 0
                                  ? `${Math.round(
                                      (node.sync_progress.downloaded_bytes /
                                        node.sync_progress.total_bytes) *
                                        100
                                    )}%`
                                  : '—'}
                                {' · '}
                                {formatBytes(node.sync_progress.downloaded_bytes)} /{' '}
                                {formatBytes(node.sync_progress.total_bytes)}
                              </div>
                            ) : null}
                          </div>
                        ) : (
                          '—'
                        )}
                      </TableCell>
                      <TableCell>
                        <span className="text-sm">
                          {node.gpu_model || '—'}
                        </span>
                      </TableCell>
                      <TableCell>
                        {node.gpu_usage !== undefined ? (
                          <div className="flex items-center gap-2">
                            <div className="h-1.5 w-16 rounded-full bg-muted">
                              <div
                                className={cn(
                                  'h-full rounded-full transition-all',
                                  node.gpu_usage > 80
                                    ? 'bg-destructive'
                                    : node.gpu_usage > 50
                                    ? 'bg-warning'
                                    : 'bg-success'
                                )}
                                style={{ width: `${node.gpu_usage}%` }}
                              />
                            </div>
                            <span className="text-xs">
                              {formatPercentage(node.gpu_usage)}
                            </span>
                          </div>
                        ) : (
                          '—'
                        )}
                      </TableCell>
                      <TableCell>{formatUptime(node.uptime_seconds)}</TableCell>
                      <TableCell>{node.total_requests.toLocaleString()}</TableCell>
                      <TableCell className="text-right">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={(e) => {
                            e.stopPropagation()
                            setSelectedNode(node)
                          }}
                        >
                          <Info className="h-4 w-4" />
                        </Button>
                      </TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </div>

          {/* Pagination */}
          {totalPages > 1 && (
            <div className="flex items-center justify-between border-t px-6 py-4">
              <p className="text-sm text-muted-foreground">
                Showing {(currentPage - 1) * PAGE_SIZE + 1} to{' '}
                {Math.min(currentPage * PAGE_SIZE, filteredAndSortedNodes.length)}{' '}
                of {filteredAndSortedNodes.length}
              </p>
              <div className="flex items-center gap-2">
                <Button
                  id="page-prev"
                  variant="outline"
                  size="sm"
                  onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
                  disabled={currentPage === 1}
                >
                  <ChevronLeft className="h-4 w-4" />
                </Button>
                <span id="page-info" className="text-sm">
                  Page {currentPage} of {totalPages}
                </span>
                <Button
                  id="page-next"
                  variant="outline"
                  size="sm"
                  onClick={() => setCurrentPage((p) => Math.min(totalPages, p + 1))}
                  disabled={currentPage === totalPages}
                >
                  <ChevronRight className="h-4 w-4" />
                </Button>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Node Detail Modal */}
      <NodeDetailModal
        node={selectedNode}
        open={!!selectedNode}
        onOpenChange={(open) => !open && setSelectedNode(null)}
      />
    </>
  )
}
