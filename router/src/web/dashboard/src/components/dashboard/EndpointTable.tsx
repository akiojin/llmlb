import { useState, useMemo } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { type DashboardEndpoint, endpointsApi } from '@/lib/api'
import { formatRelativeTime, cn } from '@/lib/utils'
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
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { EndpointDetailModal } from './EndpointDetailModal'
import {
  Search,
  ChevronUp,
  ChevronDown,
  Server,
  Info,
  ChevronLeft,
  ChevronRight,
  Play,
  RefreshCw,
  Trash2,
} from 'lucide-react'

/**
 * SPEC-66555000: ルーター主導エンドポイント登録システム
 * エンドポイント一覧表示コンポーネント
 */

interface EndpointTableProps {
  endpoints: DashboardEndpoint[]
  isLoading: boolean
}

type SortField = 'name' | 'status' | 'latency_ms' | 'model_count' | 'registered_at'
type SortDirection = 'asc' | 'desc'

const PAGE_SIZE = 10

function getStatusBadgeVariant(
  status: DashboardEndpoint['status']
): 'default' | 'destructive' | 'outline' | 'secondary' {
  switch (status) {
    case 'online':
      return 'default'
    case 'pending':
      return 'secondary'
    case 'offline':
      return 'outline'
    case 'error':
      return 'destructive'
    default:
      return 'outline'
  }
}

function getStatusLabel(status: DashboardEndpoint['status']): string {
  switch (status) {
    case 'online':
      return 'オンライン'
    case 'pending':
      return '確認中'
    case 'offline':
      return 'オフライン'
    case 'error':
      return 'エラー'
    default:
      return status
  }
}

