// API Client for LLM Router Dashboard

const API_BASE = ''

interface FetchOptions extends RequestInit {
  params?: Record<string, string | number | boolean | undefined>
}

class ApiError extends Error {
  constructor(
    public status: number,
    public statusText: string,
    message?: string
  ) {
    super(message || `${status} ${statusText}`)
    this.name = 'ApiError'
  }
}

function getToken(): string | null {
  return localStorage.getItem('jwt_token')
}

function setToken(token: string): void {
  localStorage.setItem('jwt_token', token)
}

function removeToken(): void {
  localStorage.removeItem('jwt_token')
}

function isAuthenticated(): boolean {
  return !!getToken()
}

async function fetchWithAuth<T>(
  endpoint: string,
  options: FetchOptions = {}
): Promise<T> {
  const { params, ...fetchOptions } = options

  let url = `${API_BASE}${endpoint}`
  if (params) {
    const searchParams = new URLSearchParams()
    Object.entries(params).forEach(([key, value]) => {
      if (value !== undefined) {
        searchParams.append(key, String(value))
      }
    })
    const queryString = searchParams.toString()
    if (queryString) {
      url += `?${queryString}`
    }
  }

  const token = getToken()
  const headers: HeadersInit = {
    'Content-Type': 'application/json',
    ...options.headers,
  }

  if (token) {
    headers['Authorization'] = `Bearer ${token}`
  }

  const response = await fetch(url, {
    ...fetchOptions,
    headers,
  })

  if (response.status === 401) {
    removeToken()
    window.location.href = '/dashboard/login.html'
    throw new ApiError(401, 'Unauthorized')
  }

  if (!response.ok) {
    const errorText = await response.text()
    throw new ApiError(response.status, response.statusText, errorText)
  }

  // Handle empty responses
  const text = await response.text()
  if (!text) {
    return {} as T
  }

  return JSON.parse(text)
}

