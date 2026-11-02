import { useState, useEffect, useCallback, useRef, createContext, useContext, type ReactNode } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { authApi } from '@/lib/api'
import { AUTH_SCOPE_INVALIDATED_EVENT, redirectToLogin, setExpectedUsageScope } from '@/lib/api/client'

interface User {
  id: string
  username: string
  role: string
  must_change_password: boolean
  organization_id: string | null
  organization_role: 'admin' | 'member' | null
  scope: 'global' | 'organization' | 'account' | 'unassigned'
}

interface AuthContextType {
  user: User | null
  isLoading: boolean
  isLoggedIn: boolean
  login: (username: string, password: string) => Promise<void>
  logout: () => Promise<void>
  checkAuth: () => Promise<void>
}

const AuthContext = createContext<AuthContextType | null>(null)

async function fetchCurrentUser(): Promise<User | null> {
  try {
    const data = await authApi.me()
    return {
      id: data.user_id,
      username: data.username,
      role: data.role,
      must_change_password: data.must_change_password,
      organization_id: data.organization_id ?? null,
      organization_role: data.organization_role ?? null,
      scope: data.scope ?? (data.role === 'admin' ? 'global' : 'unassigned'),
    }
  } catch {
    return null
  }
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const queryClient = useQueryClient()
  const authRequest = useRef(0)
  const scopeRefreshPending = useRef(false)
  const identity = useRef<string | undefined>(undefined)
  const [scopeKey, setScopeKey] = useState('initial')

  const applyAuthResult = useCallback(async (nextUser: User | null, requestId: number) => {
    if (requestId !== authRequest.current) return
    const nextIdentity = nextUser ? JSON.stringify([
      nextUser.id, nextUser.role, nextUser.organization_id, nextUser.organization_role, nextUser.scope,
    ]) : 'signed-out'
    if (identity.current !== undefined && identity.current !== nextIdentity) {
      await queryClient.cancelQueries()
      queryClient.clear()
      if (requestId === authRequest.current) setScopeKey(nextIdentity)
    }
    if (requestId !== authRequest.current) return
    const scope = nextUser?.scope
    setExpectedUsageScope(!nextUser ? null : scope === 'organization'
      ? `organization:${nextUser.organization_id}` : scope === 'account'
        ? `account:${nextUser.organization_id}:${nextUser.id}` : scope ?? null, nextIdentity)
    identity.current = nextIdentity
    setUser(nextUser)
    setIsLoading(false)
  }, [queryClient])

  const checkAuth = useCallback(async () => {
    const requestId = ++authRequest.current
    await applyAuthResult(await fetchCurrentUser(), requestId)
  }, [applyAuthResult])
  const invalidatePendingAuth = useCallback(() => { ++authRequest.current }, [])
  const refreshChangedScope = useCallback(async () => {
    if (scopeRefreshPending.current) return
    scopeRefreshPending.current = true
    invalidatePendingAuth()
    setIsLoading(true)
    try {
      await queryClient.cancelQueries()
      queryClient.clear()
      await checkAuth()
    } finally {
      scopeRefreshPending.current = false
    }
  }, [checkAuth, invalidatePendingAuth, queryClient])

  const login = useCallback(async (username: string, password: string) => {
    ++authRequest.current
    await authApi.login(username, password)
    await checkAuth()
  }, [checkAuth])

  const logout = useCallback(async () => {
    const requestId = ++authRequest.current
    await authApi.logout()
    await applyAuthResult(null, requestId)
    redirectToLogin()
  }, [applyAuthResult])

  useEffect(() => {
    // 認証 API の応答（外部システム）を購読し、そのコールバックで state を更新する
    void checkAuth()
    const refresh = () => { void checkAuth() }
    const invalidate = () => { void refreshChangedScope() }
    window.addEventListener('focus', refresh)
    window.addEventListener(AUTH_SCOPE_INVALIDATED_EVENT, invalidate)
    return () => {
      invalidatePendingAuth()
      window.removeEventListener('focus', refresh)
      window.removeEventListener(AUTH_SCOPE_INVALIDATED_EVENT, invalidate)
    }
  }, [checkAuth, invalidatePendingAuth, refreshChangedScope])

  return (
    <AuthContext.Provider key={scopeKey}
      value={{
        user,
        isLoading,
        isLoggedIn: !!user,
        login,
        logout,
        checkAuth,
      }}
    >
      {children}
    </AuthContext.Provider>
  )
}

export function useAuth() {
  const context = useContext(AuthContext)
  if (!context) {
    throw new Error('useAuth must be used within an AuthProvider')
  }
  return context
}
