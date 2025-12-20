import { useState } from 'react'
import { useAuth } from '@/hooks/useAuth'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { ApiKeyModal } from '@/components/api-keys/ApiKeyModal'
import { UserModal } from '@/components/users/UserModal'
import { InvitationModal } from '@/components/invitations/InvitationModal'
import {
  Cpu,
  Key,
  LogOut,
  Moon,
  Sun,
  User,
  Users,
  MessageSquare,
  RefreshCw,
  Ticket,
} from 'lucide-react'

interface HeaderProps {
  user: { username: string; role: string } | null
  isConnected?: boolean
  lastRefreshed?: Date | null
  fetchTimeMs?: number | null
}

export function Header({ user, isConnected = true, lastRefreshed, fetchTimeMs }: HeaderProps) {
  const { logout } = useAuth()
  const [theme, setTheme] = useState<'dark' | 'light'>('dark')
  const [apiKeyModalOpen, setApiKeyModalOpen] = useState(false)
  const [userModalOpen, setUserModalOpen] = useState(false)
  const [invitationModalOpen, setInvitationModalOpen] = useState(false)
  const [isRefreshing, setIsRefreshing] = useState(false)

  const toggleTheme = () => {
    const newTheme = theme === 'dark' ? 'light' : 'dark'
    setTheme(newTheme)
    document.documentElement.classList.toggle('dark', newTheme === 'dark')
  }

  const handleRefresh = () => {
    setIsRefreshing(true)
    window.location.reload()
  }

  const openPlayground = () => {
    window.open('/dashboard/playground.html', '_blank')
  }

  return (
    <>
      <header className="sticky top-0 z-40 border-b border-border/50 bg-background/80 backdrop-blur-xl">
        <div className="mx-auto flex h-16 max-w-[1600px] items-center justify-between px-4 sm:px-6 lg:px-8">
          {/* Logo */}
          <div className="flex items-center gap-3">
            <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary/10 glow-sm">
              <Cpu className="h-5 w-5 text-primary" />
            </div>
            <div>
              <h1 className="font-display text-lg font-semibold tracking-tight">
                LLM Router
              </h1>
              <p className="text-xs text-muted-foreground">Dashboard</p>
            </div>
          </div>

          {/* Status Indicators */}
          <div className="hidden md:flex items-center gap-4 text-xs text-muted-foreground">
            {/* Connection Status */}
            <span id="connection-status" className="flex items-center gap-1.5">
              <span
                className={`h-2 w-2 rounded-full ${
                  isConnected ? 'bg-green-500' : 'bg-red-500'
                }`}
              />
              Connection: {isConnected ? 'Online' : 'Offline'}
            </span>

            {/* Last Refreshed */}
            {lastRefreshed && (
              <span id="last-refreshed">
                Last updated: {lastRefreshed.toLocaleTimeString()}
              </span>
            )}
            {!lastRefreshed && (
              <span id="last-refreshed">Last updated: --:--:--</span>
            )}

            {/* Performance Metrics */}
            {fetchTimeMs !== null && fetchTimeMs !== undefined && (
              <span id="refresh-metrics">Fetch time: {fetchTimeMs}ms</span>
            )}
            {(fetchTimeMs === null || fetchTimeMs === undefined) && (
              <span id="refresh-metrics">Fetch time: --ms</span>
            )}
          </div>

          {/* Actions */}
          <div className="flex items-center gap-2">
            {/* Playground Button */}
            <Button
              id="chat-open"
              variant="outline"
              size="sm"
              onClick={openPlayground}
              className="hidden sm:inline-flex"
            >
              <MessageSquare className="mr-2 h-4 w-4" />
              Playground
            </Button>

            {/* API Keys Button */}
            <Button
              id="api-keys-button"
              variant="outline"
              size="sm"
              onClick={() => setApiKeyModalOpen(true)}
              className="hidden sm:inline-flex"
            >
              <Key className="mr-2 h-4 w-4" />
              API Keys
            </Button>

            {/* Refresh Button */}
            <Button
              id="refresh-button"
              variant="ghost"
              size="icon"
              onClick={handleRefresh}
              disabled={isRefreshing}
            >
              <RefreshCw
                className={`h-4 w-4 ${isRefreshing ? 'animate-spin' : ''}`}
              />
            </Button>

            {/* Theme Toggle */}
            <Button id="theme-toggle" variant="ghost" size="icon" onClick={toggleTheme}>
              {theme === 'dark' ? (
                <Sun className="h-4 w-4" />
              ) : (
                <Moon className="h-4 w-4" />
              )}
            </Button>

            {/* User Menu */}
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="ghost" size="icon" className="relative">
                  <User className="h-4 w-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-56">
                <DropdownMenuLabel>
                  <div className="flex flex-col space-y-1">
                    <p className="text-sm font-medium">{user?.username}</p>
                    <p className="text-xs text-muted-foreground capitalize">
                      {user?.role}
                    </p>
                  </div>
                </DropdownMenuLabel>
                <DropdownMenuSeparator />

                {/* Mobile-only items */}
                <DropdownMenuItem
                  onClick={openPlayground}
                  className="sm:hidden"
                >
                  <MessageSquare className="mr-2 h-4 w-4" />
                  Playground
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => setApiKeyModalOpen(true)}
                  className="sm:hidden"
                >
                  <Key className="mr-2 h-4 w-4" />
                  API Keys
                </DropdownMenuItem>
                <DropdownMenuSeparator className="sm:hidden" />

                {/* Admin-only items */}
                {user?.role === 'admin' && (
                  <>
                    <DropdownMenuItem onClick={() => setUserModalOpen(true)}>
                      <Users className="mr-2 h-4 w-4" />
                      Manage Users
                    </DropdownMenuItem>
                    <DropdownMenuItem onClick={() => setInvitationModalOpen(true)}>
                      <Ticket className="mr-2 h-4 w-4" />
                      Invitation Codes
                    </DropdownMenuItem>
                    <DropdownMenuSeparator />
                  </>
                )}

                <DropdownMenuItem onClick={logout} className="text-destructive">
                  <LogOut className="mr-2 h-4 w-4" />
                  Sign out
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>
      </header>

      {/* Modals */}
      <ApiKeyModal open={apiKeyModalOpen} onOpenChange={setApiKeyModalOpen} />
      <UserModal open={userModalOpen} onOpenChange={setUserModalOpen} />
      <InvitationModal open={invitationModalOpen} onOpenChange={setInvitationModalOpen} />
    </>
  )
}
