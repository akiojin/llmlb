// Shared API client utilities

import {
  AuthScopeChangedError,
  getAuthSessionSnapshot,
  invalidateAuthSession,
  isAuthSessionCurrent,
  type AuthSessionSnapshot,
} from '@/lib/authSession'

const API_BASE = ''

interface FetchOptions extends RequestInit {
  params?: Record<string, string | number | boolean | undefined>
  /** `/auth/me` establishes the newly observed scope rather than consuming it. */
  sessionRefresh?: boolean
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

export function redirectToLogin(): void {
  if (!redirectingToLogin) {
    redirectingToLogin = true
    window.location.href = '/dashboard/login.html'
  }
}

function requireCurrentSession(snapshot: AuthSessionSnapshot, response: Response, sessionRefresh: boolean): void {
  if (!isAuthSessionCurrent(snapshot)) throw new AuthScopeChangedError()
  // A shared scope (global, or one organization) can contain multiple actors.
  // Cookie changes in another tab must invalidate the previous actor's view too.
  const responseScope = response.headers.get('X-LLMLB-Scope')
  const responseAccount = response.headers.get('X-LLMLB-Account')
  if (!sessionRefresh && (
    (snapshot.scope && responseScope && responseScope !== snapshot.scope)
    || (snapshot.accountId && responseAccount && responseAccount !== snapshot.accountId)
  )) {
    invalidateAuthSession(true)
    throw new AuthScopeChangedError()
  }
}

export async function fetchWithAuth<T>(
  endpoint: string,
  options: FetchOptions = {}
): Promise<T> {
  const { params, sessionRefresh = false, ...fetchOptions } = options
  const session = getAuthSessionSnapshot()

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
    cache: 'no-store',
  })

  requireCurrentSession(session, response, sessionRefresh)

  if (response.status === 401) {
    invalidateAuthSession()
    redirectToLogin()
    throw new ApiError(401, 'Unauthorized')
  }

  if (!response.ok) {
    const error = await createApiErrorFromResponse(response)
    requireCurrentSession(session, response, sessionRefresh)
    throw error
  }

  // Handle empty responses
  const text = await response.text()
  requireCurrentSession(session, response, sessionRefresh)
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
