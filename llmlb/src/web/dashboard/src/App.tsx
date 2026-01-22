import { useEffect } from 'react'
import { useAuth } from '@/hooks/useAuth'
import { isAuthenticated } from '@/lib/api'
import Dashboard from '@/pages/Dashboard'

function App() {
  const { isLoading } = useAuth()

  useEffect(() => {
    // Redirect to login if not authenticated
    if (!isLoading && !isAuthenticated()) {
      window.location.href = '/dashboard/login.html'
    }
  }, [isLoading])

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

  return <Dashboard />
}

export default App
