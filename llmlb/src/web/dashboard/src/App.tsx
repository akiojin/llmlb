import { useEffect, useState, useCallback } from 'react'
import { useAuth } from '@/hooks/useAuth'
import Dashboard from '@/pages/Dashboard'
import EndpointPlayground from '@/pages/EndpointPlayground'
import LoadBalancerPlayground from '@/pages/LoadBalancerPlayground'
import AuditLogPage from '@/pages/AuditLog'

type Route =
  | { type: 'dashboard' }
  | { type: 'lb-playground'; initialModel?: string }
  | { type: 'playground'; endpointId: string }
  | { type: 'audit-log' }

function parseHash(): Route {
  const hash = window.location.hash.slice(1) // Remove #
  if (hash === 'audit-log') {
    return { type: 'audit-log' }
  }
  if (hash === 'lb-playground' || hash.startsWith('lb-playground?')) {
    let initialModel: string | undefined
    const qIdx = hash.indexOf('?')
    if (qIdx >= 0) {
      const params = new URLSearchParams(hash.slice(qIdx + 1))
      initialModel = params.get('model') ?? undefined
    }
    return { type: 'lb-playground', initialModel }
  }
  if (hash.startsWith('playground/')) {
    const endpointId = hash.slice('playground/'.length)
    if (endpointId) {
      return { type: 'playground', endpointId }
    }
  }
  return { type: 'dashboard' }
}

function App() {
  const { isLoading, isLoggedIn, user } = useAuth()
  const [route, setRoute] = useState<Route>(parseHash)

  const navigateToDashboard = useCallback(() => {
    window.location.hash = ''
    setRoute({ type: 'dashboard' })
  }, [])

  useEffect(() => {
    const handleHashChange = () => {
      setRoute(parseHash())
    }
    window.addEventListener('hashchange', handleHashChange)
    return () => window.removeEventListener('hashchange', handleHashChange)
  }, [])

  useEffect(() => {
    if (isLoading) return
    // Redirect to login if not authenticated
    if (!isLoggedIn) {
      window.location.href = '/dashboard/login.html'
      return
    }
    // Redirect to change-password if password change is required
    if (user?.must_change_password) {
      window.location.href = '/dashboard/change-password.html'
      return
    }
  }, [isLoading, isLoggedIn, user])

  if (isLoading) {
    return (
      <div className="flex h-screen w-full items-center justify-center bg-background">
        <div className="flex flex-col items-center gap-4">
          <div className="relative">
            <div className="h-12 w-12 rounded-full border-4 border-primary/20" />
            <div className="absolute inset-0 h-12 w-12 animate-spin rounded-full border-4 border-transparent border-t-primary" />
          </div>
          <p className="text-sm text-muted-foreground">Loading...</p>
        </div>
      </div>
    )
  }

  switch (route.type) {
    case 'audit-log':
      return <AuditLogPage onBack={navigateToDashboard} />
    case 'lb-playground':
      return <LoadBalancerPlayground onBack={navigateToDashboard} initialModel={route.initialModel} />
    case 'playground':
      return (
        <EndpointPlayground
          endpointId={route.endpointId}
          onBack={navigateToDashboard}
        />
      )
    case 'dashboard':
    default:
      return <Dashboard />
  }
}

export default App
