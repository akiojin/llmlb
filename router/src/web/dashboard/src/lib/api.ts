// API Client for LLM Router Dashboard

const API_BASE = ''

interface FetchOptions extends RequestInit {
  params?: Record<string, string | number | boolean | undefined>
}

export class ApiError extends Error {
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
    const response = await fetch(`${API_BASE}/v0/auth/login`, {
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
      await fetchWithAuth('/v0/auth/logout', { method: 'POST' })
    } finally {
      removeToken()
    }
  },

  me: () =>
    fetchWithAuth<{ id: number; username: string; role: string }>('/v0/auth/me'),
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

// /v0/dashboard/request-responses APIのレスポンス型
export interface RequestResponseRecord {
  id: string
  timestamp: string
  request_type: string
  model: string
  node_id?: string
  node_machine_name?: string
  node_ip?: string
  request_body?: unknown
  response_body?: unknown
  duration_ms: number
  status: { type: 'success' } | { type: 'error'; message: string }
  completed_at?: string
}

export interface RequestResponsesPage {
  records: RequestResponseRecord[]
  total_count: number
  page: number
  per_page: number
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
  getOverview: () => fetchWithAuth<DashboardOverview>('/v0/dashboard/overview'),

  getNodes: () => fetchWithAuth<DashboardNode[]>('/v0/dashboard/nodes'),

  getStats: () => fetchWithAuth<DashboardStats>('/v0/dashboard/stats'),

  getRequestHistory: (limit?: number) =>
    fetchWithAuth<RequestHistoryItem[]>('/v0/dashboard/request-history', {
      params: { limit },
    }),

  getNodeMetrics: (nodeId: string) =>
    fetchWithAuth<unknown[]>(`/v0/dashboard/metrics/${nodeId}`),

  getRequestResponses: (params?: {
    limit?: number
    offset?: number
    model?: string
    status?: string
  }) => fetchWithAuth<RequestResponsesPage>('/v0/dashboard/request-responses', { params }),

  getRequestResponseDetail: (id: string) =>
    fetchWithAuth<unknown>(`/v0/dashboard/request-responses/${id}`),

  exportRequestResponses: async (format: 'csv' | 'json') => {
    const token = getToken()
    const headers: HeadersInit = {}
    if (token) {
      headers['Authorization'] = `Bearer ${token}`
    }
    const response = await fetch(
      `${API_BASE}/v0/dashboard/request-responses/export?format=${format}`,
      {
      headers,
      }
    )
    if (!response.ok) {
      const errorText = await response.text()
      throw new ApiError(response.status, response.statusText, errorText)
    }
    return response.blob()
  },

  getRouterLogs: (params?: { limit?: number }) =>
    fetchWithAuth<LogResponse>('/v0/dashboard/logs/router', { params }),
}

// Nodes API
export const nodesApi = {
  list: () => fetchWithAuth<DashboardNode[]>('/v0/nodes'),

  delete: (nodeId: string) =>
    fetchWithAuth<void>(`/v0/nodes/${nodeId}`, { method: 'DELETE' }),

  disconnect: (nodeId: string) =>
    fetchWithAuth<void>(`/v0/nodes/${nodeId}/disconnect`, { method: 'POST' }),

  updateSettings: (
    nodeId: string,
    settings: { custom_name?: string; tags?: string[]; notes?: string }
  ) =>
    fetchWithAuth<void>(`/v0/nodes/${nodeId}/settings`, {
      method: 'PUT',
      body: JSON.stringify(settings),
    }),

  getLogs: (nodeId: string, params?: { limit?: number }) =>
    fetchWithAuth<LogResponse>(`/v0/nodes/${nodeId}/logs`, { params }),
}

// Models API
export type LifecycleStatus = 'pending' | 'caching' | 'registered' | 'error'

export interface DownloadProgress {
  percent: number
  bytes_downloaded?: number
  bytes_total?: number
  error?: string
}

export interface RegisteredModelView {
  name: string
  source?: string
  description?: string
  status?: string
  lifecycle_status: LifecycleStatus
  download_progress?: DownloadProgress
  ready: boolean
  path?: string
  download_url?: string
  repo?: string
  filename?: string
  size_gb?: number
  required_memory_gb?: number
  tags: string[]
}

// NOTE: AvailableModelView, AvailableModelsResponse, ConvertTask は廃止
// HFカタログは直接 https://huggingface.co を参照
// ダウンロード状態は /v0/models の lifecycle_status で確認

export const modelsApi = {
  getRegistered: async (): Promise<RegisteredModelView[]> => {
    // /v0/models - 登録モデル一覧（lifecycle_status含む）
    // APIキー認証が必要なため、ローカルストレージのAPIキーを使用
    const apiKey = localStorage.getItem('playground_api_key') || 'sk_debug'
    const response = await fetch('/v0/models', {
      headers: {
        Authorization: `Bearer ${apiKey}`,
      },
    })
    if (!response.ok) {
      // エラー詳細を取得して適切なエラーをスロー
      const errorBody = await response.json().catch(() => ({ error: 'Unknown error' }))
      const message = errorBody.error || undefined
      throw new ApiError(response.status, response.statusText, message)
    }
    // 直接 RegisteredModelView[] を返す
    return (await response.json()) as RegisteredModelView[]
  },

  // NOTE: getAvailable は廃止 - HFカタログは直接 https://huggingface.co を参照

  register: (repo: string, filename?: string) =>
    fetchWithAuth<unknown>('/v0/models/register', {
      method: 'POST',
      body: JSON.stringify({ repo, filename }),
    }),

  delete: (modelName: string) =>
    fetchWithAuth<void>(`/v0/models/${encodeURIComponent(modelName)}`, {
      method: 'DELETE',
    }),

  // NOTE: convert, getConvertTasks, getConvertTask, deleteConvertTask は廃止
  // ダウンロード状態は getRegistered の lifecycle_status で確認 (Phase 2で実装)
}

// API Keys API
export interface ApiKey {
  id: string
  name: string
  key_prefix?: string
  created_by?: string
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
  list: () =>
    fetchWithAuth<{ api_keys: ApiKey[] }>('/v0/api-keys').then(
      (res) => res.api_keys
    ),

  create: (data: { name: string; expires_at?: string }) =>
    fetchWithAuth<CreateApiKeyResponse>('/v0/api-keys', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  update: (id: string, data: { name?: string; expires_at?: string | null }) =>
    fetchWithAuth<ApiKey>(`/v0/api-keys/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    }),

  delete: (id: string) =>
    fetchWithAuth<void>(`/v0/api-keys/${id}`, { method: 'DELETE' }),
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
    const res = await fetchWithAuth<{ users: User[] }>('/v0/users')
    return res.users
  },

  create: (data: { username: string; password: string; role: string }) =>
    fetchWithAuth<User>('/v0/users', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  update: (
    id: string,
    data: { username?: string; password?: string; role?: string }
  ) =>
    fetchWithAuth<User>(`/v0/users/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    }),

  delete: (id: string) =>
    fetchWithAuth<void>(`/v0/users/${id}`, { method: 'DELETE' }),
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
export { getToken, setToken, removeToken, isAuthenticated }
