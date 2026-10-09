import { test, expect, type Locator } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'
import type { ApiKey, User, Invitation } from '../../../../src/web/dashboard/src/lib/api'

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
      const expiresAt = '2099-01-01T00:00:00Z'
      const secret = 'sk_modal_viewmodel_plaintext_only_once'
      let keys: ApiKey[] = []
      let users: User[] = []
      let invitations: Invitation[] = []
      const gets = { keys: 0, users: 0, invitations: 0, notifications: 0 }
      const keyPayloads: unknown[] = []
      const userPayloads: unknown[] = []
      const updates: unknown[] = []
      const invitationPayloads: unknown[] = []

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
        users = [user]
        return route.fulfill({ json: { user, generated_password: 'modal-generated-password' } })
      })
      await page.route('**/api/users/modal-user', (route) => {
        expect(route.request().headers()['x-csrf-token']).toBeTruthy()
        if (route.request().method() === 'DELETE') {
          users = []
          return route.fulfill({ status: 204 })
        }
        expect(route.request().method()).toBe('PUT')
        const payload = route.request().postDataJSON()
        updates.push(payload)
        users = users.map((user) => ({ ...user, ...payload, email: payload.email || null }))
        return route.fulfill({ json: users[0] })
      })
      await page.route('**/api/invitations', (route) => {
        if (route.request().method() === 'GET') {
          gets.invitations++
          return route.fulfill({ json: { invitations } })
        }
        expect(route.request().method()).toBe('POST')
        expect(route.request().headers()['x-csrf-token']).toBeTruthy()
        invitationPayloads.push(route.request().postDataJSON())
        const invitation: Invitation = {
          id: '22222222-3333-4444-5555-666666666666', created_by: 'admin',
          created_at: createdAt, expires_at: expiresAt, status: 'active',
        }
        invitations = [invitation]
        return route.fulfill({ json: { ...invitation, code: 'inv_modal_once_only' } })
      })
      await page.route('**/api/invitations/*', (route) => {
        expect(route.request().method()).toBe('DELETE')
        expect(route.request().headers()['x-csrf-token']).toBeTruthy()
        invitations = invitations.map((invitation) => ({ ...invitation, status: 'revoked' }))
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
        await page.getByRole('menuitem', { name, exact: true }).click()
      }
      const screenshot = async (name: string) => testInfo.attach(`${name}-${colorScheme}`, {
        body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png',
      })
      await page.clock.runFor(5000)
      expect(gets).toEqual({ keys: 0, users: 0, invitations: 0, notifications: 0 })

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
      await visible(userModal.getByText('No users', { exact: true }))
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
      users = users.map((user) => ({ ...user, email: 'polled@example.com' }))
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
      await visible(userModal.getByText('No users', { exact: true }))
      await expect(page.getByText('modal-generated-password', { exact: true })).toHaveCount(0)
      await userModal.getByRole('button', { name: 'Close', exact: true }).click()
      await hidden(userModal)
      const closedUsers = gets.users
      await page.clock.runFor(6000)
      expect(gets.users).toBe(closedUsers)

      await menu('Invitation Codes')
      const invitationModal = page.getByRole('dialog', { name: 'Invitation Codes', exact: true })
      await visible(invitationModal.getByText('No invitation codes', { exact: true }))
      expect(gets.invitations).toBe(1)
      await page.clock.runFor(4900)
      expect(gets.invitations).toBe(1)
      await page.clock.runFor(200)
      await expect.poll(async () => { await page.clock.runFor(1); return gets.invitations }).toBe(2)
      await invitationModal.getByRole('button', { name: 'Create Code', exact: true }).click()
      const invitationForm = page.getByRole('dialog', { name: 'Create Invitation Code', exact: true })
      await expect(invitationForm.getByRole('combobox')).toHaveText('72 hours (3 days)')
      await invitationForm.getByRole('button', { name: 'Create', exact: true }).click()
      const codeDialog = page.getByRole('dialog', { name: 'Invitation Code Created', exact: true })
      await visible(codeDialog.getByText('inv_modal_once_only', { exact: true }))
      expect(invitationPayloads).toEqual([{ expires_in_hours: 72 }])
      await expect.poll(async () => { await page.clock.runFor(1); return gets.invitations }).toBe(3)
      await screenshot('invitation-created')
      await codeDialog.getByRole('button', { name: 'Done', exact: true }).click()
      await hidden(codeDialog)
      const invitationRow = invitationModal.getByRole('row').filter({ hasText: '22222222...' })
      await visible(invitationRow.getByText('Active', { exact: true }))
      await invitationRow.getByRole('button').click()
      await page.getByRole('alertdialog', { name: 'Revoke Invitation Code' }).getByRole('button', { name: 'Revoke', exact: true }).click()
      await visible(invitationRow.getByText('Revoked', { exact: true }))
      await expect(invitationRow.getByRole('button')).toHaveCount(0)
      await expect(page.getByText('inv_modal_once_only', { exact: true })).toHaveCount(0)
      await invitationModal.getByRole('button', { name: 'Close', exact: true }).click()
      await hidden(invitationModal)
      const closedInvitations = gets.invitations
      const closedKeys = gets.keys
      await page.clock.runFor(6000)
      expect(gets.invitations).toBe(closedInvitations)
      expect(gets.users).toBe(closedUsers)
      expect(gets.keys).toBe(closedKeys)
      expect(errors).toEqual([])
    })
  })
}
