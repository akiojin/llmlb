import { useState } from 'react'
import { useAuth } from '@/hooks/useAuth'
import { useTheme } from '@/hooks/useTheme'
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
import { usePendingRegistrationCountViewModel } from '@/viewmodels/useRegistrationRequestsViewModel'
import { Badge } from '@/components/ui/badge'
import { NotificationSettingsModal } from '@/components/notifications/NotificationSettingsModal'
import { ActivityConnectionStatus } from './ActivityConnectionStatus'
import {
  Bell,
  Cpu,
  Key,
  KeyRound,
  LogOut,
  Moon,
  Network,
  ShieldCheck,
  Sun,
  User,
  Users,
  RefreshCw,
} from 'lucide-react'

type UpdateStateSummary =
  | 'up_to_date'
  | 'available'
  | 'draining'
  | 'applying'
  | 'failed'
  | undefined

interface HeaderProps {
  user: { username: string; role: string } | null
  isConnected?: boolean
  lastRefreshed?: Date | null
  fetchTimeMs?: number | null
  systemVersion?: string | null
  updateState?: UpdateStateSummary
  updateLatest?: string | null
  minimalViewer?: boolean
  activityStatus?: 'live' | 'polling'
  isActivityStale?: boolean
}

export function Header({
  user,
  isConnected = true,
  lastRefreshed,
  fetchTimeMs,
  systemVersion,
  updateState,
  updateLatest,
  minimalViewer = false,
  activityStatus,
  isActivityStale,
}: HeaderProps) {
  const { logout } = useAuth()
  const { theme, toggleTheme } = useTheme()
  const [apiKeyModalOpen, setApiKeyModalOpen] = useState(false)
  const [userModalOpen, setUserModalOpen] = useState(false)
  const { pendingCount } = usePendingRegistrationCountViewModel(user?.role === 'admin')
  const [notificationModalOpen, setNotificationModalOpen] = useState(false)
  const [isRefreshing, setIsRefreshing] = useState(false)
  const displayVersion = systemVersion ?? '--'

  const handleRefresh = () => {
    setIsRefreshing(true)
    window.location.reload()
  }

  return (
    <>
      <header className="sticky top-0 z-40 border-b border-border/50 bg-background/80 backdrop-blur-xl">
        <div className="mx-auto flex min-h-16 max-w-[1600px] flex-wrap items-center justify-between gap-y-2 px-4 py-2 sm:px-6 lg:px-8">
          {/* Logo */}
          <div className="flex items-center gap-3">
            <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary/10 glow-sm">
              <Cpu className="h-5 w-5 text-primary" />
            </div>
            <div>
              <h1 className="font-display text-lg font-semibold tracking-tight">
                LLM Load Balancer
              </h1>
              <p className="text-xs text-muted-foreground flex items-center gap-1.5">
                Dashboard
                <span id="current-version" className="font-mono text-[11px]">
                  Current v{displayVersion}
                </span>
                {!minimalViewer && (
                  <>
                    {updateState && updateState !== 'up_to_date' && (
                      <span className="inline-flex items-center gap-1">
                        <span
                          className={`h-1.5 w-1.5 rounded-full ${
                            updateState === 'failed'
                              ? 'bg-red-500'
                              : 'bg-yellow-500'
                          }`}
                        />
                        <span className="text-[10px]">
                          {updateState === 'available' && updateLatest
                            ? `v${updateLatest} available`
                            : updateState === 'draining' || updateState === 'applying'
                              ? 'Updating...'
                              : updateState === 'failed'
                                ? 'Update failed'
                                : ''}
                        </span>
                      </span>
                    )}
                    {updateState === 'up_to_date' && (
                      <span className="inline-flex items-center gap-1">
                        <span className="h-1.5 w-1.5 rounded-full bg-green-500" />
                      </span>
                    )}
                  </>
                )}
              </p>
            </div>
          </div>

          {/* Status Indicators */}
          {!minimalViewer && (
            <div className="hidden items-center gap-4 text-xs text-muted-foreground md:flex">
            {/* Connection Status */}
            <span id="connection-status" className="flex items-center gap-1.5">
              <span
                className={`h-2 w-2 rounded-full ${
                  isConnected ? 'bg-green-500' : 'bg-red-500'
                }`}
              />
              {`Connection: ${isConnected ? 'Online' : 'Offline'}`}
            </span>

            {/* Last Refreshed */}
            {lastRefreshed && (
              <span id="last-refreshed">
                {`Last updated: ${lastRefreshed.toLocaleTimeString()}`}
              </span>
            )}
            {!lastRefreshed && (
              <span id="last-refreshed">{'Last updated: --:--:--'}</span>
            )}

            {/* Performance Metrics */}
            {fetchTimeMs !== null && fetchTimeMs !== undefined && (
              <span id="refresh-metrics">{`Fetch time: ${fetchTimeMs}ms`}</span>
            )}
            {(fetchTimeMs === null || fetchTimeMs === undefined) && (
              <span id="refresh-metrics">{'Fetch time: --ms'}</span>
            )}
            </div>
          )}

          {/* Actions */}
          <div className="flex flex-wrap items-center justify-end gap-2">
            {activityStatus ? <ActivityConnectionStatus status={activityStatus} isStale={isActivityStale} /> : null}
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

            <Button
              id="lb-playground-button"
              variant="outline"
              size="sm"
              className="hidden lg:inline-flex"
              onClick={() => {
                window.location.hash = 'lb-playground'
              }}
            >
              <Network className="mr-2 h-4 w-4" />
              LB Playground
            </Button>

            {user?.role === 'admin' && (
              <Button
                id="audit-log-button"
                variant="outline"
                size="sm"
                className="hidden lg:inline-flex"
                onClick={() => {
                  window.location.hash = 'audit-log'
                }}
              >
                <ShieldCheck className="mr-2 h-4 w-4" />
                Audit Log
              </Button>
            )}

            {/* Refresh Button */}
            <Button
              id="refresh-button"
              variant="outline"
              size="icon"
              onClick={handleRefresh}
              disabled={isRefreshing}
              aria-label="Refresh dashboard"
              title="Refresh"
            >
              <RefreshCw
                className={`h-4 w-4 ${isRefreshing ? 'animate-spin' : ''}`}
              />
            </Button>

            {/* Theme Toggle */}
            <Button
              id="theme-toggle"
              variant="outline"
              size="icon"
              onClick={toggleTheme}
              aria-label={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
              title="Toggle theme"
            >
              {theme === 'dark' ? (
                <Sun className="h-4 w-4" />
              ) : (
                <Moon className="h-4 w-4" />
              )}
            </Button>

            {/* User Menu */}
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="outline" size="icon" className="relative" aria-label="User menu">
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
                  onClick={() => setApiKeyModalOpen(true)}
                  className="sm:hidden"
                >
                  <Key className="mr-2 h-4 w-4" />
                  API Keys
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => {
                    window.location.hash = 'lb-playground'
                  }}
                  className="lg:hidden"
                >
                  <Network className="mr-2 h-4 w-4" />
                  LB Playground
                </DropdownMenuItem>
                <DropdownMenuSeparator className="sm:hidden" />

                {/* Admin-only items */}
                {user?.role === 'admin' && (
                  <>
                    <DropdownMenuItem onClick={() => setUserModalOpen(true)}>
                      <Users className="mr-2 h-4 w-4" />
                      Manage Users
                      {pendingCount > 0 && <Badge className="ml-auto" data-testid="pending-registration-count" aria-label={`${pendingCount} pending registrations`}>{pendingCount}</Badge>}
                    </DropdownMenuItem>
                    <DropdownMenuItem onClick={() => setNotificationModalOpen(true)}>
                      <Bell className="mr-2 h-4 w-4" />
                      Notifications
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      onClick={() => {
                        window.location.hash = 'audit-log'
                      }}
                    >
                      <ShieldCheck className="mr-2 h-4 w-4" />
                      Audit Log
                    </DropdownMenuItem>
                    <DropdownMenuSeparator />
                  </>
                )}

                <DropdownMenuItem
                  onClick={() => {
                    window.location.href = '/dashboard/change-password.html'
                  }}
                >
                  <KeyRound className="mr-2 h-4 w-4" />
                  Change Password
                </DropdownMenuItem>
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
      {user?.role === 'admin' && <UserModal open={userModalOpen} onOpenChange={setUserModalOpen} />}
      {user?.role === 'admin' && (
        <NotificationSettingsModal
          open={notificationModalOpen}
          onOpenChange={setNotificationModalOpen}
        />
      )}
    </>
  )
}
