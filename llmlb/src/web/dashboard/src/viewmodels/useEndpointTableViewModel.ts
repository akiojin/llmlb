import { useState, useMemo } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { endpointsApi, type DashboardEndpoint, type EndpointType } from '@/lib/api/endpoints'
import { queryKeys } from '@/lib/queryKeys'
import { useInvalidateOn } from '@/hooks/useInvalidateOn'
import { invalidateDashboardSubscriptions } from '@/hooks/dashboardSubscriptions'
import { classifyEndpointLastError } from '@/lib/endpoint-errors'
import { formatRelativeTime } from '@/lib/utils'
import { sortEndpoints, type EndpointSortDirection as SortDirection,
  type EndpointSortField as SortField } from '@/components/dashboard/endpointSorting'

const PAGE_SIZE = 10
type StatusFilter = 'all' | DashboardEndpoint['status']
interface CreateEndpointForm { name: string; base_url: string; api_key: string; notes: string }

export interface EndpointTableRow extends DashboardEndpoint {
  statusLabel: string
  typeLabel: string
  errorLabel: string | undefined
  errorCountLabel: string
  requestsLabel: string
  requestHealth: 'error' | 'warning' | 'normal'
  latencyLabel: string
  lastSeenLabel: string
}

export interface EndpointTableViewModel {
  search: string
  setSearch: (value: string) => void
  statusFilter: StatusFilter
  setStatusFilter: (value: StatusFilter) => void
  typeFilter: 'all' | EndpointType
  setTypeFilter: (value: 'all' | EndpointType) => void
  sortField: SortField
  sortDirection: SortDirection
  handleSort: (field: SortField) => void
  currentPage: number
  previousPage: () => void
  nextPage: () => void
  totalPages: number
  paginationLabel: string
  filteredCount: number
  paginatedEndpoints: EndpointTableRow[]
  selectedEndpoint: DashboardEndpoint | null
  setSelectedEndpoint: (endpoint: DashboardEndpoint | null) => void
  deletingEndpoint: DashboardEndpoint | null
  setDeletingEndpoint: (endpoint: DashboardEndpoint | null) => void
  isDeleting: boolean
  isTesting: string | null
  isSyncing: string | null
  handleCreate: () => Promise<void>
  handleDelete: () => Promise<void>
  handleTest: (endpoint: DashboardEndpoint) => Promise<void>
  handleSync: (endpoint: DashboardEndpoint) => Promise<void>
  isCreateDialogOpen: boolean
  setIsCreateDialogOpen: (open: boolean) => void
  onCreateDialogOpenChange: (open: boolean) => void
  isCreating: boolean
  createError: string | null
  createForm: CreateEndpointForm
  updateCreateForm: (field: keyof CreateEndpointForm, value: string) => void
}

function getStatusLabel(
  status: DashboardEndpoint['status']
): string {
  switch (status) {
    case 'online':
      return 'Online'
    case 'pending':
      return 'Pending'
    case 'offline':
      return 'Offline'
    case 'error':
      return 'Error'
    default:
      return status
  }
}

/** SPEC-e8e9326e: Get display label for endpoint type */
function getTypeLabel(
  type: EndpointType
): string {
  switch (type) {
    case 'xllm':
      return 'xLLM'
    case 'ollama':
      return 'Ollama'
    case 'vllm':
      return 'vLLM'
    case 'lm_studio':
      return 'LM Studio'
    case 'llamacpp':
      return 'llama.cpp'
    case 'openai_compatible':
      return 'OpenAI Compatible'
    case 'unknown':
      return 'Unknown'
    default:
      return type
  }
}


