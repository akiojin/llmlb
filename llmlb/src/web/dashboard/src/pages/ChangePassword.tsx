import { useState, useEffect } from 'react'
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
import { KeyRound, Lock } from 'lucide-react'

const WRONG_CURRENT_PASSWORD_MESSAGE = 'Current password is incorrect'

export default function ChangePasswordPage() {
  const [currentPassword, setCurrentPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  const [isCheckingAuth, setIsCheckingAuth] = useState(true)
  const [mustChangePassword, setMustChangePassword] = useState(false)

  useEffect(() => {
    // Check if user is authenticated
    authApi.me()
      .then((me) => {
        setMustChangePassword(me.must_change_password)
        setIsCheckingAuth(false)
      })
      .catch(() => {
        window.location.href = '/dashboard/login.html'
      })
  }, [])

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
      await authApi.changePassword(currentPassword, newPassword)
      toast({
        title: 'Password changed',
        description: 'Please sign in with your new password',
      })
      // Redirect to login after short delay so toast is visible
      setTimeout(() => {
        window.location.href = '/dashboard/login.html'
      }, 1500)
    } catch (error) {
      const wrongCurrent =
        error instanceof ApiError && error.message === WRONG_CURRENT_PASSWORD_MESSAGE
      toast({
        variant: 'destructive',
        title: wrongCurrent ? WRONG_CURRENT_PASSWORD_MESSAGE : 'Failed to change password',
        description: wrongCurrent
          ? 'Enter the password you currently sign in with.'
          : 'Please try again',
      })
    } finally {
      setIsLoading(false)
    }
  }

  if (isCheckingAuth) {
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

  return (
    <AuthLayout>
      <Card className="glass border-border/50">
        <CardHeader className="space-y-1">
          <CardTitle className="text-2xl font-semibold">Change Password</CardTitle>
          <CardDescription>
            {mustChangePassword
              ? 'You must change your password before continuing.'
              : 'Update the password you use to sign in.'}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleSubmit} className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="current-password">Current Password</Label>
              <div className="relative">
                <KeyRound className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  id="current-password"
                  type="password"
                  placeholder="Enter your current password"
                  value={currentPassword}
                  onChange={(e) => setCurrentPassword(e.target.value)}
                  className="pl-10"
                  required
                  autoComplete="current-password"
                  autoFocus
                />
              </div>
            </div>

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
              disabled={isLoading || !currentPassword || !newPassword || !confirmPassword}
            >
              {isLoading ? <PendingLabel>Changing password...</PendingLabel> : 'Change Password'}
            </Button>
          </form>

          {!mustChangePassword && (
            <div className="mt-4 text-center text-sm text-muted-foreground">
              <a href="/dashboard/" className="text-primary hover:underline">
                Back to dashboard
              </a>
            </div>
          )}
        </CardContent>
      </Card>
    </AuthLayout>
  )
}
