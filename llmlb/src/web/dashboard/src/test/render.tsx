import type { ReactElement, ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render } from '@testing-library/react'
import { vi } from 'vitest'
import { Toaster } from '@/components/ui/toaster'
import { TooltipProvider } from '@/components/ui/tooltip'
import { AuthProvider, useAuth } from '@/hooks/useAuth'
import { ApiError, authApi } from '@/lib/api'

export type TestUser = Awaited<ReturnType<typeof authApi.me>>

export const adminUser: TestUser = {
  user_id: 'user-admin',
  username: 'admin',
  role: 'admin',
  must_change_password: false,
}

export const viewerUser: TestUser = {
  user_id: 'user-viewer',
  username: 'viewer',
  role: 'viewer',
  must_change_password: false,
}

interface RenderOptions {
  /**
   * Signed-in user returned by `/api/auth/me`. `null` renders signed out; a
   * promise keeps the session check pending until the test settles it.
   */
  user?: TestUser | Promise<TestUser> | null
}

function SessionGate({ children }: { children: ReactNode }) {
  const { isLoading } = useAuth()
  return isLoading ? null : children
}

function renderInApp(ui: ReactElement, { user = adminUser }: RenderOptions, awaitSession: boolean) {
  if (user) {
    vi.spyOn(authApi, 'me').mockImplementation(() => Promise.resolve(user))
  } else {
    vi.spyOn(authApi, 'me').mockRejectedValue(new ApiError(401, 'Unauthorized'))
  }

  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })

  // Providers live in `wrapper` so that `rerender` keeps them.
  const view = render(ui, {
    wrapper: ({ children }) => (
      <QueryClientProvider client={queryClient}>
        <AuthProvider>
          <TooltipProvider>
            {awaitSession ? <SessionGate>{children}</SessionGate> : children}
            <Toaster />
          </TooltipProvider>
        </AuthProvider>
      </QueryClientProvider>
    ),
  })

  return { ...view, queryClient }
}

/**
 * Render with the same provider tree as `main.tsx`. Queries do not retry, so
 * a rejected request reaches the error state immediately.
 */
export function renderWithProviders(ui: ReactElement, options: RenderOptions = {}) {
  return renderInApp(ui, options, false)
}

/**
 * Render a routed page the way `App` mounts it: only after the session check
 * has completed, so the page sees the signed-in user on its first render.
 */
export function renderPage(ui: ReactElement, options: RenderOptions = {}) {
  return renderInApp(ui, options, true)
}

/** A promise whose settlement the test controls, for asserting loading states. */
export function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}
