import type {
  NotificationLanguage,
  NotificationSettingsResponse,
  NotificationState,
} from '@/lib/api'
import { useNotificationSettingsViewModel } from '@/hooks/useNotificationSettingsViewModel'
import { Badge, type BadgeProps } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { AlertTriangle, Bell, Loader2 } from 'lucide-react'

const STATE_BADGES: Record<NotificationState, { label: string; variant: BadgeProps['variant'] }> = {
  active: { label: 'Active', variant: 'success' },
  disabled: { label: 'Disabled', variant: 'secondary' },
  unavailable: { label: 'Unavailable', variant: 'offline' },
}

interface NotificationSettingsModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function NotificationSettingsModal({ open, onOpenChange }: NotificationSettingsModalProps) {
  const {
    data,
    isLoading,
    error,
    refetch,
    form,
    isDirty,
    portValid,
    timeValid,
    isSaving,
    edit,
    handleOpenChange,
    handleSave,
  } = useNotificationSettingsViewModel({ open, onOpenChange })

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Bell className="h-5 w-5" />
            Notifications
          </DialogTitle>
          <DialogDescription>
            Email a daily digest of every endpoint&apos;s status to the administrators.
          </DialogDescription>
        </DialogHeader>

        {isLoading ? (
          <div role="status" aria-label="Loading" aria-busy="true" className="space-y-3 py-4">
            <Skeleton className="h-20" />
            <Skeleton className="h-10" />
            <Skeleton className="h-10" />
            <Skeleton className="h-10" />
          </div>
        ) : !data || !form ? (
          <div className="flex flex-col items-center gap-3 py-8 text-center">
            <AlertTriangle className="h-8 w-8 text-destructive" />
            <p className="font-medium">Failed to load notification settings</p>
            {error && <p className="text-sm text-muted-foreground">{error.message}</p>}
            <Button variant="outline" onClick={() => refetch()}>
              Retry
            </Button>
          </div>
        ) : (
          <div className="space-y-5 py-2">
            <NotificationStatusPanel data={data} />

            {/* Locked while saving: an edit made then would be dropped when the save returns. */}
            <fieldset disabled={isSaving} className="min-w-0 space-y-4">
              <div className="flex items-center justify-between gap-4">
                <Label htmlFor="notifications-enabled">Enable notifications</Label>
                <Switch
                  id="notifications-enabled"
                  disabled={isSaving}
                  checked={form.enabled}
                  onCheckedChange={(enabled) => edit({ enabled })}
                />
              </div>

              <div className="grid gap-4 sm:grid-cols-2">
                <div className="space-y-2">
                  <Label htmlFor="notifications-digest-time">Daily digest time</Label>
                  <span
                    id="notifications-digest-time-hint"
                    className="ml-2 text-xs leading-none text-muted-foreground"
                  >
                    Server local time
                  </span>
                  <Input
                    id="notifications-digest-time"
                    type="time"
                    aria-invalid={!timeValid}
                    aria-describedby={
                      timeValid ? 'notifications-digest-time-hint' : 'notifications-digest-time-error'
                    }
                    className="dark:[color-scheme:dark]"
                    value={form.daily_digest_time}
                    onChange={(e) => edit({ daily_digest_time: e.target.value })}
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="notifications-language">Email language</Label>
                  <Select
                    value={form.language}
                    disabled={isSaving}
                    onValueChange={(language) => edit({ language: language as NotificationLanguage })}
                  >
                    <SelectTrigger id="notifications-language">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="ja">Japanese</SelectItem>
                      <SelectItem value="en">English</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              </div>
              {!timeValid && (
                <p id="notifications-digest-time-error" role="alert" className="text-xs text-destructive">
                  Enter a time as HH:MM.
                </p>
              )}

              <div className="grid gap-4 sm:grid-cols-[1fr_6rem_1fr]">
                <div className="space-y-2">
                  <Label htmlFor="notifications-smtp-host">SMTP host</Label>
                  <Input
                    id="notifications-smtp-host"
                    placeholder="smtp.example.com"
                    autoComplete="off"
                    value={form.smtp_host}
                    onChange={(e) => edit({ smtp_host: e.target.value })}
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="notifications-smtp-port">SMTP port</Label>
                  <Input
                    id="notifications-smtp-port"
                    type="number"
                    inputMode="numeric"
                    min={1}
                    max={65535}
                    aria-invalid={!portValid}
                    aria-describedby={
                      portValid ? 'notifications-smtp-port-hint' : 'notifications-smtp-port-error'
                    }
                    value={form.smtp_port}
                    onChange={(e) => edit({ smtp_port: e.target.value })}
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="notifications-smtp-from">From address</Label>
                  <Input
                    id="notifications-smtp-from"
                    type="email"
                    placeholder="llmlb@example.com"
                    autoComplete="off"
                    value={form.smtp_from}
                    onChange={(e) => edit({ smtp_from: e.target.value })}
                  />
                </div>
              </div>
              {portValid ? (
                <p id="notifications-smtp-port-hint" className="text-xs text-muted-foreground">
                  TLS is required: port 465 connects with TLS, any other port uses STARTTLS.
                </p>
              ) : (
                <p id="notifications-smtp-port-error" role="alert" className="text-xs text-destructive">
                  Enter a port between 1 and 65535.
                </p>
              )}
            </fieldset>

            <div role="group" aria-labelledby="notifications-recipients-heading" className="space-y-2">
              <h3 id="notifications-recipients-heading" className="text-sm font-medium">
                Recipients
              </h3>
              {data.recipients.length === 0 ? (
                <p className="text-sm text-muted-foreground">
                  No administrator has a notification email. Set one in Manage Users.
                </p>
              ) : (
                <>
                  <p className="text-xs text-muted-foreground">
                    Administrators with a notification email. Change addresses in Manage Users.
                  </p>
                  <ul className="divide-y rounded-md border text-sm">
                    {data.recipients.map((recipient) => (
                      <li
                        key={`${recipient.username}:${recipient.email}`}
                        className="flex flex-wrap items-center justify-between gap-x-4 gap-y-1 px-3 py-2"
                      >
                        <span className="font-medium">{recipient.username}</span>
                        <span className="break-all text-muted-foreground">{recipient.email}</span>
                      </li>
                    ))}
                  </ul>
                </>
              )}
            </div>
          </div>
        )}