// Auth API
export const authApi = {
  login: async (username: string, password: string) => {
    const response = await fetch(`${API_BASE}/api/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
    })

    if (!response.ok) {
      throw new ApiError(response.status, response.statusText)
    }

    const data = await response.json()
    setToken(data.token)
    return data
  },

  logout: async () => {
    try {
      await fetchWithAuth('/api/auth/logout', { method: 'POST' })
    } finally {
      removeToken()
    }
  },

  me: () => fetchWithAuth<{ id: number; username: string; role: string }>('/api/auth/me'),
}

// Dashboard API
export interface DashboardStats {
  total_nodes: number
  online_nodes: number
  offline_nodes: number
  total_requests: number
  successful_requests: number
  failed_requests: number
  average_response_time_ms: number
  average_gpu_usage: number
  average_gpu_memory_usage: number
}

export interface DashboardNode {
  node_id: string
  machine_name: string
  custom_name?: string
  ip_address: string
  port: number
  status: 'online' | 'offline'
  runtime_version: string
  gpu_model?: string
  gpu_count?: number
  cpu_usage?: number
  memory_usage?: number
  gpu_usage?: number
  gpu_memory_usage?: number
  gpu_memory_total_mb?: number
  gpu_memory_used_mb?: number
  total_requests: number
  uptime_seconds: number
  last_seen: string
  tags?: string[]
  notes?: string
  ready_models?: string[]
}

export interface RequestHistoryItem {
  request_id: string
  timestamp: string
  model: string
  node_id?: string
  node_name?: string
  status: 'success' | 'error'
  duration_ms: number
  total_tokens?: number
  input_tokens?: number
  output_tokens?: number
  error?: string
  request_body?: unknown
  response_body?: unknown
}

export interface DashboardOverview {
  nodes: DashboardNode[]
  stats: DashboardStats
  history: RequestHistoryItem[]
  generated_at: string
  generation_time_ms: number
}

// Internal type for API response
interface ApiDashboardOverview {
  nodes: Array<{
    id: string
    machine_name: string
    custom_name?: string
    ip_address: string
    runtime_port: number
    status: 'online' | 'offline'
    runtime_version: string
    gpu_model?: string
    gpu_count?: number
    cpu_usage?: number
    memory_usage?: number
    gpu_usage?: number | null
    gpu_memory_usage?: number | null
    total_requests: number
    uptime_seconds: number
    last_seen: string
    tags?: string[]
    notes?: string
    loaded_models?: string[]
  }>
  stats: DashboardStats
  history: Array<{ minute: string; success: number; error: number }>
  generated_at: string
  generation_time_ms: number
}

export const dashboardApi = {
  getOverview: async (): Promise<DashboardOverview> => {
    const data = await fetchWithAuth<ApiDashboardOverview>('/api/dashboard/overview')

    // Map API nodes to frontend DashboardNode format
    const nodes: DashboardNode[] = (data.nodes || []).map((n) => ({
      node_id: n.id,
      machine_name: n.machine_name,
      custom_name: n.custom_name,
      ip_address: n.ip_address,
      port: n.runtime_port,
      status: n.status,
      runtime_version: n.runtime_version,
      gpu_model: n.gpu_model,
      gpu_count: n.gpu_count,
      cpu_usage: n.cpu_usage,
      memory_usage: n.memory_usage,
      gpu_usage: n.gpu_usage ?? undefined,
      gpu_memory_usage: n.gpu_memory_usage ?? undefined,
      total_requests: n.total_requests,
      uptime_seconds: n.uptime_seconds,
      last_seen: n.last_seen,
      tags: n.tags,
      notes: n.notes,
      ready_models: n.loaded_models,
    }))

    // Convert aggregated history to RequestHistoryItem format (empty for now - need different API)
    // The overview API returns aggregated stats, not individual requests
    const history: RequestHistoryItem[] = []

    return {
      nodes,
      stats: data.stats,
      history,
      generated_at: data.generated_at,
      generation_time_ms: data.generation_time_ms,
    }
  },

  getNodes: () => fetchWithAuth<DashboardNode[]>('/api/dashboard/nodes'),

  getStats: () => fetchWithAuth<DashboardStats>('/api/dashboard/stats'),

  getRequestHistory: async (limit?: number): Promise<RequestHistoryItem[]> => {
    interface ApiRequestRecord {
      id: string
      timestamp: string
      model: string
      node_id?: string
      agent_machine_name?: string
      status: { type: string; message?: string }
      duration_ms: number
      request_body?: unknown
      response_body?: unknown
    }
    interface ApiResponse {
      records: ApiRequestRecord[]
      total_count: number
      page: number
      per_page: number
    }
    const data = await fetchWithAuth<ApiResponse>('/api/dashboard/request-responses', {
      params: { per_page: limit || 100 },
    })
    return (data.records || []).map((r) => ({
      request_id: r.id,
      timestamp: r.timestamp,
      model: r.model,
      node_id: r.node_id,
      node_name: r.agent_machine_name,
      status: r.status.type === 'success' ? 'success' : 'error',
      duration_ms: r.duration_ms,
      error: r.status.type === 'error' ? r.status.message : undefined,
      request_body: r.request_body,
      response_body: r.response_body,
    } as RequestHistoryItem))
  },

  getNodeMetrics: (nodeId: string) =>
    fetchWithAuth<unknown[]>(`/api/dashboard/metrics/${nodeId}`),

  getRequestResponses: (params?: {
    limit?: number
    offset?: number
    model?: string
    status?: string
  }) => fetchWithAuth<unknown[]>('/api/dashboard/request-responses', { params }),

  getRequestResponseDetail: (id: string) =>
    fetchWithAuth<unknown>(`/api/dashboard/request-responses/${id}`),

  exportRequestResponses: (format: 'csv' | 'json') =>
    fetchWithAuth<Blob>('/api/dashboard/request-responses/export', {
      params: { format },
    }),

  getRouterLogs: async (params?: { limit?: number; level?: string; target?: string }) => {
    const data = await fetchWithAuth<{ entries: unknown[] }>('/api/dashboard/logs/router', { params })
    return data?.entries || []
  },

  getLogs: async (params?: { limit?: number; level?: string; target?: string }) => {
    // getLogs is an alias for getRouterLogs
    const data = await fetchWithAuth<{ entries: unknown[] }>('/api/dashboard/logs/router', { params })
    return data?.entries || []
  },
}

// Nodes API
export const nodesApi = {
  list: () => fetchWithAuth<DashboardNode[]>('/api/nodes'),

  delete: (nodeId: string) =>
    fetchWithAuth<void>(`/api/nodes/${nodeId}`, { method: 'DELETE' }),

  disconnect: (nodeId: string) =>
    fetchWithAuth<void>(`/api/nodes/${nodeId}/disconnect`, { method: 'POST' }),

  updateSettings: (
    nodeId: string,
    settings: { custom_name?: string; tags?: string[]; notes?: string }
  ) =>
    fetchWithAuth<void>(`/api/nodes/${nodeId}/settings`, {
      method: 'PUT',
      body: JSON.stringify(settings),
    }),

  getLogs: (nodeId: string, params?: { limit?: number; level?: string }) =>
    fetchWithAuth<unknown[]>(`/api/nodes/${nodeId}/logs`, { params }),
}

// Models API
export interface RegisteredModel {
  name: string
  filename: string
  repo: string
  size_bytes?: number
  state: 'registered' | 'downloading' | 'failed'
  progress?: number
  error?: string
}

export interface ConvertTask {
  id: string
  repo: string
  filename: string
  revision?: string
  quantization?: string
  chat_template?: string
  status: 'queued' | 'in_progress' | 'completed' | 'failed'
  progress: number
  error?: string
  path?: string
  created_at: string
  updated_at: string
}

export interface ModelInfo {
  name: string
  source?: string
  size_bytes?: number
  format?: string
  state: 'ready' | 'downloading' | 'converting' | 'pending' | 'error'
  progress?: number
  error?: string
}

export interface AvailableModel {
  id?: string
  provider?: string
  name: string
  url?: string
  downloads?: number
  likes?: number
}

// Response wrapper type for available models
interface AvailableModelsResponse {
  models: AvailableModel[]
  source: string
}

export const modelsApi = {
  getRegistered: async (): Promise<ModelInfo[]> => {
    const data = await fetchWithAuth<unknown[]>('/api/models/registered')
    // Map API response to ModelInfo format
    return (data || []).map((item: unknown) => {
      const m = item as Record<string, unknown>
      return {
        name: String(m.name || ''),
        source: String(m.source || ''),
        size_bytes: m.size_gb ? Number(m.size_gb) * 1024 * 1024 * 1024 : undefined,
        format: m.tags ? (m.tags as string[])[0] : undefined,
        state: m.ready ? 'ready' : (m.status === 'downloading' ? 'downloading' : 'pending'),
        progress: m.progress as number | undefined,
        error: m.error as string | undefined,
      } as ModelInfo
    })
  },

  getAvailable: async (): Promise<AvailableModel[]> => {
    const data = await fetchWithAuth<AvailableModelsResponse | AvailableModel[]>('/api/models/available')
    // Handle both object wrapper and direct array responses
    if (Array.isArray(data)) {
      return data
    }
    return data?.models || []
  },

  getLoaded: () => fetchWithAuth<unknown[]>('/api/models/loaded'),

  register: (urlOrName: string) => {
    // フル URL からリポジトリ名を抽出
    let repo = urlOrName.trim()
    const hfMatch = repo.match(/huggingface\.co\/([^/]+\/[^/]+)/)
    if (hfMatch) {
      repo = hfMatch[1]
    }
    return fetchWithAuth<{ task_id: string }>('/api/models/register', {
      method: 'POST',
      body: JSON.stringify({ repo }),
    })
  },

  pull: (modelName: string) =>
    fetchWithAuth<{ task_id: string }>('/api/models/pull', {
      method: 'POST',
      body: JSON.stringify({ model_name: modelName }),
    }),

  delete: (modelName: string) =>
    fetchWithAuth<void>(`/api/models/${encodeURIComponent(modelName)}`, {
      method: 'DELETE',
    }),

  distribute: (modelName: string, nodeIds: string[]) =>
    fetchWithAuth<{ task_ids: string[] }>('/api/models/distribute', {
      method: 'POST',
      body: JSON.stringify({ model_name: modelName, node_ids: nodeIds }),
    }),

  convert: (params: {
    repo: string
    filename: string
    revision?: string
    chat_template?: string
  }) =>
    fetchWithAuth<{ task_id: string }>('/api/models/convert', {
      method: 'POST',
      body: JSON.stringify(params),
    }),

  getConvertTasks: () => fetchWithAuth<ConvertTask[]>('/api/models/convert'),

  getConvertTask: (taskId: string) =>
    fetchWithAuth<ConvertTask>(`/api/models/convert/${taskId}`),

  deleteConvertTask: (taskId: string) =>
    fetchWithAuth<void>(`/api/models/convert/${taskId}`, { method: 'DELETE' }),

  getNodeModels: (nodeId: string) =>
    fetchWithAuth<unknown[]>(`/api/nodes/${nodeId}/models`),

  pullToNode: (nodeId: string, modelName: string) =>
    fetchWithAuth<{ task_id: string }>(`/api/nodes/${nodeId}/models/pull`, {
      method: 'POST',
      body: JSON.stringify({ model_name: modelName }),
    }),
}

// Tasks API
export interface TaskProgress {
  task_id: string
  status: 'pending' | 'running' | 'completed' | 'failed'
  progress: number
  message?: string
  error?: string
}

export const tasksApi = {
  list: () => fetchWithAuth<TaskProgress[]>('/api/tasks'),

  getProgress: (taskId: string) =>
    fetchWithAuth<TaskProgress>(`/api/tasks/${taskId}`),
}

// API Keys API
export interface ApiKey {
  id: string
  name: string
  key_prefix: string
  created_at: string
  expires_at?: string
  last_used_at?: string
}

export interface CreateApiKeyResponse {
  id: string
  name: string
  key: string // Full key only returned on creation
  key_prefix: string
  created_at: string
  expires_at?: string
}

export const apiKeysApi = {
  list: async (): Promise<ApiKey[]> => {
    const data = await fetchWithAuth<{ api_keys: ApiKey[] } | ApiKey[]>('/api/api-keys')
    if (Array.isArray(data)) {
      return data
    }
    return data?.api_keys || []
  },

  create: (data: { name: string; expires_at?: string }) =>
    fetchWithAuth<CreateApiKeyResponse>('/api/api-keys', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  update: (id: string, data: { name?: string; expires_at?: string | null }) =>
    fetchWithAuth<ApiKey>(`/api/api-keys/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    }),

  delete: (id: string) =>
    fetchWithAuth<void>(`/api/api-keys/${id}`, { method: 'DELETE' }),
}

