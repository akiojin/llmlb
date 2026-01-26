// API Client for LLM Load Balancer Dashboard

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
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(options.headers as Record<string, string>),
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
export interface RegisterRequest {
  invitation_code: string
  username: string
  password: string
}

export interface RegisterResponse {
  id: string
  username: string
  role: string
  created_at: string
}

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

  register: async (data: RegisterRequest): Promise<RegisterResponse> => {
    const response = await fetch(`${API_BASE}/v0/auth/register`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    })

    if (!response.ok) {
      const errorText = await response.text()
      throw new ApiError(response.status, response.statusText, errorText)
    }

    return response.json()
  },
}

// Dashboard API
export interface DashboardStats {
  total_nodes: number
  online_nodes: number
  pending_nodes: number
  registering_nodes: number
  offline_nodes: number
  total_requests: number
  successful_requests: number
  failed_requests: number
  total_active_requests: number
  queued_requests: number
  average_response_time_ms: number
  average_gpu_usage: number
  average_gpu_memory_usage: number
  // Token statistics
  total_input_tokens: number
  total_output_tokens: number
  total_tokens: number
}

export type SyncState = 'idle' | 'running' | 'success' | 'failed'

export interface SyncProgress {
  model_id: string
  file: string
  downloaded_bytes: number
  total_bytes: number
}

export interface DashboardNode {
  node_id: string
  machine_name: string
  custom_name?: string
  ip_address: string
  port: number
  status: 'online' | 'pending' | 'registering' | 'offline'
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
  sync_state?: SyncState
  sync_progress?: SyncProgress
  sync_updated_at?: string
  // Token statistics
  total_input_tokens: number
  total_output_tokens: number
  total_tokens: number
}

/**
 * SPEC-66555000: Router-Driven Endpoint Registration System
 * Dashboard display info for external inference services (Ollama, vLLM, xLLM, etc.)
 */
export type EndpointType = 'xllm' | 'ollama' | 'vllm' | 'openai_compatible' | 'unknown'

export interface DashboardEndpoint {
  id: string
  name: string
  base_url: string
  status: 'pending' | 'online' | 'offline' | 'error'
  endpoint_type: EndpointType
  health_check_interval_secs: number
  inference_timeout_secs: number
  latency_ms?: number
  last_seen?: string
  last_error?: string
  error_count: number
  registered_at: string
  notes?: string
  supports_responses_api: boolean
  model_count: number
}

/**
 * SPEC-66555000: Model download task for xLLM endpoints
 */
export interface DownloadTask {
  task_id: string
  model: string
  status: 'pending' | 'downloading' | 'completed' | 'failed' | 'cancelled'
  progress: number
  speed_mbps?: number
  eta_seconds?: number
  error?: string
  filename?: string
}

/**
 * SPEC-66555000: Model metadata from endpoint
 */
