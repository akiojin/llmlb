import {
  useModelsTableViewModel,
  type AggregatedModel,
  type SupportedApi,
  type SortField,
  type ModelEndpoint,
  type ModelTraffic,
} from '@/viewmodels/useModelsTableViewModel'
import { useModelEndpointStatsViewModel } from '@/viewmodels/useModelEndpointStatsViewModel'
import {
  type RegisteredModelView,
  type DashboardEndpoint,
  type ModelsView,
  type LifecycleStatus,
} from '@/lib/api'
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
  Plus,
  Trash2,
  Server,
} from 'lucide-react'
import { EmptyState } from '@/components/ui/empty-state'
import { ModelAddWizard } from './ModelAddWizard'
import { ModelDeleteDialog } from './ModelDeleteDialog'
import { ModelIdentity } from './ModelIdentity'

/**
 * SPEC-8795f98f: Models Tab
 */

interface ModelsTableProps {
  models: RegisteredModelView[]
  endpoints: DashboardEndpoint[]
  isLoading: boolean
  onRefresh?: () => void
  viewerMode?: boolean
  /** US-029: 表示モード（canonical 集約 / detail 全 variant） */
  view?: ModelsView
  /** US-029: 表示モード切替コールバック */
  onViewChange?: (view: ModelsView) => void
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

const SUPPORTED_API_BADGES: {
  key: SupportedApi
  icon: typeof MessageSquare
  label: string
}[] = [
  { key: 'chat_completions', icon: MessageSquare, label: 'Chat' },
  { key: 'completions', icon: FileText, label: 'Completion' },
  { key: 'responses', icon: MessageSquare, label: 'Responses' },
  { key: 'embeddings', icon: Layers, label: 'Embed' },
  { key: 'fine_tune', icon: Settings, label: 'Tune' },
  { key: 'inference', icon: Cpu, label: 'Infer' },
  { key: 'audio_speech', icon: Volume2, label: 'TTS' },
  { key: 'audio_transcription', icon: Mic, label: 'STT' },
  { key: 'image_input', icon: Image, label: 'Image' },
  { key: 'image_generation', icon: Image, label: 'Image Gen' },
]

interface ColumnDef {
  key: string
  label: string
  defaultVisible: boolean
  render: (model: AggregatedModel) => React.ReactNode
}

function SupportedApiBadges({ apis }: { apis: SupportedApi[] }) {
  const active = SUPPORTED_API_BADGES.filter((api) => apis.includes(api.key))
  if (active.length === 0) {
    return <span className="text-xs text-muted-foreground">Not reported</span>
  }
  return (
    <TooltipProvider>
      <div className="flex gap-1 flex-wrap">
        {active.map(({ key, icon: Icon, label }) => (
          <Tooltip key={key}>
            <TooltipTrigger asChild>
              <Badge variant="outline" className="gap-1 px-2 py-0.5">
                <Icon className="h-3 w-3" />
                <span>{label}</span>
              </Badge>
            </TooltipTrigger>
            <TooltipContent>{label}</TooltipContent>
          </Tooltip>
        ))}
      </div>
    </TooltipProvider>
  )
}

function TrafficCell({ stat }: { stat: ModelTraffic }) {
  if (stat.total === 0) {
    return <span className="text-sm tabular-nums text-muted-foreground">0</span>
  }
  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="text-sm tabular-nums">{stat.totalLabel}</span>
        </TooltipTrigger>
        <TooltipContent>
          <div className="text-xs space-y-0.5">
            <div className="text-green-400">{`OK: ${stat.successfulLabel}`}</div>
            <div className="text-red-400">{`Fail: ${stat.failedLabel}`}</div>
          </div>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}

function EndpointCountCell({ count, label }: { count: number; label: string }) {
  return (
    <div className="flex items-center gap-2">
      <Server className="h-3.5 w-3.5 text-muted-foreground" />
      <span className={count === 0 ? 'text-sm tabular-nums text-muted-foreground' : 'text-sm tabular-nums'}>
        {label}
      </span>
    </div>
  )
}

function EndpointStatsRow({
  endpoint,
  modelId,
  onDelete,
}: {
  endpoint: DashboardEndpoint
  modelId: string
  onDelete?: () => void
}) {
  const { totalRequests, successfulRequests, failedRequests, modelTpsSummary, playgroundHref } =
    useModelEndpointStatsViewModel(endpoint.id, modelId)

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
        <span>{`Total: ${totalRequests}`}</span>
        <span className="text-green-600">
          {`OK: ${successfulRequests}`}
        </span>
        <span className="text-red-600">
          {`Fail: ${failedRequests}`}
        </span>
        <span>{`TPS: ${modelTpsSummary}`}</span>
        <a
          href={playgroundHref}
          className="text-primary hover:underline"
        >
          <Play className="h-3 w-3" />
        </a>
        {onDelete && (
          <button
            onClick={(e) => {
              e.stopPropagation()
              onDelete()
            }}
            className="text-muted-foreground hover:text-destructive transition-colors"
            title="Delete model from endpoint"
          >
            <Trash2 className="h-3 w-3" />
          </button>
        )}
      </div>
    </div>
  )
}