// Users API
export interface User {
  id: string
  username: string
  role: 'admin' | 'user'
  created_at: string
}

export const usersApi = {
  list: async (): Promise<User[]> => {
    const data = await fetchWithAuth<{ users: User[] } | User[]>('/api/users')
    if (Array.isArray(data)) {
      return data
    }
    return data?.users || []
  },

  create: (data: { username: string; password: string; role: string }) =>
    fetchWithAuth<User>('/api/users', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  update: (
    id: string,
    data: { username?: string; password?: string; role?: string }
  ) =>
    fetchWithAuth<User>(`/api/users/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    }),

  delete: (id: string) =>
    fetchWithAuth<void>(`/api/users/${id}`, { method: 'DELETE' }),
}

// Chat API (OpenAI compatible)
export interface ChatMessage {
  role: 'system' | 'user' | 'assistant'
  content: string
}

export interface ChatSession {
  id: string
  title: string
  messages: ChatMessage[]
  model?: string
  created_at: string
  updated_at: string
}

export interface ChatCompletionRequest {
  model: string
  messages: ChatMessage[]
  stream?: boolean
  temperature?: number
  max_tokens?: number
}

export const chatApi = {
  complete: async (
    request: ChatCompletionRequest,
    apiKey?: string,
    onChunk?: (chunk: string) => void
  ) => {
    const headers: HeadersInit = {
      'Content-Type': 'application/json',
    }

    if (apiKey) {
      headers['Authorization'] = `Bearer ${apiKey}`
    }

    const response = await fetch(`${API_BASE}/v1/chat/completions`, {
      method: 'POST',
      headers,
      body: JSON.stringify(request),
    })

    if (!response.ok) {
      throw new ApiError(response.status, response.statusText)
    }

    if (request.stream && onChunk) {
      const reader = response.body?.getReader()
      if (!reader) throw new Error('No response body')

      const decoder = new TextDecoder()
      let buffer = ''

      while (true) {
        const { done, value } = await reader.read()
        if (done) break

        buffer += decoder.decode(value, { stream: true })
        const lines = buffer.split('\n')
        buffer = lines.pop() || ''

        for (const line of lines) {
          if (line.startsWith('data: ')) {
            const data = line.slice(6)
            if (data === '[DONE]') continue
            try {
              const parsed = JSON.parse(data)
              const content = parsed.choices?.[0]?.delta?.content
              if (content) {
                onChunk(content)
              }
            } catch {
              // Ignore parse errors
            }
          }
        }
      }

      return null
    }

    return response.json()
  },

  getModels: (apiKey?: string) => {
    const headers: HeadersInit = {}
    if (apiKey) {
      headers['Authorization'] = `Bearer ${apiKey}`
    }
    return fetch(`${API_BASE}/v1/models`, { headers }).then((r) => r.json())
  },

  // Session management (local storage based for now)
  getSessions: async (): Promise<ChatSession[]> => {
    const sessions = localStorage.getItem('chat_sessions')
    return sessions ? JSON.parse(sessions) : []
  },

  saveSessions: async (sessions: ChatSession[]): Promise<void> => {
    localStorage.setItem('chat_sessions', JSON.stringify(sessions))
  },
}

// Export utilities
export { ApiError, getToken, setToken, removeToken, isAuthenticated }