export function EndpointTable({ endpoints, isLoading }: EndpointTableProps) {
  const queryClient = useQueryClient()
  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState<
    'all' | 'online' | 'pending' | 'offline' | 'error'
  >('all')
  const [sortField, setSortField] = useState<SortField>('status')
  const [sortDirection, setSortDirection] = useState<SortDirection>('desc')
  const [currentPage, setCurrentPage] = useState(1)
  const [selectedEndpoint, setSelectedEndpoint] = useState<DashboardEndpoint | null>(null)
  const [deletingEndpoint, setDeletingEndpoint] = useState<DashboardEndpoint | null>(null)
  const [isDeleting, setIsDeleting] = useState(false)
  const [isTesting, setIsTesting] = useState<string | null>(null)
  const [isSyncing, setIsSyncing] = useState<string | null>(null)

  const handleDelete = async () => {
    if (!deletingEndpoint) return
    setIsDeleting(true)
    try {
      await endpointsApi.delete(deletingEndpoint.id)
      await queryClient.invalidateQueries({ queryKey: ['dashboard-endpoints'] })
    } catch (error) {
      console.error('Failed to delete endpoint:', error)
    } finally {
      setIsDeleting(false)
      setDeletingEndpoint(null)
    }
  }

  const handleTest = async (endpoint: DashboardEndpoint) => {
    setIsTesting(endpoint.id)
    try {
      await endpointsApi.test(endpoint.id)
      await queryClient.invalidateQueries({ queryKey: ['dashboard-endpoints'] })
    } catch (error) {
      console.error('Failed to test endpoint:', error)
    } finally {
      setIsTesting(null)
    }
  }

  const handleSync = async (endpoint: DashboardEndpoint) => {
    setIsSyncing(endpoint.id)
    try {
      await endpointsApi.sync(endpoint.id)
      await queryClient.invalidateQueries({ queryKey: ['dashboard-endpoints'] })
    } catch (error) {
      console.error('Failed to sync endpoint:', error)
    } finally {
      setIsSyncing(null)
    }
  }

  const filteredEndpoints = useMemo(() => {
    return endpoints.filter((endpoint) => {
      const matchesSearch =
        endpoint.name.toLowerCase().includes(search.toLowerCase()) ||
        endpoint.base_url.toLowerCase().includes(search.toLowerCase())
      const matchesStatus = statusFilter === 'all' || endpoint.status === statusFilter
      return matchesSearch && matchesStatus
    })
  }, [endpoints, search, statusFilter])

  const sortedEndpoints = useMemo(() => {
    return [...filteredEndpoints].sort((a, b) => {
      let comparison = 0
      switch (sortField) {
        case 'name':
          comparison = a.name.localeCompare(b.name)
          break
        case 'status': {
          const statusOrder = { online: 0, pending: 1, offline: 2, error: 3 }
          comparison = statusOrder[a.status] - statusOrder[b.status]
          break
        }
        case 'latency_ms':
          comparison = (a.latency_ms ?? Infinity) - (b.latency_ms ?? Infinity)
          break
        case 'model_count':
          comparison = a.model_count - b.model_count
          break
        case 'registered_at':
          comparison = new Date(a.registered_at).getTime() - new Date(b.registered_at).getTime()
          break
      }
      return sortDirection === 'asc' ? comparison : -comparison
    })
  }, [filteredEndpoints, sortField, sortDirection])

  const paginatedEndpoints = useMemo(() => {
    const start = (currentPage - 1) * PAGE_SIZE
    return sortedEndpoints.slice(start, start + PAGE_SIZE)
  }, [sortedEndpoints, currentPage])

  const totalPages = Math.ceil(sortedEndpoints.length / PAGE_SIZE)

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDirection(sortDirection === 'asc' ? 'desc' : 'asc')
    } else {
      setSortField(field)
      setSortDirection('desc')
    }
  }

  const SortIcon = ({ field }: { field: SortField }) => {
    if (sortField !== field) return null
    return sortDirection === 'asc' ? (
      <ChevronUp className="ml-1 h-4 w-4 inline" />
    ) : (
      <ChevronDown className="ml-1 h-4 w-4 inline" />
    )
  }

  if (isLoading && endpoints.length === 0) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Server className="h-5 w-5" />
            エンドポイント一覧
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-center h-32">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary"></div>
          </div>
        </CardContent>
      </Card>
    )
  }

  return (
    <>
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle className="flex items-center gap-2">
              <Server className="h-5 w-5" />
              エンドポイント一覧
              <Badge variant="secondary" className="ml-2">
                {filteredEndpoints.length}
              </Badge>
            </CardTitle>
          </div>
        </CardHeader>
        <CardContent>
          {/* Filters */}
          <div className="flex flex-col sm:flex-row gap-4 mb-4">
            <div className="relative flex-1">
              <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 text-muted-foreground h-4 w-4" />
              <Input
                placeholder="名前またはURLで検索..."
                value={search}
                onChange={(e) => {
                  setSearch(e.target.value)
                  setCurrentPage(1)
                }}
                className="pl-10"
              />
            </div>
            <Select
              value={statusFilter}
              onValueChange={(value: typeof statusFilter) => {
                setStatusFilter(value)
                setCurrentPage(1)
              }}
            >
              <SelectTrigger className="w-[180px]">
                <SelectValue placeholder="ステータス" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">すべて</SelectItem>
                <SelectItem value="online">オンライン</SelectItem>
                <SelectItem value="pending">確認中</SelectItem>
                <SelectItem value="offline">オフライン</SelectItem>
                <SelectItem value="error">エラー</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {/* Table */}
          <div className="rounded-md border">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead
                    className="cursor-pointer hover:bg-muted/50"
                    onClick={() => handleSort('name')}
                  >
                    名前
                    <SortIcon field="name" />
                  </TableHead>
                  <TableHead>URL</TableHead>
                  <TableHead
                    className="cursor-pointer hover:bg-muted/50"
                    onClick={() => handleSort('status')}
                  >
                    ステータス
                    <SortIcon field="status" />
                  </TableHead>
                  <TableHead
                    className="cursor-pointer hover:bg-muted/50 text-right"
                    onClick={() => handleSort('latency_ms')}
                  >
                    レイテンシ
                    <SortIcon field="latency_ms" />
                  </TableHead>
                  <TableHead
                    className="cursor-pointer hover:bg-muted/50 text-right"
                    onClick={() => handleSort('model_count')}
                  >
                    モデル数
                    <SortIcon field="model_count" />
                  </TableHead>
                  <TableHead>最終確認</TableHead>
                  <TableHead className="text-right">操作</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {paginatedEndpoints.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={7} className="text-center py-8 text-muted-foreground">
                      {search || statusFilter !== 'all'
                        ? '条件に一致するエンドポイントがありません'
                        : 'エンドポイントが登録されていません'}
                    </TableCell>
                  </TableRow>
                ) : (
                  paginatedEndpoints.map((endpoint) => (
                    <TableRow key={endpoint.id}>
                      <TableCell className="font-medium">
                        <div className="flex items-center gap-2">
                          <span>{endpoint.name}</span>
                          {endpoint.supports_responses_api && (
                            <Badge variant="outline" className="text-xs">
                              Responses API
                            </Badge>
                          )}
                        </div>
                      </TableCell>
                      <TableCell>
                        <span className="text-muted-foreground font-mono text-sm">
                          {endpoint.base_url}
                        </span>
                      </TableCell>
                      <TableCell>
                        <Badge variant={getStatusBadgeVariant(endpoint.status)}>
                          {getStatusLabel(endpoint.status)}
                        </Badge>
                        {endpoint.last_error && (
                          <span className="ml-2 text-xs text-destructive">
                            ({endpoint.error_count}回)
                          </span>
                        )}
                      </TableCell>
                      <TableCell className="text-right">
                        {endpoint.latency_ms != null ? `${endpoint.latency_ms}ms` : '-'}
                      </TableCell>
                      <TableCell className="text-right">{endpoint.model_count}</TableCell>
                      <TableCell>
                        {endpoint.last_seen ? formatRelativeTime(endpoint.last_seen) : '-'}
                      </TableCell>
                      <TableCell className="text-right">
                        <div className="flex items-center justify-end gap-1">
                          <Button
                            variant="ghost"
                            size="icon"
                            onClick={() => setSelectedEndpoint(endpoint)}
                            title="詳細"
                          >
                            <Info className="h-4 w-4" />
                          </Button>
                          <Button
                            variant="ghost"
                            size="icon"
                            onClick={() => handleTest(endpoint)}
                            disabled={isTesting === endpoint.id}
                            title="接続テスト"
                          >
                            <Play
                              className={cn('h-4 w-4', isTesting === endpoint.id && 'animate-pulse')}
                            />
                          </Button>
                          <Button
                            variant="ghost"
                            size="icon"
                            onClick={() => handleSync(endpoint)}
                            disabled={isSyncing === endpoint.id || endpoint.status !== 'online'}
                            title="モデル同期"
                          >
                            <RefreshCw
                              className={cn('h-4 w-4', isSyncing === endpoint.id && 'animate-spin')}
                            />
                          </Button>
                          <Button
                            variant="ghost"
                            size="icon"
                            onClick={() => setDeletingEndpoint(endpoint)}
                            title="削除"
                          >
                            <Trash2 className="h-4 w-4 text-destructive" />
                          </Button>
                        </div>
                      </TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </div>

          {/* Pagination */}
          {totalPages > 1 && (
            <div className="flex items-center justify-between mt-4">
              <div className="text-sm text-muted-foreground">
                {sortedEndpoints.length} 件中 {(currentPage - 1) * PAGE_SIZE + 1} -{' '}
                {Math.min(currentPage * PAGE_SIZE, sortedEndpoints.length)} 件を表示
              </div>
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
                  {currentPage} / {totalPages}
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

      {/* Detail Modal */}
      {selectedEndpoint && (
        <EndpointDetailModal
          endpoint={selectedEndpoint}
          open={!!selectedEndpoint}
          onOpenChange={(open) => !open && setSelectedEndpoint(null)}
        />
      )}

      {/* Delete Confirmation Dialog */}
      <AlertDialog open={!!deletingEndpoint} onOpenChange={(open) => !open && setDeletingEndpoint(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>エンドポイントを削除しますか？</AlertDialogTitle>
            <AlertDialogDescription>
              「{deletingEndpoint?.name}」を削除します。この操作は取り消せません。
              このエンドポイントに関連するモデルも利用できなくなります。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={isDeleting}>キャンセル</AlertDialogCancel>
            <AlertDialogAction
              onClick={handleDelete}
              disabled={isDeleting}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            >
              {isDeleting ? '削除中...' : '削除'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}
