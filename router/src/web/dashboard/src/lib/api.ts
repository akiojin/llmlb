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

  exportRequestResponses: (format: 'csv' | 'json') =>
    fetchWithAuth<Blob>('/api/dashboard/request-responses/export', {
      params: { format },
    }),

  getCoordinatorLogs: (params?: { limit?: number; level?: string; target?: string }) =>
    fetchWithAuth<unknown[]>('/api/dashboard/logs/coordinator', { params }),

  getLogs: (params?: { limit?: number; level?: string; target?: string }) =>
    fetchWithAuth<unknown[]>('/api/dashboard/logs', { params }),
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

export interface ModelInfo {
  name: string
  source?: string
  size_bytes?: number
  format?: string
  state: 'ready' | 'downloading' | 'converting' | 'pending' | 'error'
  capabilities: ModelCapabilities
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

export const modelsApi = {
  getRegistered: () =>
    fetchWithAuth<ModelInfo[]>('/api/models/registered'),

  getAvailable: () => fetchWithAuth<AvailableModel[]>('/api/models/available'),

  getLoaded: () => fetchWithAuth<unknown[]>('/api/models/loaded'),

  register: (urlOrName: string) =>
    fetchWithAuth<{ task_id: string }>('/api/models/register', {
      method: 'POST',
      body: JSON.stringify({ url: urlOrName }),
    }),

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

  getConvertTasks: () => fetchWithAuth<unknown[]>('/api/models/convert'),

  getConvertTask: (taskId: string) =>
    fetchWithAuth<unknown>(`/api/models/convert/${taskId}`),

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
export type CapabilitySupport = 'supported' | 'unsupported' | 'unknown'

export interface ModelCapabilities {
  input_image: CapabilitySupport
  input_audio: CapabilitySupport
}

export interface AttachmentBase {
  kind: 'image' | 'audio'
  mime: string
  name?: string
  size_bytes: number
}

export interface ImageAttachment extends AttachmentBase {
  kind: 'image'
  // Used for preview and OpenAI "image_url.url" (data URL or URL)
  data_url: string
}

export interface AudioAttachment extends AttachmentBase {
  kind: 'audio'
  // OpenAI "input_audio.data" base64 (no data URL prefix)
  base64_data: string
  // e.g. "wav", "mp3"
  format: string
  // Used for preview (object URL or data URL)
  preview_url: string
}

export type ChatAttachment = ImageAttachment | AudioAttachment

export type ChatContentPart =
  | { type: 'text'; text: string }
  | { type: 'image_url'; image_url: { url: string } }
  | { type: 'input_audio'; input_audio: { data: string; format: string } }

export interface ChatMessage {
  role: 'system' | 'user' | 'assistant'
  content: string | ChatContentPart[]
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
