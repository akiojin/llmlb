// Shared API client utilities

const API_BASE = ''
export const AUTH_SCOPE_INVALIDATED_EVENT = 'llmlb:auth-scope-invalidated'
let expectedUsageScope: string | null = null
let expectedIdentity: string | null = null
let sessionGeneration = 0

/** Snapshot for commands and cache callbacks that may outlive their account. */
export function getSessionGeneration(): number {
  return sessionGeneration
}

/** Bind data responses to the session confirmed by /auth/me. */
export function setExpectedUsageScope(scope: string | null, identity: string | null = scope): void {
  if (scope !== expectedUsageScope || identity !== expectedIdentity) sessionGeneration++
  expectedUsageScope = scope
  expectedIdentity = identity
}

function invalidateAuthScope(): void {
  window.dispatchEvent(new Event(AUTH_SCOPE_INVALIDATED_EVENT))
}

interface FetchOptions extends RequestInit {
  params?: Record<string, string | number | boolean | undefined>
}

interface ApiErrorOptions {
  message?: string
  errorType?: string
  errorCode?: string | number
  rawBody?: string
}

interface ApiErrorBodyShape {
  // llmlb's own `/api/*` errors use a plain string; OpenAI-compatible errors use an object.
  error?:
    | string
    | {
        message?: string
        type?: string
        code?: string | number
      }
  message?: string
}

export class ApiError extends Error {
  public errorType?: string
  public errorCode?: string | number
  public rawBody?: string

  constructor(
    public status: number,
    public statusText: string,
    options: string | ApiErrorOptions = {}
  ) {
    const normalized: ApiErrorOptions =
      typeof options === 'string' ? { message: options } : options

    super(normalized.message || `${status} ${statusText}`)
    this.name = 'ApiError'
    this.errorType = normalized.errorType
    this.errorCode = normalized.errorCode
    this.rawBody = normalized.rawBody
  }
}

function parseApiErrorBody(bodyText: string): ApiErrorOptions {
  if (!bodyText) {
    return {}
  }

  try {
    const parsed = JSON.parse(bodyText) as ApiErrorBodyShape
    if (typeof parsed?.error === 'string') {
      return {
        message: parsed.error,
        rawBody: bodyText,
      }
    }
    if (parsed?.error) {
      return {
        message: parsed.error.message,
        errorType: parsed.error.type,
        errorCode: parsed.error.code,
        rawBody: bodyText,
      }
    }
    if (typeof parsed?.message === 'string') {
      return {
        message: parsed.message,
        rawBody: bodyText,
      }
    }
  } catch {
    // Plain-text error body.
  }

  return {
    message: bodyText,
    rawBody: bodyText,
  }
}

export async function createApiErrorFromResponse(response: Response): Promise<ApiError> {
  const bodyText = await response.text()
  return new ApiError(response.status, response.statusText, parseApiErrorBody(bodyText))
}

// 401 を複数の並行リクエストが同時に受け取っても、ログインへのリダイレクトは一度だけ
// 行う（多重 navigation によるちらつきを防ぐ）。
let redirectingToLogin = false

/** Share one navigation between API failures and the session gate. */
export function redirectToLogin(): void {
  if (redirectingToLogin) return
  redirectingToLogin = true
  window.location.href = '/dashboard/login.html'
}

export async function fetchWithAuth<T>(
  endpoint: string,
  options: FetchOptions = {}
): Promise<T> {
  const requestGeneration = sessionGeneration
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

  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(options.headers as Record<string, string>),
  }

  const method = (fetchOptions.method || 'GET').toUpperCase()
  if (method !== 'GET' && method !== 'HEAD') {
    const csrfToken = getCsrfToken()
    if (csrfToken) {
      headers['X-CSRF-Token'] = csrfToken
    }
  }

  const response = await fetch(url, {
    ...fetchOptions,
    headers,
    credentials: 'include',
  })

  const responseScope = response.headers.get('x-llmlb-usage-scope')
  const assertCurrentSession = (cancelUnreadBody = false) => {
    if (requestGeneration !== sessionGeneration || (expectedUsageScope && responseScope && expectedUsageScope !== responseScope)) {
      // An old account's response must not refresh the newly confirmed session.
      if (requestGeneration === sessionGeneration) invalidateAuthScope()
      if (cancelUnreadBody) void response.body?.cancel().catch(() => {})
      throw new ApiError(409, 'Session scope changed', { message: 'Your access changed. Reloading your current organization.' })
    }
  }
  assertCurrentSession(true)
  if (response.status === 403 && (endpoint.startsWith('/api/dashboard/') || endpoint.startsWith('/api/organizations') || endpoint.startsWith('/api/me/api-keys'))) {
    invalidateAuthScope()
  }
  if (response.status === 401) {
    if (!endpoint.startsWith('/api/auth/')) invalidateAuthScope()
    redirectToLogin()
    throw new ApiError(401, 'Unauthorized')
  }

  if (!response.ok) {
    const error = await createApiErrorFromResponse(response)
    assertCurrentSession()
    throw error
  }

  // Handle empty responses
  const text = await response.text()
  assertCurrentSession()
  if (!text) {
    return {} as T
  }

  return JSON.parse(text)
}

export function getCsrfToken(): string | null {
  const match = document.cookie.match(/(?:^|; )llmlb_csrf=([^;]*)/)
  return match ? decodeURIComponent(match[1]) : null
}

export { API_BASE }
