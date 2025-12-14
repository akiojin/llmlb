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

export interface LogEntry {
  timestamp: string
  level: string
  message?: string
  target?: string
  fields?: Record<string, unknown>
}

export interface LogResponse {
  source: string
  entries: LogEntry[]
  path?: string
}

export const dashboardApi = {
  getOverview: () => fetchWithAuth<DashboardOverview>('/api/dashboard/overview'),

  getNodes: () => fetchWithAuth<DashboardNode[]>('/api/dashboard/nodes'),

  getStats: () => fetchWithAuth<DashboardStats>('/api/dashboard/stats'),

  getRequestHistory: (limit?: number) =>
    fetchWithAuth<RequestHistoryItem[]>('/api/dashboard/request-history', {
      params: { limit },
    }),

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

  exportRequestResponses: async (format: 'csv' | 'json') => {
    const token = getToken()
    const headers: HeadersInit = {}
    if (token) {
      headers['Authorization'] = `Bearer ${token}`
    }
    const response = await fetch(`${API_BASE}/api/dashboard/request-responses/export?format=${format}`, {
      headers,
    })
    if (!response.ok) {
      const errorText = await response.text()
      throw new ApiError(response.status, response.statusText, errorText)
    }
    return response.blob()
  },

  getRouterLogs: (params?: { limit?: number }) =>
    fetchWithAuth<LogResponse>('/api/dashboard/logs/router', { params }),
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

  getLogs: (nodeId: string, params?: { limit?: number }) =>
    fetchWithAuth<LogResponse>(`/api/nodes/${nodeId}/logs`, { params }),
}

// Models API
export interface RegisteredModelView {
  name: string
  source?: string
  description?: string
  status?: string
  ready: boolean
  path?: string
  download_url?: string
  repo?: string
  filename?: string
  size_gb?: number
  required_memory_gb?: number
  tags: string[]
}

export interface AvailableModelView {
  name: string
  display_name?: string
  description?: string
  tags?: string[]
  size_gb?: number
  required_memory_gb?: number
  repo?: string
  filename?: string
  download_url?: string
  quantization?: string
}

export interface AvailableModelsResponse {
  models: AvailableModelView[]
  source: string
  cached?: boolean
  pagination?: {
    limit: number
    offset: number
    total: number
  }
}

export interface ConvertTask {
  id: string
  repo: string
  filename: string
  revision?: string | null
  quantization?: string | null
  chat_template?: string | null
  status: 'queued' | 'in_progress' | 'completed' | 'failed'
  progress: number
  error?: string | null
  path?: string | null
  created_at: string
  updated_at: string
}

export const modelsApi = {
  getRegistered: () => fetchWithAuth<RegisteredModelView[]>('/api/models/registered'),

  getAvailable: () =>
    fetchWithAuth<AvailableModelsResponse>('/api/models/available', {
      params: { source: 'hf' },
    }),

  register: (repo: string, filename?: string) =>
    fetchWithAuth<unknown>('/api/models/register', {
      method: 'POST',
      body: JSON.stringify({ repo, filename }),
    }),

  delete: (modelName: string) =>
    fetchWithAuth<void>(`/api/models/${encodeURIComponent(modelName)}`, {
      method: 'DELETE',
    }),

  convert: (params: {
    repo: string
    filename: string
    revision?: string
    chat_template?: string
  }) =>
    fetchWithAuth<{ task_id: string; status: string }>('/api/models/convert', {
      method: 'POST',
      body: JSON.stringify(params),
    }),

  getConvertTasks: () => fetchWithAuth<ConvertTask[]>('/api/models/convert'),

  getConvertTask: (taskId: string) =>
    fetchWithAuth<ConvertTask>(`/api/models/convert/${taskId}`),

  deleteConvertTask: (taskId: string) =>
    fetchWithAuth<void>(`/api/models/convert/${taskId}`, { method: 'DELETE' }),
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
  list: () => fetchWithAuth<ApiKey[]>('/api/api-keys'),

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
  list: () => fetchWithAuth<User[]>('/api/users'),

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
