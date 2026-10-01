import { test, expect, type Page, type TestInfo } from '@playwright/test'
import { ensureDashboardLogin, createUser, deleteUser } from '../../helpers/api-helpers'

interface NotificationSettings {
  enabled: boolean
  smtp_host: string
  smtp_port: number
  smtp_from: string
  daily_digest_time: string
  language: 'ja' | 'en'
}

interface NotificationSettingsResponse {
  settings: NotificationSettings
  status: { state: string; reason: string | null }
  recipients: { username: string; email: string }[]
}

const EMAIL_HINT =
  'Destination for operational notifications, which are sent to administrators only. It is not a login identifier: users sign in with their username.'

// The settings API accepts the dashboard session only (JWT cookie + CSRF token),
// so the test calls it from the signed-in page.
async function notificationsApi(
  page: Page,
  settings?: NotificationSettings,
): Promise<NotificationSettingsResponse> {
  return page.evaluate(async (body) => {
    const csrf = document.cookie.match(/(?:^|; )llmlb_csrf=([^;]*)/)?.[1]
    const response = await fetch('/api/dashboard/notifications', {
      method: body ? 'PUT' : 'GET',
      credentials: 'include',
      headers: {
        'Content-Type': 'application/json',
        ...(csrf ? { 'X-CSRF-Token': decodeURIComponent(csrf) } : {}),
      },
      body: body ? JSON.stringify(body) : undefined,
    })
    if (!response.ok) throw new Error(`notifications API returned ${response.status}`)
    return response.json()
  }, settings)
}

async function attachScreenshot(page: Page, testInfo: TestInfo, name: string) {
  await testInfo.attach(name, { body: await page.screenshot(), contentType: 'image/png' })
}

async function openUserMenuItem(page: Page, name: string) {
  await page.getByRole('button', { name: 'User menu' }).click()
  await page.getByRole('menuitem', { name }).click()
}

// Both themes change the same server-wide settings, so they must not overlap.
test.describe.configure({ mode: 'serial' })

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Notification settings (${colorScheme}) @dashboard`, () => {
    test.use({ colorScheme })

    test('admin sets a notification email and configures the daily digest (#777)', async ({
      page,
      request,
    }, testInfo) => {
      test.setTimeout(120_000)

      const suffix = `${Date.now()}-${Math.random().toString(16).slice(2)}`
      const username = `e2e-notify-${suffix}`
      const email = `${username}@example.com`

      const pageErrors: string[] = []
      // The pre-login session probe returns 401 by design; only collect errors after login.
      let loggedIn = false
      page.on('pageerror', (err) => {
        if (loggedIn) pageErrors.push(err.message)
      })
      page.on('console', (msg) => {
        if (loggedIn && msg.type() === 'error') pageErrors.push(msg.text())
      })

      const created = await createUser(request, username, '', 'admin')
      expect(created.id).not.toBe('')
      let original: NotificationSettings | undefined

      try {
        await ensureDashboardLogin(page)
        loggedIn = true
        if (colorScheme === 'dark') {
          await expect(page.locator('html')).toHaveClass(/\bdark\b/)
        } else {
          await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)
        }

        // Known starting point: turned off, no SMTP host. Enabling it below can
        // therefore never send mail.
        original = (await notificationsApi(page)).settings
        await notificationsApi(page, {
          enabled: false,
          smtp_host: '',
          smtp_port: 587,
          smtp_from: '',
          daily_digest_time: '09:00',
          language: 'ja',
        })

        // AC-2: the email is set in Manage Users and described as a notification destination.
        await openUserMenuItem(page, 'Manage Users')
        const users = page.getByRole('dialog', { name: 'User Management' })
        const row = users.getByRole('row').filter({ hasText: username })
        await expect(row).toContainText('Not set')

        await row.getByRole('button', { name: `Edit ${username}` }).click()
        const edit = page.getByRole('dialog', { name: 'Edit User' })
        const emailField = edit.getByLabel('Notification email (optional)')
        await expect(emailField).toHaveValue('')
        await expect(emailField).toHaveAccessibleDescription(EMAIL_HINT)
        await expect(edit.getByText(EMAIL_HINT)).toBeVisible()
        await emailField.fill(email)
        await edit.getByRole('button', { name: 'Update' }).click()

        await expect(edit).toBeHidden()
        await expect(row).toContainText(email)
        // The row actions stay reachable next to a long address.
        await expect(row.getByRole('button', { name: `Delete ${username}` })).toBeInViewport({ ratio: 1 })
        await attachScreenshot(page, testInfo, `manage-users-${colorScheme}`)
        await users.getByRole('button', { name: 'Close' }).click()
        await expect(users).toBeHidden()

        // AC-5: status, recipients and the editable settings.
        await openUserMenuItem(page, 'Notifications')
        const dialog = page.getByRole('dialog', { name: 'Notifications' })
        const status = dialog.getByRole('group', { name: 'Notification status' })
        const enabled = dialog.getByRole('switch', { name: 'Enable notifications' })
        const save = dialog.getByRole('button', { name: 'Save' })

        await expect(status.getByText('Disabled', { exact: true })).toBeVisible()
        await expect(enabled).not.toBeChecked()
        await expect(save).toBeDisabled()
        await expect(
          dialog.getByRole('group', { name: 'Recipients' }).getByRole('listitem').filter({ hasText: username }),
        ).toContainText(email)

        await enabled.click()
        await dialog.getByLabel('Daily digest time').fill('18:30')
        await dialog.getByRole('combobox', { name: 'Email language' }).click()
        await page.getByRole('option', { name: 'English' }).click()
        await save.click()

        // AC-7: enabled without an SMTP host -> the reason is shown, nothing is sent.
        await expect(status.getByText('Unavailable', { exact: true })).toBeVisible()
        const stored = await notificationsApi(page)
        expect(stored.settings).toMatchObject({
          enabled: true,
          daily_digest_time: '18:30',
          language: 'en',
        })
        expect(stored.status.state).toBe('unavailable')
        expect(stored.status.reason).toBeTruthy()
        await expect(status.getByRole('alert')).toHaveText(stored.status.reason as string)
        expect(stored.recipients).toContainEqual({ username, email })
        await expect(save).toBeDisabled()
        // Exact match: the toast's screen-reader announcement briefly repeats the text.
        await expect(
          page.getByText(`Notifications cannot be sent yet: ${stored.status.reason}`, { exact: true }),
        ).toBeVisible()
        await status.scrollIntoViewIfNeeded()
        await attachScreenshot(page, testInfo, `notifications-${colorScheme}`)

        // The saved values come back from the server after a reload.
        await page.reload()
        await expect(page.locator('#theme-toggle')).toBeVisible({ timeout: 15000 })
        await openUserMenuItem(page, 'Notifications')
        await expect(enabled).toBeChecked()
        await expect(dialog.getByLabel('Daily digest time')).toHaveValue('18:30')
        await expect(dialog.getByRole('combobox', { name: 'Email language' })).toHaveText('English')
        await expect(status.getByRole('alert')).toHaveText(stored.status.reason as string)

        expect(pageErrors).toEqual([])
      } finally {
        // The settings are server-wide, so a failed restore must not pass silently.
        const removed = await deleteUser(request, created.id)
        if (original) await notificationsApi(page, original)
        expect(removed).toBe(true)
      }
    })
  })
}
