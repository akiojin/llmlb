// Operational notifications API (SPEC #777)

import { fetchWithAuth } from './client'

export type NotificationLanguage = 'ja' | 'en'

export type NotificationState = 'active' | 'disabled' | 'unavailable'

/** Non-secret settings. The SMTP credentials are set in the server environment only. */
export interface NotificationSettings {
  enabled: boolean
  /** Empty when not set. */
  smtp_host: string
  smtp_port: number
  /** Empty when not set. */
  smtp_from: string
  /** Server local time, `HH:MM`. */
  daily_digest_time: string
  language: NotificationLanguage
}

export interface NotificationStatus {
  state: NotificationState
  /** Why notifications cannot be sent. `null` when `state` is `active`. */
  reason: string | null
}

export interface NotificationRecipient {
  username: string
  email: string
}

export interface NotificationSettingsResponse {
  settings: NotificationSettings
  status: NotificationStatus
  credentials_configured: boolean
  /** Administrators with a notification email. */
  recipients: NotificationRecipient[]
  /** `YYYY-MM-DD`, or `null` when no digest was sent yet. */
  last_digest_sent_date: string | null
  /** `YYYY-MM-DD HH:MM reason`, or `null` when the last delivery succeeded. */
  last_digest_error: string | null
}

export const notificationsApi = {
  get: () => fetchWithAuth<NotificationSettingsResponse>('/api/dashboard/notifications'),

  update: (settings: NotificationSettings) =>
    fetchWithAuth<NotificationSettingsResponse>('/api/dashboard/notifications', {
      method: 'PUT',
      body: JSON.stringify(settings),
    }),
}
