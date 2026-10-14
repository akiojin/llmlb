import { useState, useEffect, useCallback, createContext, useContext, type ReactNode } from 'react'
import { authApi } from '@/lib/api'

interface User {
  id: string
  username: string
  role: string
  must_change_password: boolean
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
    }
  } catch {
    return null
  }
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null)
  const [isLoading, setIsLoading] = useState(true)

  const applyAuthResult = useCallback((nextUser: User | null) => {
    setUser(nextUser)
    setIsLoading(false)
  }, [])

  const checkAuth = useCallback(async () => {
    applyAuthResult(await fetchCurrentUser())
  }, [applyAuthResult])

  const login = useCallback(async (username: string, password: string) => {
    await authApi.login(username, password)
    await checkAuth()
  }, [checkAuth])

  const logout = useCallback(async () => {
    await authApi.logout()
    setUser(null)
    window.location.href = '/dashboard/login.html'
  }, [])

  useEffect(() => {
    // 認証 API の応答（外部システム）を購読し、そのコールバックで state を更新する
    fetchCurrentUser().then(applyAuthResult)
  }, [applyAuthResult])

  return (
    <AuthContext.Provider
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
