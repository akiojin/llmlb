import { test, expect, type Locator } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'
import type { PendingRegistration } from '../../helpers/registration.fixture'
import type { ApiKey, User } from '../../../../src/web/dashboard/src/lib/api'

test.use({ trace: 'on' })

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Admin modal ViewModels (${colorScheme}) @dashboard`, () => {
    test.use({ colorScheme })

    test('preserves CRUD, form resets, secret lifetime and open-only polling (#866)', async ({ page }, testInfo) => {
      test.setTimeout(90000)
      const errors: string[] = []
      page.on('pageerror', (error) => errors.push(error.message))
      page.on('console', (message) => {
        if (message.type() === 'error') errors.push(message.text())
      })

      // Session creation and /api/auth/me remain real; only management data is controlled.
      const login = await page.request.post('/api/auth/login', {
        data: { username: 'admin', password: 'test' },
      })
      expect(login.ok()).toBe(true)
      const createdAt = '2026-10-01T00:00:00Z'
      const secret = 'sk_modal_viewmodel_plaintext_only_once'
      let keys: ApiKey[] = []
      const seedAdmin: User = { id: 'modal-seed-admin', username: 'admin', role: 'admin', email: null, created_at: createdAt }
      let users: User[] = [seedAdmin]
      let registrations: PendingRegistration[] = []
      const gets = { keys: 0, users: 0, registrations: 0, notifications: 0 }
      const keyPayloads: unknown[] = []
      const userPayloads: unknown[] = []
      const updates: unknown[] = []
      const approvalPayloads: unknown[] = []

      await page.route('**/api/me/api-keys', (route) => {
        if (route.request().method() === 'GET') {
          gets.keys++
          return route.fulfill({ json: { api_keys: keys } })
        }
        expect(route.request().method()).toBe('POST')
        expect(route.request().headers()['x-csrf-token']).toBeTruthy()
        const payload = route.request().postDataJSON()
        keyPayloads.push(payload)
        const key: ApiKey = {
          id: `key-${keyPayloads.length}`, name: payload.name, key_prefix: 'sk_modal',
          created_at: createdAt, permissions: payload.permissions,
        }
        keys = [key, ...keys]
        return route.fulfill({ json: { ...key, key: secret } })
      })
      await page.route('**/api/me/api-keys/*', (route) => {
        expect(route.request().method()).toBe('DELETE')
        expect(route.request().headers()['x-csrf-token']).toBeTruthy()
        keys = keys.filter((key) => !route.request().url().endsWith(`/${key.id}`))
        return route.fulfill({ status: 204 })
      })
      await page.route('**/api/users', (route) => {
        if (route.request().method() === 'GET') {
          gets.users++
          return route.fulfill({ json: { users } })
        }
        expect(route.request().method()).toBe('POST')
        expect(route.request().headers()['x-csrf-token']).toBeTruthy()
        const payload = route.request().postDataJSON()
        userPayloads.push(payload)
        const user: User = {
          id: 'modal-user', username: payload.username, role: payload.role,
          email: payload.email ?? null, created_at: createdAt,
        }
        users = [seedAdmin, user]
        return route.fulfill({ json: { user, generated_password: 'modal-generated-password' } })
      })
      await page.route('**/api/users/modal-user', (route) => {
        expect(route.request().headers()['x-csrf-token']).toBeTruthy()
        if (route.request().method() === 'DELETE') {
          users = users.filter((user) => user.id !== 'modal-user')
          return route.fulfill({ status: 204 })
        }
        expect(route.request().method()).toBe('PUT')
        const payload = route.request().postDataJSON()
        updates.push(payload)
        users = users.map((user) => user.id === 'modal-user' ? { ...user, ...payload, email: payload.email || null } : user)
        return route.fulfill({ json: users.find((user) => user.id === 'modal-user') })
      })
      await page.route('**/api/registration-requests', (route) => {
        expect(route.request().method()).toBe('GET')
        gets.registrations++
        return route.fulfill({ json: registrations })
      })
      await page.route('**/api/registration-requests/*/approve', (route) => {
        expect(route.request().method()).toBe('POST')
        expect(route.request().headers()['x-csrf-token']).toBeTruthy()
        const payload = route.request().postDataJSON()
        approvalPayloads.push(payload)
        const pending = registrations.find((item) => route.request().url().endsWith(`/${item.id}/approve`))!
        const user: User = { id: pending.id, username: pending.username, role: payload.role, email: null, created_at: createdAt }
        users = [...users, user]
        registrations = registrations.filter((item) => item.id !== pending.id)
        return route.fulfill({ status: 201, json: user })
      })
      await page.route('**/api/registration-requests/*', (route) => {
        expect(route.request().method()).toBe('DELETE')
        expect(route.request().headers()['x-csrf-token']).toBeTruthy()
        registrations = registrations.filter((item) => !route.request().url().endsWith(`/${item.id}`))
        return route.fulfill({ status: 204 })
      })
      await page.route('**/api/dashboard/notifications', (route) => {
        expect(route.request().method()).toBe('GET')
        gets.notifications++
        return route.fulfill({ json: {
          settings: { enabled: false, smtp_host: '', smtp_port: 587, smtp_from: '', daily_digest_time: '09:00', language: 'en' },
          status: { state: 'disabled', reason: null }, credentials_configured: false,
          recipients: users.filter((user) => user.role === 'admin' && user.email)
            .map((user) => ({ username: user.username, email: user.email })),
          last_digest_sent_date: null, last_digest_error: null,
        } })
      })

      await page.clock.install()
      await page.clock.pauseAt(Date.now() + 1000)
      await ensureDashboardLogin(page)
      if (colorScheme === 'dark') await expect(page.locator('html')).toHaveClass(/\bdark\b/)
      else await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)
      // Flush React Query's notification timers while the clock stays below polling boundaries.
      const visible = async (locator: Locator) => expect.poll(async () => {
        await page.clock.runFor(1)
        return locator.isVisible()
      }).toBe(true)
      const hidden = async (locator: Locator) => expect.poll(async () => {
        await page.clock.runFor(1)
        return locator.isVisible()
      }).toBe(false)
      const menu = async (name: string) => {
        await page.getByRole('button', { name: 'User menu' }).click()
        // The pending-count badge contributes to Manage Users' accessible name.
        await page.getByRole('menuitem').filter({ hasText: name }).click()
      }
      const screenshot = async (name: string) => testInfo.attach(`${name}-${colorScheme}`, {
        body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png',
      })
      await page.clock.runFor(5000)
      expect({ keys: gets.keys, users: gets.users, notifications: gets.notifications }).toEqual({ keys: 0, users: 0, notifications: 0 })
      expect(gets.registrations).toBeGreaterThan(0) // Header continuously tracks pending count.

      await page.locator('#api-keys-button').click()
      const keyModal = page.getByRole('dialog', { name: 'API Keys', exact: true })
      await visible(keyModal.getByText('No API keys', { exact: true }))
      expect(gets.keys).toBe(1)
      await keyModal.getByRole('button', { name: 'Create Key', exact: true }).click()
      const keyForm = page.getByRole('dialog', { name: 'Create API Key', exact: true })
      await keyForm.getByLabel('Name', { exact: true }).fill('discarded draft')
      await keyForm.getByRole('button', { name: 'Cancel', exact: true }).click()
      await keyModal.getByRole('button', { name: 'Create Key', exact: true }).click()
      await expect(keyForm.getByLabel('Name', { exact: true })).toHaveValue('')
      await keyForm.getByLabel('Name', { exact: true }).fill('modal-key')
      await keyForm.getByRole('button', { name: 'Create', exact: true }).click()
      await visible(keyModal.getByRole('cell', { name: 'modal-key', exact: true }))
      await hidden(keyForm)
      expect(keyPayloads).toEqual([{
        name: 'modal-key', permissions: ['openai.inference', 'openai.models.read'],
      }])
      expect(gets.keys).toBe(1) // Creation inserts the public row without a list refetch.
      await expect(keyModal.getByRole('table')).not.toContainText(secret)
      const createdKey = keyModal.getByText('API Key Created Successfully').locator('..')
      await createdKey.locator('button:not(#copy-api-key)').first().click()
      await expect(createdKey.locator('code')).toHaveText(secret)
      await page.clock.runFor(10000)
      expect(gets.keys).toBe(1) // The provider's five-second polling is explicitly disabled.
      await expect(createdKey.locator('code')).toHaveText(secret)
      await screenshot('api-key-created')
      const refreshedKeys = page.waitForResponse((response) =>
        new URL(response.url()).pathname === '/api/me/api-keys' && response.request().method() === 'GET')
      await keyModal.getByRole('button', { name: 'Refresh API keys', exact: true }).click()
      await hidden(createdKey)
      await (await refreshedKeys).finished()
      await page.clock.runFor(1)
      expect(gets.keys).toBe(2)
      await keyModal.getByRole('button', { name: 'Create Key', exact: true }).click()
      await keyForm.getByLabel('Name', { exact: true }).fill('close-secret-key')
      await keyForm.getByRole('button', { name: 'Create', exact: true }).click()
      await visible(createdKey)
      await keyModal.getByRole('button', { name: 'Close', exact: true }).click()
      await hidden(keyModal)
      await page.locator('#api-keys-button').click()
      await visible(keyModal)
      await expect(createdKey).toHaveCount(0)
      await expect(keyModal).not.toContainText(secret)
      const keyRow = keyModal.getByRole('row').filter({ hasText: 'close-secret-key' })
      await keyRow.getByRole('button').click()
      await page.getByRole('alertdialog', { name: 'Delete API Key' }).getByRole('button', { name: 'Delete', exact: true }).click()
      await hidden(keyRow)
      await visible(keyModal)
      await expect(keyModal.getByRole('cell', { name: 'close-secret-key', exact: true })).toHaveCount(0)
      await keyModal.getByRole('button', { name: 'Close', exact: true }).click()
      await hidden(keyModal)

      // Prime a fresh notifications cache. Creating an admin must invalidate it
      // before the five-second staleTime or polling interval can explain a GET.
      await menu('Notifications')
      const notifications = page.getByRole('dialog', { name: 'Notifications', exact: true })
      await visible(notifications.getByRole('group', { name: 'Recipients' }))
      expect(gets.notifications).toBe(1)
      const primedAt = await page.evaluate(() => Date.now())
      // The footer action precedes DialogContent's accessible corner close button.
      await notifications.getByRole('button', { name: 'Close', exact: true }).first().click()
      await hidden(notifications)
      await menu('Manage Users')
      const userModal = page.getByRole('dialog', { name: 'User Management', exact: true })
      await visible(userModal.getByRole('cell', { name: 'admin', exact: true }))
      expect(gets.users).toBe(1)
      await userModal.getByRole('button', { name: 'Add User', exact: true }).click()
      const userForm = page.getByRole('dialog', { name: 'Create User', exact: true })
      await userForm.getByLabel('Email', { exact: true }).fill('draft@example.com')
      await userForm.getByRole('button', { name: 'Cancel', exact: true }).click()
      await userModal.getByRole('button', { name: 'Add User', exact: true }).click()
      await expect(userForm.getByLabel('Email', { exact: true })).toHaveValue('')
      await expect(userForm.getByRole('combobox')).toHaveText('Viewer')
      await userForm.getByLabel('Email', { exact: true }).fill('modal-admin@example.com')
      await userForm.getByLabel('Notification email (optional)').fill(' ops@example.com ')
      await userForm.getByRole('combobox').click()
      await page.getByRole('option', { name: 'Admin', exact: true }).click()
      await userForm.getByRole('button', { name: 'Create', exact: true }).click()
      const password = page.getByRole('dialog', { name: 'User Created Successfully', exact: true })
      await visible(password.getByText('modal-generated-password', { exact: true }))
      await expect.poll(async () => { await page.clock.runFor(1); return gets.users }).toBe(2)
      expect(userPayloads).toEqual([{ username: 'modal-admin@example.com', role: 'admin', email: 'ops@example.com' }])
      await screenshot('user-created')
      await password.getByRole('button', { name: 'I have saved the password', exact: true }).click()
      await hidden(password)
      await visible(userModal.getByRole('cell', { name: 'modal-admin@example.com', exact: true }))
      await userModal.getByRole('button', { name: 'Close', exact: true }).click()
      await hidden(userModal)
      await menu('Notifications')
      await visible(notifications.getByRole('listitem').filter({ hasText: 'ops@example.com' }))
      expect(gets.notifications).toBe(2)
      expect(await page.evaluate(() => Date.now()) - primedAt).toBeLessThan(5000)
      await notifications.getByRole('button', { name: 'Close', exact: true }).first().click()
      await hidden(notifications)
      await menu('Manage Users')
      const userRow = userModal.getByRole('row').filter({ hasText: 'modal-admin@example.com' })
      await visible(userRow)
      const beforeUserPoll = gets.users
      users = users.map((user) => user.id === 'modal-user' ? { ...user, email: 'polled@example.com' } : user)
      await page.clock.runFor(4900)
      expect(gets.users).toBe(beforeUserPoll)
      await expect(userRow).toContainText('ops@example.com')
      await page.clock.runFor(200)
      await expect.poll(async () => { await page.clock.runFor(1); return gets.users }).toBe(beforeUserPoll + 1)
      await visible(userRow.getByText('polled@example.com', { exact: true }))
      await userRow.getByRole('button', { name: 'Edit modal-admin@example.com', exact: true }).click()
      const editUser = page.getByRole('dialog', { name: 'Edit User', exact: true })
      await expect(editUser.getByLabel('Notification email (optional)')).toHaveValue('polled@example.com')
      await editUser.getByLabel('Notification email (optional)').fill('')
      await editUser.getByRole('button', { name: 'Update', exact: true }).click()
      await hidden(editUser)
      await visible(userRow.getByText('Not set', { exact: true }))
      expect(updates).toEqual([{ email: '' }])
      await userRow.getByRole('button', { name: 'Delete modal-admin@example.com', exact: true }).click()
      await page.getByRole('alertdialog', { name: 'Delete User' }).getByRole('button', { name: 'Delete', exact: true }).click()
      await hidden(userRow)
      await visible(userModal.getByRole('cell', { name: 'admin', exact: true }))
      await expect(page.getByText('modal-generated-password', { exact: true })).toHaveCount(0)
      await userModal.getByRole('button', { name: 'Close', exact: true }).click()
      await hidden(userModal)
      const closedUsers = gets.users
      await page.clock.runFor(6000)
      expect(gets.users).toBe(closedUsers)

      registrations = [
        { id: 'pending-approve', username: 'approved@example.com', created_at: createdAt },
        { id: 'pending-reject', username: 'rejected@example.com', created_at: createdAt },
      ]
      await page.clock.runFor(5100)
      await menu('Manage Users')
      const pendingSection = userModal.getByRole('region', { name: 'Pending registrations', exact: true })
      const pendingApproval = pendingSection.getByTestId('registration-request-pending-approve')
      await visible(pendingApproval)
      await expect(page.getByRole('menuitem', { name: 'Invitation Codes', exact: true })).toHaveCount(0)
      await pendingApproval.getByRole('button', { name: 'Approve approved@example.com', exact: true }).click()
      const approval = page.getByRole('dialog', { name: 'Approve registration', exact: true })
      await expect(approval.getByRole('combobox', { name: 'Approval role', exact: true })).toHaveText('Viewer')
      await approval.getByRole('combobox', { name: 'Approval role', exact: true }).click()
      await page.getByRole('option', { name: 'Admin', exact: true }).click()
      await approval.getByRole('button', { name: 'Approve', exact: true }).click()
      await hidden(approval)
      await hidden(pendingApproval)
      await visible(userModal.getByRole('cell', { name: 'approved@example.com', exact: true }))
      expect(approvalPayloads).toEqual([{ role: 'admin' }])
      await screenshot('registration-approved')
      const pendingRejection = pendingSection.getByTestId('registration-request-pending-reject')
      await pendingRejection.getByRole('button', { name: 'Reject rejected@example.com', exact: true }).click()
      await page.getByRole('alertdialog', { name: 'Reject registration', exact: true }).getByRole('button', { name: 'Reject', exact: true }).click()
      await hidden(pendingRejection)
      await visible(pendingSection.getByText('No pending registrations', { exact: true }))
      await userModal.getByRole('button', { name: 'Close', exact: true }).click()
      await hidden(userModal)
      const closedKeys = gets.keys
      const usersAfterRegistration = gets.users
      // Hold the next poll until the virtual clock has finished advancing.
      let releaseRegistrationPoll!: () => void
      const registrationPollGate = new Promise<void>((resolve) => { releaseRegistrationPoll = resolve })
      await page.route('**/api/registration-requests', async (route) => {
        expect(route.request().method()).toBe('GET')
        await registrationPollGate
        gets.registrations++
        await route.fulfill({ json: registrations })
      })
      const registrationsWhileClosed = gets.registrations
      const registrationPoll = page.waitForResponse((response) =>
        new URL(response.url()).pathname === '/api/registration-requests' && response.request().method() === 'GET')
      await page.clock.runFor(6000)
      releaseRegistrationPoll()
      await (await registrationPoll).finished()
      expect(gets.registrations).toBeGreaterThan(registrationsWhileClosed) // Header remains live.
      expect(gets.users).toBe(usersAfterRegistration)
      expect(gets.keys).toBe(closedKeys)
      expect(errors).toEqual([])
    })
  })
}