export interface ModelMetadata {
  model: string
  context_length?: number
  size_bytes?: number
  quantization?: string
  family?: string
  parameter_size?: string
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

// Token Statistics API types
export interface TokenStats {
  total_input_tokens: number
  total_output_tokens: number
  total_tokens: number
  request_count: number
}

export interface DailyTokenStats extends TokenStats {
  date: string
}

export interface MonthlyTokenStats extends TokenStats {
  month: string
}

export const dashboardApi = {
  getOverview: () => fetchWithAuth<DashboardOverview>('/v0/dashboard/overview'),

  getNodes: () => fetchWithAuth<DashboardNode[]>('/v0/dashboard/nodes'),

  /** SPEC-66555000: List endpoints */
  getEndpoints: () => fetchWithAuth<DashboardEndpoint[]>('/v0/dashboard/endpoints'),

  getStats: () => fetchWithAuth<DashboardStats>('/v0/dashboard/stats'),

  // Token statistics endpoints
  getTokenStats: () => fetchWithAuth<TokenStats>('/v0/dashboard/stats/tokens'),

  getDailyTokenStats: (days?: number) =>
    fetchWithAuth<DailyTokenStats[]>('/v0/dashboard/stats/tokens/daily', {
      params: { days },
    }),

  getMonthlyTokenStats: (months?: number) =>
    fetchWithAuth<MonthlyTokenStats[]>('/v0/dashboard/stats/tokens/monthly', {
      params: { months },
    }),

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
    fetchWithAuth<LogResponse>('/v0/dashboard/logs/lb', { params }),
}

/**
 * Endpoints API
 * SPEC-66555000: Router-Driven Endpoint Registration System
 * Management API for external inference services (Ollama, vLLM, xLLM, etc.)
 */
export const endpointsApi = {
  /** List endpoints for dashboard */
  list: () => fetchWithAuth<DashboardEndpoint[]>('/v0/dashboard/endpoints'),

  /** SPEC-66555000: List endpoints by type */
  listByType: (type: EndpointType) =>
    fetchWithAuth<DashboardEndpoint[]>('/v0/endpoints', {
      params: { type },
    }),

  /** Create endpoint */
  create: (data: {
    name: string
    base_url: string
    api_key?: string
    health_check_interval_secs?: number
    inference_timeout_secs?: number
    notes?: string
    endpoint_type?: EndpointType
  }) =>
    fetchWithAuth<DashboardEndpoint>('/v0/endpoints', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  /** Get endpoint details */
  get: (id: string) => fetchWithAuth<DashboardEndpoint>(`/v0/endpoints/${id}`),

  /** Update endpoint */
  update: (
    id: string,
    data: {
      name?: string
      base_url?: string
      api_key?: string
      health_check_interval_secs?: number
      inference_timeout_secs?: number
      notes?: string
      endpoint_type?: EndpointType
    }
  ) =>
    fetchWithAuth<DashboardEndpoint>(`/v0/endpoints/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    }),

  /** Delete endpoint */
  delete: (id: string) =>
    fetchWithAuth<void>(`/v0/endpoints/${id}`, { method: 'DELETE' }),

  /** Test connection */
  test: (id: string) =>
    fetchWithAuth<{ success: boolean; message?: string; latency_ms?: number }>(
      `/v0/endpoints/${id}/test`,
      { method: 'POST' }
    ),

  /** Sync models */
  sync: (id: string) =>
    fetchWithAuth<{ synced_models: number }>(`/v0/endpoints/${id}/sync`, {
      method: 'POST',
    }),

  /** Get endpoint models */
  getModels: (id: string) =>
    fetchWithAuth<{
      endpoint_id: string
      models: Array<{
        model_id: string
        capabilities?: string[]
        max_tokens?: number
        last_checked?: string
      }>
    }>(`/v0/endpoints/${id}/models`),

  /** SPEC-66555000: Download model (xLLM only) */
  downloadModel: (
    id: string,
    data: { model: string; filename?: string }
  ) =>
    fetchWithAuth<{ task_id: string }>(`/v0/endpoints/${id}/download`, {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  /** SPEC-66555000: Get download progress (xLLM only) */
  getDownloadProgress: (id: string) =>
    fetchWithAuth<{ tasks: DownloadTask[] }>(
      `/v0/endpoints/${id}/download/progress`
    ),

  /** SPEC-66555000: Get model metadata */
  getModelInfo: (id: string, model: string) =>
    fetchWithAuth<ModelMetadata>(
      `/v0/endpoints/${id}/models/${encodeURIComponent(model)}/info`
    ),

  /** Proxy chat completions to endpoint (JWT authenticated) */
  chatCompletions: async (
    id: string,
    request: {
      model: string
      messages: Array<{ role: string; content: string | Array<unknown> }>
      stream?: boolean
      temperature?: number
      max_tokens?: number
    },
    onChunk?: (chunk: string) => void
  ) => {
    const token = localStorage.getItem('jwt_token')
    const headers: HeadersInit = {
      'Content-Type': 'application/json',
    }
    if (token) {
      headers['Authorization'] = `Bearer ${token}`
    }

    const response = await fetch(`${API_BASE}/v0/endpoints/${id}/chat/completions`, {
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
}

// Models API
export type LifecycleStatus = 'pending' | 'caching' | 'registered' | 'error'

export interface DownloadProgress {
  percent: number
  bytes_downloaded?: number
  bytes_total?: number
  error?: string
}

// Azure OpenAI 形式の capabilities (boolean object)
export interface ModelCapabilities {
  chat_completion: boolean
  completion: boolean
  embeddings: boolean
  fine_tune: boolean
  inference: boolean
  text_to_speech: boolean
  speech_to_text: boolean
  image_generation: boolean
}

// /v1/models レスポンスの model object
export interface OpenAIModel {
  id: string
  object: 'model'
  created: number
  owned_by: string
  capabilities: ModelCapabilities
  // Dashboard extended fields
  lifecycle_status: LifecycleStatus
  download_progress?: DownloadProgress | null
  ready: boolean
  repo?: string | null
  filename?: string | null
  size_bytes?: number
  required_memory_bytes?: number
  source?: string
  tags?: string[]
  description?: string
  chat_template?: string
}

// /v0/models/discover-gguf response types
export interface GgufFileInfo {
  filename: string
  size_bytes: number
  quantization?: string | null
}

export interface GgufDiscoveryResult {
  repo: string
  provider: string
  trusted: boolean
  files: GgufFileInfo[]
}

export interface DiscoverGgufResponse {
  base_model: string
  gguf_alternatives: GgufDiscoveryResult[]
  cached: boolean
}

// /v1/models レスポンス
interface OpenAIModelsResponse {
  object: 'list'
  data: OpenAIModel[]
}

// 後方互換用: RegisteredModelView は OpenAIModel にマッピング
export interface RegisteredModelView {
  owned_by?: string // "router" | "openai" | "google" | "anthropic"
  name: string
  source?: string
  description?: string
  status?: string
  lifecycle_status: LifecycleStatus
  download_progress?: DownloadProgress
  ready: boolean
  repo?: string
  filename?: string
  size_gb?: number
  required_memory_gb?: number
  tags: string[]
  capabilities?: ModelCapabilities
  chat_template?: string
}

// OpenAIModel を RegisteredModelView に変換
function toRegisteredModelView(model: OpenAIModel): RegisteredModelView {
  const sizeGb =
    typeof model.size_bytes === 'number' ? model.size_bytes / (1024 * 1024 * 1024) : undefined
  const requiredGb =
    typeof model.required_memory_bytes === 'number'
      ? model.required_memory_bytes / (1024 * 1024 * 1024)
      : undefined
  return {
    name: model.id,
    owned_by: model.owned_by,
    lifecycle_status: model.lifecycle_status,
    download_progress: model.download_progress ?? undefined,
    ready: model.ready,
    source: model.source,
    description: model.description,
    repo: model.repo ?? undefined,
    filename: model.filename ?? undefined,
    size_gb: sizeGb,
    required_memory_gb: requiredGb,
    capabilities: model.capabilities,
    tags: model.tags ?? [],
    chat_template: model.chat_template,
  }
}

// NOTE: Model Hub機能は廃止されました
// モデル管理はエンドポイント側の責任（ゲートウェイ設計方針）
// ダウンロード状態は /v1/models の lifecycle_status で確認

export const modelsApi = {
  /** OpenAI互換の登録済みモデル一覧を取得 */
  getRegistered: async (): Promise<RegisteredModelView[]> => {
    // /v1/models - OpenAI-compatible model list (includes lifecycle_status)
    // Requires API key auth, using API key from localStorage
    const apiKey = localStorage.getItem('llmlb-api-key') || 'sk_debug'
    const response = await fetch('/v1/models', {
      headers: {
        Authorization: `Bearer ${apiKey}`,
      },
    })
    if (!response.ok) {
      // Get error details and throw appropriate error
      const errorBody = await response.json().catch(() => ({ error: 'Unknown error' }))
      const message = errorBody.error || undefined
      throw new ApiError(response.status, response.statusText, message)
    }
    const json = (await response.json()) as OpenAIModelsResponse
    // Convert from OpenAI format to RegisteredModelView format
    return json.data.map(toRegisteredModelView)
  },
}

// API Keys API
export type ApiKeyScope = 'endpoint' | 'api' | 'admin'

export interface ApiKey {
  id: string
  name: string
  key_prefix?: string
  created_by?: string
  created_at: string
  expires_at?: string
  last_used_at?: string
  scopes: ApiKeyScope[]
}

export interface CreateApiKeyResponse {
  id: string
  name: string
  key: string // Full key only returned on creation
  key_prefix: string
  created_at: string
  expires_at?: string
  scopes: ApiKeyScope[]
}

export const apiKeysApi = {
  list: () =>
    fetchWithAuth<{ api_keys: ApiKey[] }>('/v0/api-keys').then(
      (res) => res.api_keys
    ),

  create: (data: { name: string; expires_at?: string; scopes: ApiKeyScope[] }) =>
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

// Invitations API
export interface Invitation {
  id: string
  created_by: string
  created_at: string
  expires_at: string
  status: 'active' | 'used' | 'revoked'
  used_by?: string
  used_at?: string
}

export interface CreateInvitationResponse {
  id: string
  code: string
  created_at: string
  expires_at: string
}

export const invitationsApi = {
  list: async (): Promise<Invitation[]> => {
    const res = await fetchWithAuth<{ invitations: Invitation[] }>('/v0/invitations')
    return res.invitations
  },

  create: (expiresInHours?: number) =>
    fetchWithAuth<CreateInvitationResponse>('/v0/invitations', {
      method: 'POST',
      body: JSON.stringify({ expires_in_hours: expiresInHours }),
    }),

  revoke: (id: string) =>
    fetchWithAuth<void>(`/v0/invitations/${id}`, { method: 'DELETE' }),
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
