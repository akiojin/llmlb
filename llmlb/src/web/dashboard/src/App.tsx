import { useEffect, useState, useCallback } from 'react'
import { useAuth } from '@/hooks/useAuth'
import Dashboard from '@/pages/Dashboard'
import EndpointPlayground from '@/pages/EndpointPlayground'

type Route =
  | { type: 'dashboard' }
  | { type: 'playground'; endpointId: string }

function parseHash(): Route {
  const hash = window.location.hash.slice(1) // Remove #
  if (hash.startsWith('playground/')) {
    const endpointId = hash.slice('playground/'.length)
    if (endpointId) {
      return { type: 'playground', endpointId }
    }
  }
  return { type: 'dashboard' }
}

function App() {
  const { isLoading, isLoggedIn } = useAuth()
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
    // Redirect to login if not authenticated
    if (!isLoading && !isLoggedIn) {
      window.location.href = '/dashboard/login.html'
    }
  }, [isLoading, isLoggedIn])

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
