import { useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import {
  type RegisteredModelView,
  type DashboardEndpoint,
  type LifecycleStatus,
  type AggregatedModel,
  type ModelCapabilities,
  aggregateModels,
  endpointsApi,
} from '@/lib/api'
import { formatBytes } from '@/lib/utils'
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
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import {
  Package,
  Search,
  RefreshCw,
  ChevronRight,
  ChevronDown,
  ChevronUp,
  MessageSquare,
  FileText,
  Layers,
  Settings,
  Cpu,
  Volume2,
  Mic,
  Image,
  Settings2,
  Play,
  Filter,
} from 'lucide-react'

/**
 * SPEC-8795f98f: Models Tab
 */

interface ModelsTableProps {
  models: RegisteredModelView[]
  endpoints: DashboardEndpoint[]
  isLoading: boolean
  onRefresh?: () => void
}

type SortField = 'id' | 'bestStatus' | 'sizeBytes' | 'ownedBy'
type SortDirection = 'asc' | 'desc'

const LIFECYCLE_PRIORITY: Record<LifecycleStatus, number> = {
  registered: 4,
  caching: 3,
  pending: 2,
  error: 1,
}

function getLifecycleBadgeVariant(
  status: LifecycleStatus
): 'online' | 'pending' | 'destructive' {
  switch (status) {
    case 'registered':
      return 'online'
    case 'caching':
    case 'pending':
      return 'pending'
    case 'error':
      return 'destructive'
  }
}

function getLifecycleLabel(status: LifecycleStatus): string {
  return status.charAt(0).toUpperCase() + status.slice(1)
}

const CAPABILITY_ICONS: {
  key: keyof ModelCapabilities
  icon: typeof MessageSquare
  label: string
}[] = [
  { key: 'chat_completion', icon: MessageSquare, label: 'Chat' },
  { key: 'completion', icon: FileText, label: 'Completion' },
  { key: 'embeddings', icon: Layers, label: 'Embed' },
  { key: 'fine_tune', icon: Settings, label: 'Tune' },
  { key: 'inference', icon: Cpu, label: 'Infer' },
  { key: 'text_to_speech', icon: Volume2, label: 'TTS' },
  { key: 'speech_to_text', icon: Mic, label: 'STT' },
  { key: 'image_generation', icon: Image, label: 'Image' },
]

interface ColumnDef {
  key: string
  label: string
  defaultVisible: boolean
  render: (model: AggregatedModel) => React.ReactNode
}

function CapabilityBadges({ capabilities }: { capabilities: ModelCapabilities }) {
  const active = CAPABILITY_ICONS.filter((c) => capabilities[c.key])
  if (active.length === 0) return <span className="text-muted-foreground text-xs">-</span>
  return (
    <TooltipProvider>
      <div className="flex gap-1 flex-wrap">
        {active.map(({ key, icon: Icon, label }) => (
          <Tooltip key={key}>
            <TooltipTrigger asChild>
              <Badge variant="outline" className="px-1.5 py-0.5">
                <Icon className="h-3 w-3" />
              </Badge>
            </TooltipTrigger>
            <TooltipContent>{label}</TooltipContent>
          </Tooltip>
        ))}
      </div>
    </TooltipProvider>
  )
}

function EndpointStatsRow({
  endpoint,
  modelId,
}: {
  endpoint: DashboardEndpoint
  modelId: string
}) {
  const { data: stats } = useQuery({
    queryKey: ['endpoint-model-stats', endpoint.id],
    queryFn: () => endpointsApi.getModelStats(endpoint.id),
  })

  const modelStat = stats?.find((s) => s.model_id === modelId)

  return (
    <div className="flex items-center justify-between py-1.5 px-3 text-sm">
      <div className="flex items-center gap-2 min-w-0">
        <Badge
          variant={endpoint.status === 'online' ? 'online' : endpoint.status === 'error' ? 'destructive' : 'pending'}
          className="text-xs"
        >
          {endpoint.status}
        </Badge>
        <span className="truncate font-medium">{endpoint.name}</span>
      </div>
      <div className="flex items-center gap-4 text-xs text-muted-foreground shrink-0">
        {modelStat ? (
          <>
            <span>Total: {modelStat.total_requests.toLocaleString()}</span>
            <span className="text-green-600">OK: {modelStat.successful_requests.toLocaleString()}</span>
            <span className="text-red-600">Fail: {modelStat.failed_requests.toLocaleString()}</span>
          </>
        ) : (
          <span>-</span>
        )}
        <a
          href={`#playground/${endpoint.id}`}
          className="text-primary hover:underline"
        >
          <Play className="h-3 w-3" />
        </a>
      </div>
    </div>
  )
}

export function ModelsTable({ models, endpoints, isLoading, onRefresh }: ModelsTableProps) {
  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState<LifecycleStatus | 'all'>('all')
  const [capabilityFilters, setCapabilityFilters] = useState<Record<string, boolean>>({})
  const [sortField, setSortField] = useState<SortField>('id')
  const [sortDirection, setSortDirection] = useState<SortDirection>('asc')
  const [expandedModels, setExpandedModels] = useState<Set<string>>(new Set())
  const [columnVisibility, setColumnVisibility] = useState<Record<string, boolean>>({
    id: true,
    bestStatus: true,
    ready: true,
    capabilities: true,
    sizeBytes: true,
    ownedBy: true,
    maxTokens: false,
    source: false,
    tags: false,
    description: false,
    repo: false,
    filename: false,
    requiredMemoryBytes: false,
    chatTemplate: false,
  })

  const aggregated = useMemo(() => aggregateModels(models), [models])

  const columns: ColumnDef[] = useMemo(
    () => [
      {
        key: 'id',
        label: 'Model ID',
        defaultVisible: true,
        render: (m) => (
          <span className="font-mono text-sm truncate" title={m.id}>
            {m.id}
          </span>
        ),
      },
      {
        key: 'bestStatus',
        label: 'Status',
        defaultVisible: true,
        render: (m) => (
          <Badge variant={getLifecycleBadgeVariant(m.bestStatus)}>
            {getLifecycleLabel(m.bestStatus)}
          </Badge>
        ),
      },
      {
        key: 'ready',
        label: 'Ready',
        defaultVisible: true,
        render: (m) => (
          <span
            className={`inline-block h-2.5 w-2.5 rounded-full ${m.ready ? 'bg-green-500' : 'bg-gray-300'}`}
            title={m.ready ? 'Ready' : 'Not Ready'}
          />
        ),
      },
      {
        key: 'capabilities',
        label: 'Capabilities',
        defaultVisible: true,
        render: (m) => <CapabilityBadges capabilities={m.capabilities} />,
      },
      {
        key: 'sizeBytes',
        label: 'Size',
        defaultVisible: true,
        render: (m) => (
          <span className="text-sm">{m.sizeBytes ? formatBytes(m.sizeBytes) : '-'}</span>
        ),
      },
      {
        key: 'ownedBy',
        label: 'Owned By',
        defaultVisible: true,
        render: (m) => <span className="text-sm">{m.ownedBy ?? '-'}</span>,
      },
      {
        key: 'maxTokens',
        label: 'Max Tokens',
        defaultVisible: false,
        render: (m) => (
          <span className="text-sm">
            {m.maxTokens != null ? m.maxTokens.toLocaleString() : '-'}
          </span>
        ),
      },
      {
        key: 'source',
        label: 'Source',
        defaultVisible: false,
        render: (m) => <span className="text-sm">{m.source ?? '-'}</span>,
      },
      {
        key: 'tags',
        label: 'Tags',
        defaultVisible: false,
        render: (m) =>
          m.tags.length > 0 ? (
            <div className="flex gap-1 flex-wrap">
              {m.tags.map((tag) => (
                <Badge key={tag} variant="secondary" className="text-xs">
                  {tag}
                </Badge>
              ))}
            </div>
          ) : (
            <span className="text-sm text-muted-foreground">-</span>
          ),
      },
      {
        key: 'description',
        label: 'Description',
        defaultVisible: false,
        render: (m) => (
          <span className="text-sm truncate max-w-[200px] inline-block" title={m.description}>
            {m.description ?? '-'}
          </span>
        ),
      },
      {
        key: 'repo',
        label: 'Repo',
        defaultVisible: false,
        render: (m) => <span className="text-sm">{m.repo ?? '-'}</span>,
      },
      {
        key: 'filename',
        label: 'Filename',
        defaultVisible: false,
        render: (m) => (
          <span className="text-sm font-mono">{m.filename ?? '-'}</span>
        ),
      },
      {
        key: 'requiredMemoryBytes',
        label: 'Required Memory',
        defaultVisible: false,
        render: (m) => (
          <span className="text-sm">
            {m.requiredMemoryBytes ? formatBytes(m.requiredMemoryBytes) : '-'}
          </span>
        ),
      },
      {
        key: 'chatTemplate',
        label: 'Chat Template',
        defaultVisible: false,
        render: (m) => (
          <span className="text-sm truncate max-w-[200px] inline-block" title={m.chatTemplate}>
            {m.chatTemplate ?? '-'}
          </span>
        ),
      },
    ],
    []
  )

  const visibleColumns = useMemo(
    () => columns.filter((col) => columnVisibility[col.key]),
    [columns, columnVisibility]
  )

  const activeCapFilters = useMemo(
    () => Object.entries(capabilityFilters).filter(([, v]) => v).map(([k]) => k),
    [capabilityFilters]
  )

  const filtered = useMemo(() => {
    return aggregated.filter((m) => {
      if (search && !m.id.toLowerCase().includes(search.toLowerCase())) return false
      if (statusFilter !== 'all' && m.bestStatus !== statusFilter) return false
      if (activeCapFilters.length > 0) {
        for (const cap of activeCapFilters) {
          if (!m.capabilities[cap as keyof ModelCapabilities]) return false
        }
      }
      return true
    })
  }, [aggregated, search, statusFilter, activeCapFilters])

  const sorted = useMemo(() => {
    return [...filtered].sort((a, b) => {
      let cmp = 0
      switch (sortField) {
        case 'id':
          cmp = a.id.localeCompare(b.id)
          break
        case 'bestStatus':
          cmp = LIFECYCLE_PRIORITY[a.bestStatus] - LIFECYCLE_PRIORITY[b.bestStatus]
          break
        case 'sizeBytes':
          cmp = (a.sizeBytes ?? 0) - (b.sizeBytes ?? 0)
          break
        case 'ownedBy':
          cmp = (a.ownedBy ?? '').localeCompare(b.ownedBy ?? '')
          break
      }
      return sortDirection === 'asc' ? cmp : -cmp
    })
  }, [filtered, sortField, sortDirection])

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDirection(sortDirection === 'asc' ? 'desc' : 'asc')
    } else {
      setSortField(field)
      setSortDirection('asc')
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

  const toggleExpand = (id: string) => {
    setExpandedModels((prev) => {
      const next = new Set(prev)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
      }
      return next
    })
  }

  if (isLoading && models.length === 0) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Package className="h-5 w-5" />
            Models
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-center h-32">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
          </div>
        </CardContent>
      </Card>
    )
  }

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between">
          <CardTitle className="flex items-center gap-2">
            <Package className="h-5 w-5" />
            Models
            <Badge variant="secondary" className="ml-2">
              {aggregated.length}
            </Badge>
          </CardTitle>
          {onRefresh && (
            <Button variant="outline" size="sm" onClick={onRefresh}>
              <RefreshCw className="h-4 w-4 mr-1" />
              Refresh
            </Button>
          )}
        </div>
      </CardHeader>
      <CardContent>
        {/* Filters */}
        <div className="flex flex-col sm:flex-row gap-4 mb-4">
          <div className="relative flex-1">
            <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 text-muted-foreground h-4 w-4" />
            <Input
              placeholder="Search by model ID..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="pl-10"
            />
          </div>
          <Select
            value={statusFilter}
            onValueChange={(v) => setStatusFilter(v as LifecycleStatus | 'all')}
          >
            <SelectTrigger className="w-[140px]">
              <SelectValue placeholder="Status" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All Status</SelectItem>
              <SelectItem value="registered">Registered</SelectItem>
              <SelectItem value="caching">Caching</SelectItem>
              <SelectItem value="pending">Pending</SelectItem>
              <SelectItem value="error">Error</SelectItem>
            </SelectContent>
          </Select>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" size="sm">
                <Filter className="h-4 w-4 mr-1" />
                Capabilities
                {activeCapFilters.length > 0 && (
                  <Badge variant="secondary" className="ml-1 text-xs">
                    {activeCapFilters.length}
                  </Badge>
                )}
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              {CAPABILITY_ICONS.map(({ key, label }) => (
                <DropdownMenuCheckboxItem
                  key={key}
                  checked={!!capabilityFilters[key]}
                  onCheckedChange={(checked) =>
                    setCapabilityFilters((prev) => ({ ...prev, [key]: !!checked }))
                  }
                >
                  {label}
                </DropdownMenuCheckboxItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" size="sm">
                <Settings2 className="h-4 w-4 mr-1" />
                Columns
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              {columns.map((col) => (
                <DropdownMenuCheckboxItem
                  key={col.key}
                  checked={!!columnVisibility[col.key]}
                  onCheckedChange={(checked) =>
                    setColumnVisibility((prev) => ({ ...prev, [col.key]: !!checked }))
                  }
                  disabled={col.key === 'id'}
                >
                  {col.label}
                </DropdownMenuCheckboxItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>

        {/* Table */}
        <div className="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-8" />
                {visibleColumns.map((col) => {
                  const sortable: SortField[] = ['id', 'bestStatus', 'sizeBytes', 'ownedBy']
                  const isSortable = sortable.includes(col.key as SortField)
                  return (
                    <TableHead
                      key={col.key}
                      className={isSortable ? 'cursor-pointer hover:bg-muted/50' : ''}
                      onClick={isSortable ? () => handleSort(col.key as SortField) : undefined}
                    >
                      {col.label}
                      {isSortable && <SortIcon field={col.key as SortField} />}
                    </TableHead>
                  )
                })}
                <TableHead className="w-10" />
              </TableRow>
            </TableHeader>
            <TableBody>
              {sorted.length === 0 ? (
                <TableRow>
                  <TableCell
                    colSpan={visibleColumns.length + 2}
                    className="text-center py-8 text-muted-foreground"
                  >
                    {search || statusFilter !== 'all' || activeCapFilters.length > 0
                      ? 'No models match the filter criteria'
                      : 'No models registered'}
                  </TableCell>
                </TableRow>
              ) : (
                sorted.map((model) => {
                  const isExpanded = expandedModels.has(model.id)
                  return (
                    <ModelRow
                      key={model.id}
                      model={model}
                      visibleColumns={visibleColumns}
                      isExpanded={isExpanded}
                      onToggleExpand={() => toggleExpand(model.id)}
                      endpoints={endpoints}
                    />
                  )
                })
              )}
            </TableBody>
          </Table>
        </div>
      </CardContent>
    </Card>
  )
}

function ModelRow({
  model,
  visibleColumns,
  isExpanded,
  onToggleExpand,
  endpoints,
}: {
  model: AggregatedModel
  visibleColumns: ColumnDef[]
  isExpanded: boolean
  onToggleExpand: () => void
  endpoints: DashboardEndpoint[]
}) {
  return (
    <>
      <TableRow className="cursor-pointer hover:bg-muted/50" onClick={onToggleExpand}>
        <TableCell className="w-8 px-2">
          <Button variant="ghost" size="icon" className="h-6 w-6">
            {isExpanded ? (
              <ChevronDown className="h-4 w-4" />
            ) : (
              <ChevronRight className="h-4 w-4" />
            )}
          </Button>
        </TableCell>
        {visibleColumns.map((col) => (
          <TableCell key={col.key}>{col.render(model)}</TableCell>
        ))}
        <TableCell className="w-10">
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7"
                  disabled={!model.ready}
                  onClick={(e) => {
                    e.stopPropagation()
                    window.location.hash = 'lb-playground?model=' + encodeURIComponent(model.id)
                  }}
                >
                  <Play className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Open in Playground</TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </TableCell>
      </TableRow>
      {isExpanded && (
        <TableRow>
          <TableCell colSpan={visibleColumns.length + 2} className="bg-muted/30 p-0">
            <div className="py-2 px-4">
              <div className="text-xs font-medium text-muted-foreground mb-2">
                Endpoints ({model.endpointCount} source{model.endpointCount !== 1 ? 's' : ''})
              </div>
              <div className="space-y-1 rounded-md border bg-background">
                {endpoints.map((ep) => (
                  <EndpointStatsRow
                    key={ep.id}
                    endpoint={ep}
                    modelId={model.id}
                  />
                ))}
              </div>
            </div>
          </TableCell>
        </TableRow>
      )}
    </>
  )
}
