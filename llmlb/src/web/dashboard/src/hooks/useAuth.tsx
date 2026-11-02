import {
  Fragment, useState, useEffect, useCallback, useRef,
  createContext, useContext, type ReactNode,
} from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { authApi, type CurrentUser } from '@/lib/api/auth'
import { redirectToLogin } from '@/lib/api/client'
import {
  getAuthSessionSnapshot,
  invalidateAuthSession,
  setAuthSession,
  subscribeToAuthSessionInvalidation,
} from '@/lib/authSession'

export interface User {
  id: string
  username: string
  role: string
  must_change_password: boolean
  organization_id: string | null
  organization_role: 'admin' | 'member' | null
  disabled: boolean
  scope: string | null
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

function currentUser(data: CurrentUser): User | null {
  if (data.disabled) return null
  return {
    id: data.user_id,
    username: data.username,
    role: data.role,
    must_change_password: data.must_change_password,
    organization_id: data.organization_id ?? null,
    organization_role: data.organization_role ?? null,
    disabled: data.disabled ?? false,
    scope: data.scope ?? null,
  }
}

function identityOf(user: User | null): string | null {
  return user ? JSON.stringify([
    user.id, user.role, user.organization_id, user.organization_role,
    user.disabled, user.must_change_password, user.scope,
  ]) : null
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient()
  const latestRequest = useRef(0)
  const hasResolvedAuth = useRef(false)
  const [auth, setAuth] = useState<{ user: User | null; isLoading: boolean; generation: number }>({
    user: null, isLoading: true, generation: getAuthSessionSnapshot().generation,
  })

  const clearPrivateQueries = useCallback(() => {
    // cancelQueries aborts active query signals synchronously; clear removes old pages and mutations.
    void queryClient.cancelQueries()
    queryClient.clear()
  }, [queryClient])

  const checkAuth = useCallback(async () => {
    const request = ++latestRequest.current
    let nextUser: User | null
    try {
      nextUser = currentUser(await authApi.me())
    } catch {
      nextUser = null
    }
    if (request !== latestRequest.current) return
    const firstResolution = !hasResolvedAuth.current
    hasResolvedAuth.current = true
    const previous = getAuthSessionSnapshot()
    const next = setAuthSession(identityOf(nextUser), nextUser?.scope ?? null, nextUser?.id ?? null)
    // App waits for the first me before mounting private pages. There is no
    // previous authenticated view to discard at that initial resolution.
    const boundaryChanged = !firstResolution && next.generation !== previous.generation
    if (boundaryChanged) clearPrivateQueries()
    setAuth((previousAuth) => ({ user: nextUser, isLoading: false,
      generation: boundaryChanged ? next.generation : previousAuth.generation }))
  }, [clearPrivateQueries])

  const login = useCallback(async (username: string, password: string) => {
    invalidateAuthSession()
    const attempt = latestRequest.current
    await authApi.login(username, password)
    if (attempt === latestRequest.current) await checkAuth()
  }, [checkAuth])

  const logout = useCallback(async () => {
    // Privacy does not depend on the logout endpoint replying promptly.
    invalidateAuthSession()
    try {
      await authApi.logout()
    } finally {
      redirectToLogin()
    }
  }, [])

  useEffect(() => {
    const unsubscribe = subscribeToAuthSessionInvalidation((recheck) => {
      hasResolvedAuth.current = true
      ++latestRequest.current
      clearPrivateQueries()
      setAuth({ user: null, isLoading: recheck, generation: getAuthSessionSnapshot().generation })
      if (recheck) void checkAuth()
    })
    void checkAuth()
    return () => {
      unsubscribe()
      ++latestRequest.current
      setAuthSession(null, null)
      clearPrivateQueries()
    }
  }, [checkAuth, clearPrivateQueries])

  return (
    <AuthContext.Provider
      value={{
        user: auth.user,
        isLoading: auth.isLoading,
        isLoggedIn: !!auth.user,
        login,
        logout,
        checkAuth,
      }}
    >
      <Fragment key={auth.generation}>{children}</Fragment>
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
