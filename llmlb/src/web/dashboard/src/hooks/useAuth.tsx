import { useState, useEffect, useCallback, createContext, useContext, type ReactNode } from 'react'
import { authApi } from '@/lib/api'

interface User {
  id: number
  username: string
  role: string
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

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null)
  const [isLoading, setIsLoading] = useState(true)

  const checkAuth = useCallback(async () => {
    try {
      const userData = await authApi.me()
      setUser(userData)
    } catch {
      setUser(null)
    } finally {
      setIsLoading(false)
    }
  }, [])

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
    checkAuth()
  }, [checkAuth])

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
