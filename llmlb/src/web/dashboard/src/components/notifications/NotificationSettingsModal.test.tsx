import { useState } from 'react'
import { screen, waitFor, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import {
  ApiError,
  notificationsApi,
  type NotificationSettings,
  type NotificationSettingsResponse,
} from '@/lib/api'
import { deferred, renderWithProviders } from '@/test/render'
import { NotificationSettingsModal } from './NotificationSettingsModal'

const settings: NotificationSettings = {
  enabled: true,
  smtp_host: 'smtp.example.com',
  smtp_port: 587,
  smtp_from: 'llmlb@example.com',
  daily_digest_time: '09:00',
  language: 'ja',
}

function response(overrides: Partial<NotificationSettingsResponse> = {}): NotificationSettingsResponse {
  return {
    settings,
    status: { state: 'active', reason: null },
    credentials_configured: true,
    recipients: [{ username: 'admin', email: 'ops@example.com' }],
    last_digest_sent_date: '2026-10-01',
    last_digest_error: null,
    ...overrides,
  }
}

function stubGet(overrides: Partial<NotificationSettingsResponse> = {}) {
  return vi.spyOn(notificationsApi, 'get').mockResolvedValue(response(overrides))
}

async function renderModal() {
  const view = renderWithProviders(<NotificationSettingsModal open onOpenChange={() => {}} />)
  await screen.findByRole('switch', { name: 'Enable notifications' })
  return view
}

function status() {
  return within(screen.getByRole('group', { name: 'Notification status' }))
}

function recipients() {
  return within(screen.getByRole('group', { name: 'Recipients' }))
}

async function fill(user: UserEvent, label: string, value: string) {
  const field = screen.getByLabelText(label)
  await user.clear(field)
  await user.click(field)
  await user.paste(value)
}

describe('NotificationSettingsModal', () => {
  it('does not request the settings while the dialog is closed', () => {
    const get = stubGet()
    renderWithProviders(<NotificationSettingsModal open={false} onOpenChange={() => {}} />)

    expect(get).not.toHaveBeenCalled()
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('switches from loading to the stored settings', async () => {
    const pending = deferred<NotificationSettingsResponse>()
    vi.spyOn(notificationsApi, 'get').mockReturnValue(pending.promise)
    renderWithProviders(<NotificationSettingsModal open onOpenChange={() => {}} />)

    expect(await screen.findByRole('status', { name: 'Loading' })).toBeInTheDocument()
    expect(screen.queryByRole('switch')).not.toBeInTheDocument()

    pending.resolve(response())

    expect(await screen.findByRole('switch', { name: 'Enable notifications' })).toBeChecked()
    expect(screen.queryByRole('status', { name: 'Loading' })).not.toBeInTheDocument()
    expect(screen.getByLabelText('Daily digest time')).toHaveValue('09:00')
    expect(screen.getByRole('combobox', { name: 'Email language' })).toHaveTextContent('Japanese')
    expect(screen.getByLabelText('SMTP host')).toHaveValue('smtp.example.com')
    expect(screen.getByLabelText('SMTP port')).toHaveValue(587)
    expect(screen.getByLabelText('From address')).toHaveValue('llmlb@example.com')
  })

  it('offers a retry when the settings cannot be loaded', async () => {
    const get = vi
      .spyOn(notificationsApi, 'get')
      .mockRejectedValueOnce(new ApiError(500, 'Internal Server Error', 'database is locked'))
      .mockResolvedValue(response())
    const user = userEvent.setup()
    renderWithProviders(<NotificationSettingsModal open onOpenChange={() => {}} />)

    expect(await screen.findByText('Failed to load notification settings')).toBeInTheDocument()
    expect(screen.getByText('database is locked')).toBeInTheDocument()
    expect(screen.queryByRole('switch')).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Retry' }))

    expect(await screen.findByRole('switch', { name: 'Enable notifications' })).toBeChecked()
    expect(get).toHaveBeenCalledTimes(2)
  })

  describe('status', () => {
    it('shows Active without a reason when notifications can be sent', async () => {
      stubGet()
      await renderModal()

      expect(status().getByText('Active')).toBeInTheDocument()
      expect(status().getByText('SMTP credentials are set in the server environment.')).toBeInTheDocument()
      expect(status().getByText('Last digest sent: 2026-10-01')).toBeInTheDocument()
      expect(status().queryByText(/Last delivery failure/)).not.toBeInTheDocument()
    })

    it('shows Disabled with the reason when an administrator turned notifications off', async () => {
      stubGet({
        settings: { ...settings, enabled: false },
        status: { state: 'disabled', reason: 'Notifications are turned off' },
      })
      await renderModal()

      expect(status().getByText('Disabled')).toBeInTheDocument()
      expect(status().getByText('Notifications are turned off')).toBeInTheDocument()
      expect(screen.getByRole('switch', { name: 'Enable notifications' })).not.toBeChecked()
    })

    it('shows why notifications are unavailable when the SMTP settings are missing or invalid', async () => {
      stubGet({
        status: { state: 'unavailable', reason: 'SMTP host is not configured' },
        credentials_configured: false,
        last_digest_sent_date: null,
      })
      await renderModal()

      expect(status().getByText('Unavailable')).toBeInTheDocument()
      expect(status().getByRole('alert')).toHaveTextContent('SMTP host is not configured')
      expect(
        status().getByText(
          'SMTP credentials are not set. Set LLMLB_SMTP_USERNAME and LLMLB_SMTP_PASSWORD in the server environment.',
        ),
      ).toBeInTheDocument()
      expect(status().getByText('Last digest sent: never')).toBeInTheDocument()
    })

    it('shows the most recent delivery failure', async () => {
      stubGet({ last_digest_error: '2026-10-01 09:00 connection refused' })
      await renderModal()

      expect(
        status().getByText('Last delivery failure: 2026-10-01 09:00 connection refused'),
      ).toBeInTheDocument()
    })
  })

  describe('recipients', () => {
    it('lists the administrators that have a notification email', async () => {
      stubGet({
        recipients: [
          { username: 'admin', email: 'ops@example.com' },
          { username: 'alice', email: 'alice@example.com' },
        ],
      })
      await renderModal()

      const items = recipients().getAllByRole('listitem')
      expect(items).toHaveLength(2)
      expect(items[0]).toHaveTextContent('admin')
      expect(items[0]).toHaveTextContent('ops@example.com')
      expect(items[1]).toHaveTextContent('alice')
      expect(items[1]).toHaveTextContent('alice@example.com')
      expect(
        recipients().getByText(
          'Administrators with a notification email. Change addresses in Manage Users.',
        ),
      ).toBeInTheDocument()
    })

    it('says where to set an address when no administrator has one', async () => {
      stubGet({ recipients: [] })
      await renderModal()

      expect(recipients().queryByRole('listitem')).not.toBeInTheDocument()
      expect(
        recipients().getByText(
          'No administrator has a notification email. Set one in Manage Users.',
        ),
      ).toBeInTheDocument()
    })
  })

  describe('saving', () => {
    it('keeps Save disabled until a setting was changed', async () => {
      stubGet()
      const user = userEvent.setup()
      await renderModal()

      expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled()

      await user.click(screen.getByRole('switch', { name: 'Enable notifications' }))

      expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled()
    })

    it('saves the edited settings and shows the status the server returned', async () => {
      stubGet({
        settings: { ...settings, enabled: false, smtp_host: '' },
        status: { state: 'disabled', reason: 'Notifications are turned off' },
      })
      const saved: NotificationSettings = {
        enabled: true,
        smtp_host: 'mail.example.org',
        smtp_port: 465,
        smtp_from: 'alerts@example.org',
        daily_digest_time: '18:30',
        language: 'en',
      }
      const update = vi
        .spyOn(notificationsApi, 'update')
        .mockResolvedValue(response({ settings: saved }))
      const user = userEvent.setup()
      await renderModal()

      await user.click(screen.getByRole('switch', { name: 'Enable notifications' }))
      await fill(user, 'Daily digest time', '18:30')
      await user.click(screen.getByRole('combobox', { name: 'Email language' }))
      await user.click(await screen.findByRole('option', { name: 'English' }))
      await fill(user, 'SMTP host', 'mail.example.org')
      await fill(user, 'SMTP port', '465')
      await fill(user, 'From address', 'alerts@example.org')
      await user.click(screen.getByRole('button', { name: 'Save' }))

      await waitFor(() => expect(update).toHaveBeenCalledExactlyOnceWith(saved))
      expect(await screen.findByText('Notification settings saved')).toBeInTheDocument()
      expect(await status().findByText('Active')).toBeInTheDocument()
      expect(status().queryByText('Disabled')).not.toBeInTheDocument()
      expect(screen.getByLabelText('SMTP host')).toHaveValue('mail.example.org')
      expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled()
    })

    it('reports the reason the server gives when it cannot send with the saved settings', async () => {
      stubGet({
        settings: { ...settings, enabled: false, smtp_host: '' },
        status: { state: 'disabled', reason: 'Notifications are turned off' },
      })
      vi.spyOn(notificationsApi, 'update').mockResolvedValue(
        response({
          settings: { ...settings, smtp_host: '' },
          status: { state: 'unavailable', reason: 'SMTP host is not configured' },
        }),
      )
      const user = userEvent.setup()
      await renderModal()

      await user.click(screen.getByRole('switch', { name: 'Enable notifications' }))
      await user.click(screen.getByRole('button', { name: 'Save' }))

      expect(await status().findByText('Unavailable')).toBeInTheDocument()
      expect(status().getByRole('alert')).toHaveTextContent('SMTP host is not configured')
      // The status panel can be scrolled out of view next to Save, so the
      // confirmation repeats the reason.
      expect(screen.getByText('Notification settings saved')).toBeInTheDocument()
      expect(
        screen.getByText('Notifications cannot be sent yet: SMTP host is not configured'),
      ).toBeInTheDocument()
    })

    it('keeps the edits and shows the server message when the save is rejected', async () => {
      stubGet()
      const update = vi
        .spyOn(notificationsApi, 'update')
        .mockRejectedValue(
          new ApiError(400, 'Bad Request', 'smtp_from must be a valid mail address'),
        )
      const user = userEvent.setup()
      await renderModal()

      await fill(user, 'From address', 'not-an-address')
      await user.click(screen.getByRole('button', { name: 'Save' }))

      expect(await screen.findByText('Failed to save notification settings')).toBeInTheDocument()
      expect(screen.getByText('smtp_from must be a valid mail address')).toBeInTheDocument()
      expect(update).toHaveBeenCalledExactlyOnceWith({ ...settings, smtp_from: 'not-an-address' })
      expect(screen.getByLabelText('From address')).toHaveValue('not-an-address')
      expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled()
    })

    it('keeps the saved settings when a refresh that started before the save finishes after it', async () => {
      const get = stubGet()
      const saved = { ...settings, smtp_host: 'mail.example.org' }
      vi.spyOn(notificationsApi, 'update').mockResolvedValue(response({ settings: saved }))
      const user = userEvent.setup()
      const { queryClient } = await renderModal()

      // A refresh (for example after a user was edited) that is still in flight.
      const staleRefresh = deferred<NotificationSettingsResponse>()
      get.mockReturnValue(staleRefresh.promise)
      void queryClient.invalidateQueries({ queryKey: ['notification-settings'] })
      await waitFor(() => expect(get).toHaveBeenCalledTimes(2))

      await fill(user, 'SMTP host', 'mail.example.org')
      await user.click(screen.getByRole('button', { name: 'Save' }))
      expect(await screen.findByText('Notification settings saved')).toBeInTheDocument()

      staleRefresh.resolve(response())
      await waitFor(() => expect(queryClient.isFetching()).toBe(0))

      expect(queryClient.getQueryData<NotificationSettingsResponse>(['notification-settings'])?.settings).toEqual(
        saved,
      )
      expect(screen.getByLabelText('SMTP host')).toHaveValue('mail.example.org')
      expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled()
    })

    it('locks the fields while a save is pending', async () => {
      stubGet()
      const pending = deferred<NotificationSettingsResponse>()
      vi.spyOn(notificationsApi, 'update').mockReturnValue(pending.promise)
      const user = userEvent.setup()
      await renderModal()

      await fill(user, 'SMTP host', 'mail.example.org')
      await user.click(screen.getByRole('button', { name: 'Save' }))

      await waitFor(() => expect(screen.getByLabelText('SMTP host')).toBeDisabled())
      expect(screen.getByRole('switch', { name: 'Enable notifications' })).toBeDisabled()
      expect(screen.getByLabelText('Daily digest time')).toBeDisabled()
      expect(screen.getByRole('combobox', { name: 'Email language' })).toBeDisabled()
      expect(screen.getByLabelText('SMTP port')).toBeDisabled()
      expect(screen.getByLabelText('From address')).toBeDisabled()
      expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled()

      pending.resolve(response({ settings: { ...settings, smtp_host: 'mail.example.org' } }))

      await waitFor(() => expect(screen.getByLabelText('SMTP host')).toBeEnabled())
      expect(screen.getByLabelText('SMTP host')).toHaveValue('mail.example.org')
    })

    it('saves the current stored value of a field that was not edited', async () => {
      const get = stubGet()
      const update = vi.spyOn(notificationsApi, 'update').mockResolvedValue(response())
      const user = userEvent.setup()
      const { queryClient } = await renderModal()

      await fill(user, 'Daily digest time', '18:30')

      // Another administrator turns notifications off while this dialog is open.
      get.mockResolvedValue(
        response({
          settings: { ...settings, enabled: false },
          status: { state: 'disabled', reason: 'Notifications are turned off' },
        }),
      )
      await queryClient.invalidateQueries({ queryKey: ['notification-settings'] })

      await waitFor(() =>
        expect(screen.getByRole('switch', { name: 'Enable notifications' })).not.toBeChecked(),
      )
      expect(screen.getByLabelText('Daily digest time')).toHaveValue('18:30')

      await user.click(screen.getByRole('button', { name: 'Save' }))

      await waitFor(() =>
        expect(update).toHaveBeenCalledExactlyOnceWith({
          ...settings,
          enabled: false,
          daily_digest_time: '18:30',
        }),
      )
    })

    it('discards unsaved edits when the dialog is closed', async () => {
      stubGet()
      function Harness() {
        const [open, setOpen] = useState(true)
        return (
          <>
            <button onClick={() => setOpen(true)}>Open notifications</button>
            <NotificationSettingsModal open={open} onOpenChange={setOpen} />
          </>
        )
      }
      const user = userEvent.setup()
      renderWithProviders(<Harness />)
      await screen.findByRole('switch', { name: 'Enable notifications' })

      await fill(user, 'SMTP host', 'mail.example.org')
      await user.keyboard('{Escape}')
      await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
      await user.click(screen.getByRole('button', { name: 'Open notifications' }))

      expect(await screen.findByLabelText('SMTP host')).toHaveValue('smtp.example.com')
      expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled()
    })

    it('rejects an empty digest time before saving', async () => {
      stubGet()
      const user = userEvent.setup()
      await renderModal()

      const field = screen.getByLabelText('Daily digest time')
      await user.clear(field)

      expect(field).toBeInvalid()
      expect(field).toHaveAccessibleDescription('Enter a time as HH:MM.')
      expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled()
    })

    it.each(['0', '65536', '', '25.5'])('rejects the SMTP port "%s" before saving', async (port) => {
      stubGet()
      const update = vi.spyOn(notificationsApi, 'update')
      const user = userEvent.setup()
      await renderModal()

      const field = screen.getByLabelText('SMTP port')
      await user.clear(field)
      if (port) {
        await user.click(field)
        await user.paste(port)
      }

      expect(field).toBeInvalid()
      expect(field).toHaveAccessibleDescription('Enter a port between 1 and 65535.')
      expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled()
      expect(update).not.toHaveBeenCalled()
    })
  })
})
