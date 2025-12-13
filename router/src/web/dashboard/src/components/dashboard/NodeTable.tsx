import { useState, useMemo } from 'react'
import { type DashboardNode } from '@/lib/api'
import { formatUptime, formatPercentage, formatRelativeTime, cn } from '@/lib/utils'
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
import {
  Search,
  ChevronUp,
  ChevronDown,
  Server,
  Info,
  ChevronLeft,
  ChevronRight,
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
  const [statusFilter, setStatusFilter] = useState<'all' | 'online' | 'offline'>('all')
  const [sortField, setSortField] = useState<SortField>('status')
  const [sortDirection, setSortDirection] = useState<SortDirection>('desc')
  const [currentPage, setCurrentPage] = useState(1)
  const [selectedNode, setSelectedNode] = useState<DashboardNode | null>(null)

  const filteredAndSortedNodes = useMemo(() => {
    let result = [...nodes]

    // Filter by search
    if (search) {
      const searchLower = search.toLowerCase()
      result = result.filter(
        (node) =>
          node.machine_name.toLowerCase().includes(searchLower) ||
          node.custom_name?.toLowerCase().includes(searchLower) ||
          node.ip_address.toLowerCase().includes(searchLower) ||
          node.node_id.toLowerCase().includes(searchLower)
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

  const SortIcon = ({ field }: { field: SortField }) => {
    if (sortField !== field) return null
    return sortDirection === 'asc' ? (
      <ChevronUp className="h-4 w-4" />
    ) : (
      <ChevronDown className="h-4 w-4" />
    )
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
                  setStatusFilter(value as 'all' | 'online' | 'offline')
                  setCurrentPage(1)
                }}
              >
                <SelectTrigger className="w-full sm:w-32">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All</SelectItem>
                  <SelectItem value="online">Online</SelectItem>
                  <SelectItem value="offline">Offline</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        </CardHeader>

        <CardContent className="px-0">
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
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
              <TableBody>
                {paginatedNodes.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={8} className="h-32 text-center">
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
                          variant={node.status === 'online' ? 'online' : 'offline'}
                          className="gap-1"
                        >
                          <span
                            className={cn(
                              'h-1.5 w-1.5 rounded-full',
                              node.status === 'online'
                                ? 'bg-success animate-pulse'
                                : 'bg-destructive'
                            )}
                          />
                          {node.status}
                        </Badge>
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
                  variant="outline"
                  size="sm"
                  onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
                  disabled={currentPage === 1}
                >
                  <ChevronLeft className="h-4 w-4" />
                </Button>
                <span className="text-sm">
                  Page {currentPage} of {totalPages}
                </span>
                <Button
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
