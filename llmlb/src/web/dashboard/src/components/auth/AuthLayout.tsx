import type { ReactNode } from 'react'
import { Cpu } from 'lucide-react'

/** Shared full-screen shell (grid background, logo, footer) for signed-out account pages. */
export function AuthLayout({ children }: { children: ReactNode }) {
  return (
    <div className="relative min-h-screen w-full overflow-hidden bg-background">
      {/* Background Grid Pattern */}
      <div className="absolute inset-0 bg-grid opacity-30" />

      {/* Gradient Orbs */}
      <div className="absolute -left-40 -top-40 h-80 w-80 rounded-full bg-primary/20 blur-[100px]" />
      <div className="absolute -bottom-40 -right-40 h-80 w-80 rounded-full bg-primary/10 blur-[100px]" />

      {/* Content */}
      <div className="relative flex min-h-screen items-center justify-center p-4">
        <div className="w-full max-w-md animate-fade-up">
          {/* Logo */}
          <div className="mb-8 flex flex-col items-center gap-4">
            <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-primary/10 glow-sm">
              <Cpu className="h-8 w-8 text-primary" />
            </div>
            <div className="text-center">
              <h1 className="font-display text-3xl font-bold tracking-tight">
                LLM Load Balancer
              </h1>
              <p className="mt-1 text-sm text-muted-foreground">
                Mission Control Dashboard
              </p>
            </div>
          </div>

          {children}

          {/* Footer */}
          <p className="mt-6 text-center text-xs text-muted-foreground">
            LLM Load Balancer Dashboard v1.0
          </p>
        </div>
      </div>
    </div>
  )
}

/** Inline spinner + label used inside submit buttons while a request is pending. */
export function PendingLabel({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center gap-2">
      <div className="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
      {children}
    </div>
  )
}
