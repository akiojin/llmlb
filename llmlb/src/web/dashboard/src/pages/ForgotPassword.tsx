import { useState } from 'react'
import { authApi } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { AuthLayout, PendingLabel } from '@/components/auth/AuthLayout'
import { toast } from '@/hooks/use-toast'
import { Mail, MailCheck } from 'lucide-react'

export default function ForgotPasswordPage() {
  const [email, setEmail] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  const [isSubmitted, setIsSubmitted] = useState(false)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setIsLoading(true)

    try {
      await authApi.forgotPassword(email)
      setIsSubmitted(true)
    } catch {
      toast({
        variant: 'destructive',
        title: 'Request failed',
        description: 'Could not request a password reset. Please try again.',
      })
    } finally {
      setIsLoading(false)
    }
  }

  return (
    <AuthLayout>
      <Card className="glass border-border/50">
        <CardHeader className="space-y-1">
          <CardTitle className="text-2xl font-semibold">Forgot Password</CardTitle>
          <CardDescription>
            Enter the email address you sign in with to request a reset link.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {isSubmitted ? (
            <div className="space-y-4" role="status">
              <div className="flex items-start gap-3 rounded-lg border border-border/50 bg-muted/30 p-4">
                <MailCheck className="mt-0.5 h-5 w-5 shrink-0 text-primary" />
                <div className="space-y-1 text-sm">
                  <p className="font-medium">Reset requested</p>
                  <p className="text-muted-foreground">
                    If an account exists for this email, a password reset link has been issued.
                    It is valid for 30 minutes. Ask your administrator for the link.
                  </p>
                </div>
              </div>
              <Button variant="outline" className="w-full" asChild>
                <a href="/dashboard/reset-password.html">I have a reset link</a>
              </Button>
            </div>
          ) : (
            <form onSubmit={handleSubmit} className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="email">Email</Label>
                <div className="relative">
                  <Mail className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                  <Input
                    id="email"
                    type="email"
                    placeholder="you@example.com"
                    value={email}
                    onChange={(e) => setEmail(e.target.value)}
                    className="pl-10"
                    required
                    autoComplete="username"
                    autoFocus
                  />
                </div>
              </div>

              <Button
                type="submit"
                variant="glow"
                className="w-full"
                disabled={isLoading || !email}
              >
                {isLoading ? <PendingLabel>Requesting...</PendingLabel> : 'Request reset link'}
              </Button>
            </form>
          )}

          <div className="mt-4 text-center text-sm text-muted-foreground">
            Remembered it?{' '}
            <a href="/dashboard/login.html" className="text-primary hover:underline">
              Back to sign in
            </a>
          </div>
        </CardContent>
      </Card>
    </AuthLayout>
  )
}
