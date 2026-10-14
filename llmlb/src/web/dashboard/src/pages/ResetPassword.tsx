import { useState } from 'react'
import { authApi, ApiError } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { AuthLayout, PendingLabel } from '@/components/auth/AuthLayout'
import {
  PasswordStrengthMeter,
  isPasswordValid,
} from '@/components/auth/PasswordStrengthMeter'
import { toast } from '@/hooks/use-toast'
import { CheckCircle2, KeyRound, Lock } from 'lucide-react'

const INVALID_TOKEN_MESSAGE = 'Invalid or expired reset token'

/**
 * Reads the reset token from the link fragment (`#token=...`) and strips it from the address
 * bar so it does not linger in browser history. The fragment is never sent to the server.
 */
function takeTokenFromLocation(): string {
  const token = new URLSearchParams(window.location.hash.slice(1)).get('token') ?? ''
  if (token) {
    window.history.replaceState(null, '', window.location.pathname)
  }
  return token
}

export default function ResetPasswordPage() {
  const [linkToken] = useState(takeTokenFromLocation)
  const [manualToken, setManualToken] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  const [isSuccess, setIsSuccess] = useState(false)

  const token = linkToken || manualToken.trim()

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()

    if (!isPasswordValid(newPassword)) {
      toast({
        variant: 'destructive',
        title: 'Password too weak',
        description: 'Use at least 8 characters including an uppercase letter and a number.',
      })
      return
    }

    if (newPassword !== confirmPassword) {
      toast({
        variant: 'destructive',
        title: 'Validation error',
        description: 'Passwords do not match',
      })
      return
    }

    setIsLoading(true)

    try {
      await authApi.resetPassword(token, newPassword)
      setIsSuccess(true)
    } catch (error) {
      const invalidToken = error instanceof ApiError && error.message === INVALID_TOKEN_MESSAGE
      toast({
        variant: 'destructive',
        title: invalidToken ? 'Reset link is invalid or expired' : 'Failed to reset password',
        description: invalidToken
          ? 'Request a new reset link and try again.'
          : error instanceof ApiError && error.status === 400
            ? error.message
            : 'Please try again',
      })
    } finally {
      setIsLoading(false)
    }
  }

  return (
    <AuthLayout>
      <Card className="glass border-border/50">
        <CardHeader className="space-y-1">
          <CardTitle className="text-2xl font-semibold">Reset Password</CardTitle>
          <CardDescription>Choose a new password for your account.</CardDescription>
        </CardHeader>
        <CardContent>
          {isSuccess ? (
            <div className="space-y-4" role="status">
              <div className="flex items-start gap-3 rounded-lg border border-border/50 bg-muted/30 p-4">
                <CheckCircle2 className="mt-0.5 h-5 w-5 shrink-0 text-primary" />
                <div className="space-y-1 text-sm">
                  <p className="font-medium">Password updated</p>
                  <p className="text-muted-foreground">
                    Sign in with your new password. Other sessions have been signed out.
                  </p>
                </div>
              </div>
              <Button variant="glow" className="w-full" asChild>
                <a href="/dashboard/login.html">Go to sign in</a>
              </Button>
            </div>
          ) : (
            <form onSubmit={handleSubmit} className="space-y-4">
              {!linkToken && (
                <div className="space-y-2">
                  <Label htmlFor="reset-token">Reset token</Label>
                  <div className="relative">
                    <KeyRound className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                    <Input
                      id="reset-token"
                      type="text"
                      placeholder="Paste the token from your reset link"
                      value={manualToken}
                      onChange={(e) => setManualToken(e.target.value)}
                      className="pl-10 font-mono"
                      required
                      autoComplete="off"
                      spellCheck={false}
                      autoFocus
                    />
                  </div>
                </div>
              )}

              <div className="space-y-2">
                <Label htmlFor="new-password">New Password</Label>
                <div className="relative">
                  <Lock className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                  <Input
                    id="new-password"
                    type="password"
                    placeholder="Enter a new password"
                    value={newPassword}
                    onChange={(e) => setNewPassword(e.target.value)}
                    className="pl-10"
                    required
                    autoComplete="new-password"
                    autoFocus={Boolean(linkToken)}
                    aria-describedby={newPassword ? 'new-password-strength' : undefined}
                  />
                </div>
                <PasswordStrengthMeter id="new-password-strength" password={newPassword} />
              </div>

              <div className="space-y-2">
                <Label htmlFor="confirm-password">Confirm Password</Label>
                <div className="relative">
                  <Lock className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                  <Input
                    id="confirm-password"
                    type="password"
                    placeholder="Confirm new password"
                    value={confirmPassword}
                    onChange={(e) => setConfirmPassword(e.target.value)}
                    className="pl-10"
                    required
                    autoComplete="new-password"
                  />
                </div>
              </div>

              <Button
                type="submit"
                variant="glow"
                className="w-full"
                disabled={isLoading || !token || !newPassword || !confirmPassword}
              >
                {isLoading ? <PendingLabel>Resetting password...</PendingLabel> : 'Reset Password'}
              </Button>
            </form>
          )}

          {!isSuccess && (
            <div className="mt-4 text-center text-sm text-muted-foreground">
              Need a new link?{' '}
              <a href="/dashboard/forgot-password.html" className="text-primary hover:underline">
                Request password reset
              </a>
            </div>
          )}
        </CardContent>
      </Card>
    </AuthLayout>
  )
}