/** Endpoint list state, presentation values and commands; the parent owns acquisition. */
export function useEndpointTableViewModel(endpoints: DashboardEndpoint[]): EndpointTableViewModel {
  const queryClient = useQueryClient()
  useInvalidateOn(['endpoints'], queryKeys.dashboardEndpoints())
  // Test without a status transition and sync do not emit WS notifications.
  // Commands use the same declared subscriptions and await the refresh as before.
  const refreshEndpoints = (id: string) =>
    invalidateDashboardSubscriptions(queryClient, { changed: 'endpoints', id })
  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState<
    'all' | 'online' | 'pending' | 'offline' | 'error'
  >('all')
  const [typeFilter, setTypeFilter] = useState<'all' | EndpointType>('all')
  const [sortField, setSortField] = useState<SortField>('status')
  const [sortDirection, setSortDirection] = useState<SortDirection>('desc')
  const [currentPage, setCurrentPage] = useState(1)
  const [selectedEndpoint, setSelectedEndpoint] = useState<DashboardEndpoint | null>(null)
  const [deletingEndpoint, setDeletingEndpoint] = useState<DashboardEndpoint | null>(null)
  const [isDeleting, setIsDeleting] = useState(false)
  const [isTesting, setIsTesting] = useState<string | null>(null)
  const [isSyncing, setIsSyncing] = useState<string | null>(null)
  // Create endpoint state
  const [isCreateDialogOpen, setIsCreateDialogOpen] = useState(false)
  const [isCreating, setIsCreating] = useState(false)
  const [createError, setCreateError] = useState<string | null>(null)
  const [createForm, setCreateForm] = useState({
    name: '',
    base_url: '',
    api_key: '',
    notes: '',
  })

  const handleCreate = async () => {
    if (!createForm.name || !createForm.base_url) return
    setIsCreating(true)
    setCreateError(null)
    try {
      const created = await endpointsApi.create({
        name: createForm.name,
        base_url: createForm.base_url,
        api_key: createForm.api_key || undefined,
        notes: createForm.notes || undefined,
      })
      await refreshEndpoints(created.id)
      setIsCreateDialogOpen(false)
      setCreateForm({ name: '', base_url: '', api_key: '', notes: '' })
    } catch (error) {
      console.error('Failed to create endpoint:', error)
      setCreateError(error instanceof Error ? error.message : 'Failed to create endpoint')
    } finally {
      setIsCreating(false)
    }
  }

  const handleDelete = async () => {
    if (!deletingEndpoint) return
    setIsDeleting(true)
    try {
      await endpointsApi.delete(deletingEndpoint.id)
      await refreshEndpoints(deletingEndpoint.id)
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
      await refreshEndpoints(endpoint.id)
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
      await refreshEndpoints(endpoint.id)
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
      const matchesType = typeFilter === 'all' || endpoint.endpoint_type === typeFilter
      return matchesSearch && matchesStatus && matchesType
    })
  }, [endpoints, search, statusFilter, typeFilter])

  const sortedEndpoints = useMemo(() => {
    return sortEndpoints(
      filteredEndpoints,
      sortField,
      sortDirection
    )
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

  const rows = paginatedEndpoints.map((endpoint): EndpointTableRow => {
    const failureRate = endpoint.total_requests > 0
      ? endpoint.failed_requests / endpoint.total_requests : 0
    return {
      ...endpoint,
      statusLabel: getStatusLabel(endpoint.status),
      typeLabel: getTypeLabel(endpoint.endpoint_type),
      errorLabel: classifyEndpointLastError(endpoint.last_error)?.label,
      errorCountLabel: `(${endpoint.error_count} errors)`,
      requestsLabel: endpoint.total_requests > 0
        ? `${endpoint.total_requests.toLocaleString()} (${((endpoint.successful_requests / endpoint.total_requests) * 100).toFixed(1)}%)`
        : '-',
      requestHealth: failureRate >= 0.2 ? 'error' : failureRate >= 0.05 ? 'warning' : 'normal',
      latencyLabel: endpoint.latency_ms != null ? `${endpoint.latency_ms}ms` : '-',
      lastSeenLabel: endpoint.last_seen ? formatRelativeTime(endpoint.last_seen) : '-',
    }
  })

  return {
    search, setSearch: (value) => { setSearch(value); setCurrentPage(1) },
    statusFilter, setStatusFilter: (value) => { setStatusFilter(value); setCurrentPage(1) },
    typeFilter, setTypeFilter: (value) => { setTypeFilter(value); setCurrentPage(1) },
    sortField, sortDirection, handleSort, currentPage, totalPages,
    previousPage: () => setCurrentPage((page) => Math.max(1, page - 1)),
    nextPage: () => setCurrentPage((page) => Math.min(totalPages, page + 1)),
    paginationLabel: `Showing ${(currentPage - 1) * PAGE_SIZE + 1} - ${Math.min(currentPage * PAGE_SIZE, sortedEndpoints.length)} of ${sortedEndpoints.length}`,
    filteredCount: filteredEndpoints.length, paginatedEndpoints: rows,
    selectedEndpoint, setSelectedEndpoint, deletingEndpoint, setDeletingEndpoint,
    isDeleting, isTesting, isSyncing, handleCreate, handleDelete, handleTest, handleSync,
    isCreateDialogOpen, setIsCreateDialogOpen, isCreating, createError, createForm,
    updateCreateForm: (field, value) => setCreateForm((form) => ({ ...form, [field]: value })),
    onCreateDialogOpenChange: (open) => {
      if (!open) {
        setCreateError(null)
        setCreateForm({ name: '', base_url: '', api_key: '', notes: '' })
      }
      setIsCreateDialogOpen(open)
    },
  }
}
