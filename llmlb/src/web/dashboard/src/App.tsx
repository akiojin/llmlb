import { useEffect, useState, useCallback } from 'react'
import { useAuth } from '@/hooks/useAuth'
import Dashboard from '@/pages/Dashboard'
import EndpointPlayground from '@/pages/EndpointPlayground'
import LoadBalancerPlayground from '@/pages/LoadBalancerPlayground'
import AuditLogPage from '@/pages/AuditLog'
import OrganizationsPage from '@/pages/Organizations'
import { redirectToLogin } from '@/lib/api/client'
import { Button } from '@/components/ui/button'

type Route =
  | { type: 'dashboard' }
  | { type: 'lb-playground'; initialModel?: string }
  | { type: 'playground'; endpointId: string }
  | { type: 'audit-log' }
  | { type: 'organizations' }

function parseHash(): Route {
  const hash = window.location.hash.slice(1) // Remove #
  if (hash === 'organizations') return { type: 'organizations' }
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
  const { isLoading, isLoggedIn, user, logout } = useAuth()
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
      redirectToLogin()
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

  // Redirects happen in an effect; private pages must not mount during that interval.
  if (!isLoggedIn || user?.must_change_password) return null
  if (user?.role !== 'admin' && !user?.organization_id) {
    return <main className="flex min-h-screen items-center justify-center bg-background p-6"><div className="max-w-md space-y-4 text-center">
      <h1 className="font-display text-xl font-semibold">Organization membership required</h1>
      <p className="text-sm text-muted-foreground">Ask a system administrator to assign your organization before viewing usage or using inference.</p>
      <Button variant="outline" onClick={() => { void logout() }}>Sign out</Button>
    </div></main>
  }

  switch (route.type) {
    case 'organizations':
      return <OrganizationsPage onBack={navigateToDashboard} />
    case 'audit-log':
      if (user?.role !== 'admin') return <main className="p-6"><p>System administrator access required.</p><Button variant="link" onClick={navigateToDashboard}>Back to Dashboard</Button></main>
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