        <DialogFooter>
          <Button variant="outline" onClick={() => handleOpenChange(false)}>
            Close
          </Button>
          <Button
            onClick={handleSave}
            disabled={!isDirty || !portValid || !timeValid || isSaving}
          >
            {isSaving && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function NotificationStatusPanel({ data }: { data: NotificationSettingsResponse }) {
  const { status } = data
  const badge = STATE_BADGES[status.state]

  return (
    <div
      role="group"
      aria-label="Notification status"
      className="space-y-2 rounded-md border bg-muted/30 p-3 text-sm"
    >
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant={badge.variant}>{badge.label}</Badge>
        {status.reason &&
          (status.state === 'unavailable' ? (
            <span role="alert" className="flex items-center gap-1.5 text-destructive">
              <AlertTriangle className="h-4 w-4 shrink-0" />
              {status.reason}
            </span>
          ) : (
            <span className="text-muted-foreground">{status.reason}</span>
          ))}
      </div>
      <p className="text-xs text-muted-foreground">
        {data.credentials_configured
          ? 'SMTP credentials are set in the server environment.'
          : 'SMTP credentials are not set. Set LLMLB_SMTP_USERNAME and LLMLB_SMTP_PASSWORD in the server environment.'}
      </p>
      <p className="text-xs text-muted-foreground">
        {`Last digest sent: ${data.last_digest_sent_date ?? 'never'}`}
      </p>
      {data.last_digest_error && (
        <p className="break-words text-xs text-destructive">
          {`Last delivery failure: ${data.last_digest_error}`}
        </p>
      )}
    </div>
  )
}