export function ModelsTable({
  models,
  endpoints,
  isLoading,
  onRefresh,
  viewerMode = false,
  view,
  onViewChange,
}: ModelsTableProps) {
  // US-029: Canonical 表示 ⇔ 詳細表示のトグル（view/onViewChange 両指定時のみ表示）
  const viewToggle =
    view && onViewChange ? (
      <div className="inline-flex overflow-hidden rounded-md border" role="group" aria-label="Model view mode">
        <Button
          variant={view === 'canonical' ? 'secondary' : 'ghost'}
          size="sm"
          className="rounded-none"
          aria-pressed={view === 'canonical'}
          onClick={() => onViewChange('canonical')}
        >
          Canonical
        </Button>
        <Button
          variant={view === 'detail' ? 'secondary' : 'ghost'}
          size="sm"
          className="rounded-none"
          aria-pressed={view === 'detail'}
          onClick={() => onViewChange('detail')}
        >
          Detail
        </Button>
      </div>
    ) : null

  const {
    search, setSearch, statusFilter, setStatusFilter, capabilityFilters, setCapabilityFilter,
    sortField, sortDirection, handleSort, expandedModels, toggleExpand, addWizardOpen, setAddWizardOpen,
    deleteDialog, openDeleteDialog, setDeleteDialogOpen, columnVisibility, setColumnVisible,
    aggregatedWithStatsFallback, getModelTraffic, modelEndpoints, activeApiFilters, sorted, viewerFiltered,
    openPlayground,
  } = useModelsTableViewModel({ models, endpoints, viewerMode })

  const columns: ColumnDef[] = [
      {
        key: 'id',
        label: 'Model ID',
        defaultVisible: true,
        render: (m) => (
          <ModelIdentity
            id={m.id}
            canonicalName={m.canonicalName}
            aliases={m.aliases}
            isCanonical={m.isCanonical}
          />
        ),
      },
      {
        key: 'bestStatus',
        label: 'Status',
        defaultVisible: true,
        render: (m) => (
          <div className="flex items-center gap-1.5">
            <span
              className={`inline-block h-2 w-2 rounded-full shrink-0 ${m.ready ? 'bg-green-500' : 'bg-gray-300'}`}
              title={m.ready ? 'Ready' : 'Not Ready'}
            />
            <Badge variant={getLifecycleBadgeVariant(m.bestStatus)}>
              {m.lifecycleLabel}
            </Badge>
          </div>
        ),
      },
      {
        key: 'endpointCount',
        label: 'Endpoints',
        defaultVisible: true,
        render: (m) => <EndpointCountCell count={m.endpointCount} label={m.endpointCountLabel} />,
      },
      {
        key: 'totalRequests',
        label: 'Routed Requests',
        defaultVisible: true,
        render: (m) => <TrafficCell stat={getModelTraffic(m.id)} />,
      },
      {
        key: 'supportedApis',
        label: 'APIs',
        defaultVisible: true,
        render: (m) => <SupportedApiBadges apis={m.supportedApis} />,
      },
      {
        key: 'maxTokens',
        label: 'Max Tokens',
        defaultVisible: false,
        render: (m) => (
          <span className="text-sm">
            {m.maxTokensLabel}
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
            {m.requiredMemoryLabel}
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
  ]

  const visibleColumns = columns.filter((col) => columnVisibility[col.key])

  const SortIcon = ({ field }: { field: SortField }) => {
    if (sortField !== field) return null
    return sortDirection === 'asc' ? (
      <ChevronUp className="ml-1 h-4 w-4 inline" />
    ) : (
      <ChevronDown className="ml-1 h-4 w-4 inline" />
    )
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

  if (viewerMode) {
    return (
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle className="flex items-center gap-2">
              <Package className="h-5 w-5" />
              Models
              <Badge variant="secondary" className="ml-2">
                {aggregatedWithStatsFallback.length}
              </Badge>
            </CardTitle>
            <div className="flex items-center gap-2">
              {viewToggle}
              {onRefresh && (
                <Button variant="outline" size="sm" onClick={onRefresh}>
                  <RefreshCw className="h-4 w-4 mr-1" />
                  Refresh
                </Button>
              )}
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <div className="relative mb-4">
            <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 text-muted-foreground h-4 w-4" />
            <Input
              placeholder="Search by model ID..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="pl-10"
            />
          </div>
          <div className="rounded-md border">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Model ID</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Description</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {viewerFiltered.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={3} className="p-0">
                      <EmptyState
                        icon={<Package className="h-10 w-10" />}
                        title={search ? 'No models match your search' : 'No models registered'}
                      />
                    </TableCell>
                  </TableRow>
                ) : (
                  viewerFiltered.map((model) => (
                    <TableRow key={model.id}>
                      <TableCell>
                        <ModelIdentity
                          id={model.id}
                          canonicalName={model.canonicalName}
                          aliases={model.aliases}
                          isCanonical={model.isCanonical}
                        />
                      </TableCell>
                      <TableCell>
                        <div className="flex items-center gap-1.5">
                          <span
                            className={`inline-block h-2 w-2 rounded-full shrink-0 ${
                              model.ready ? 'bg-green-500' : 'bg-gray-300'
                            }`}
                            title={model.ready ? 'Ready' : 'Not Ready'}
                          />
                          <Badge variant={getLifecycleBadgeVariant(model.bestStatus)}>
                            {model.lifecycleLabel}
                          </Badge>
                        </div>
                      </TableCell>
                      <TableCell className="text-sm text-muted-foreground">
                        <span
                          className="truncate inline-block max-w-[640px] align-bottom"
                          title={model.description ?? ''}
                        >
                          {model.description ?? '-'}
                        </span>
                      </TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
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
              {aggregatedWithStatsFallback.length}
            </Badge>
          </CardTitle>
          <div className="flex items-center gap-2">
            {viewToggle}
            <Button variant="outline" size="sm" onClick={() => setAddWizardOpen(true)}>
              <Plus className="h-4 w-4 mr-1" />
              Add Model
            </Button>
            {onRefresh && (
              <Button variant="outline" size="sm" onClick={onRefresh}>
                <RefreshCw className="h-4 w-4 mr-1" />
                Refresh
              </Button>
            )}
          </div>
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
            onValueChange={setStatusFilter}
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
                APIs
                {activeApiFilters.length > 0 && (
                  <Badge variant="secondary" className="ml-1 text-xs">
                    {activeApiFilters.length}
                  </Badge>
                )}
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              {SUPPORTED_API_BADGES.map(({ key, label }) => (
                <DropdownMenuCheckboxItem
                  key={key}
                  checked={!!capabilityFilters[key]}
                  onCheckedChange={(checked) =>
                    setCapabilityFilter(key, !!checked)
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
                    setColumnVisible(col.key, !!checked)
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
                  const sortable: SortField[] = [
                    'id',
                    'bestStatus',
                    'endpointCount',
                    'totalRequests',
                  ]
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
                  <TableCell colSpan={visibleColumns.length + 2} className="p-0">
                    <EmptyState
                      icon={<Package className="h-10 w-10" />}
                      title={
                        search || statusFilter !== 'all' || activeApiFilters.length > 0
                          ? 'No models match the filter criteria'
                          : 'No models registered'
                      }
                      description={
                        search || statusFilter !== 'all' || activeApiFilters.length > 0
                          ? undefined
                          : 'Register a model or connect an endpoint that serves models.'
                      }
                    />
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
                      modelEndpoints={modelEndpoints.get(model.id) ?? []}
                      onDeleteModel={(endpoint) => openDeleteDialog(model.id, endpoint)}
                      onOpenPlayground={() => openPlayground(model.id)}
                    />
                  )
                })
              )}
            </TableBody>
          </Table>
        </div>

        <ModelAddWizard open={addWizardOpen} onOpenChange={setAddWizardOpen} />
        <ModelDeleteDialog
          open={deleteDialog.open}
          onOpenChange={setDeleteDialogOpen}
          modelId={deleteDialog.modelId}
          endpointId={deleteDialog.endpointId}
          endpointName={deleteDialog.endpointName}
          endpointType={deleteDialog.endpointType}
        />
      </CardContent>
    </Card>
  )
}

function ModelRow({
  model,
  visibleColumns,
  isExpanded,
  onToggleExpand,
  modelEndpoints,
  onDeleteModel,
  onOpenPlayground,
}: {
  model: AggregatedModel
  visibleColumns: ColumnDef[]
  isExpanded: boolean
  onToggleExpand: () => void
  modelEndpoints: ModelEndpoint[]
  onDeleteModel: (endpoint: ModelEndpoint) => void
  onOpenPlayground: () => void
}) {
  return (
    <>
      <TableRow className="cursor-pointer hover:bg-muted/50" onClick={onToggleExpand}>
        <TableCell className="w-8 px-2">
          <Button
            variant="secondary"
            size="icon"
            aria-label={isExpanded ? 'Collapse row' : 'Expand row'}
            aria-expanded={isExpanded}
            className="h-6 w-6 bg-transparent shadow-none hover:bg-muted/70"
          >
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
                  variant="secondary"
                  size="icon"
                  aria-label="Open in playground"
                  className="h-7 w-7 bg-transparent shadow-none hover:bg-muted/70"
                  disabled={!model.ready}
                  onClick={(e) => {
                    e.stopPropagation()
                    onOpenPlayground()
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
                {model.endpointSourcesLabel}
              </div>
              <div className="space-y-1 rounded-md border bg-background">
                {modelEndpoints.length > 0 ? (
                  modelEndpoints.map((ep) => (
                    <EndpointStatsRow
                      key={ep.id}
                      endpoint={ep}
                      modelId={model.id}
                      onDelete={
                        ep.canDeleteModel
                          ? () => onDeleteModel(ep)
                          : undefined
                      }
                    />
                  ))
                ) : (
                  <div className="py-2 px-3 text-xs text-muted-foreground">
                    No endpoints currently serve this model
                  </div>
                )}
              </div>
            </div>
          </TableCell>
        </TableRow>
      )}
    </>
  )
}
