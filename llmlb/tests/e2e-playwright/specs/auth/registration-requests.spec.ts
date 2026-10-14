import { type Page } from '@playwright/test'
import {
  test, expect, REGISTRATION_PASSWORD, type RegistrationApi, type PendingRegistration,
} from '../../helpers/registration.fixture'
import { LoginPage } from '../../pages/auth.page'
import { DashboardPage } from '../../pages/dashboard.page'

async function login(page: Page, username: string, password: string) {
  const loginPage = new LoginPage(page)
  await loginPage.goto()
  await loginPage.login(username, password)
  await loginPage.waitForDashboard()
  await expect(page.locator('#theme-toggle')).toBeVisible()
}

async function openRequests(page: Page) {
  const dashboard = new DashboardPage(page)
  await dashboard.openManageUsersModal()
  await expect(dashboard.userModal.getByRole('region', { name: 'Pending registrations', exact: true })).toBeVisible()
  return dashboard.userModal
}

async function approve(page: Page, pending: PendingRegistration, role: 'admin' | 'viewer') {
  const row = page.getByTestId(`registration-request-${pending.id}`)
  await expect(row).toContainText(pending.username)
  await row.getByRole('button', { name: `Approve ${pending.username}`, exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Approve registration', exact: true })
  const roleSelect = dialog.getByRole('combobox', { name: 'Approval role', exact: true })
  await expect(roleSelect).toHaveText('Viewer')
  if (role === 'admin') {
    await roleSelect.click()
    await page.getByRole('option', { name: 'Admin', exact: true }).click()
  }
  const responsePromise = page.waitForResponse((response) =>
    new URL(response.url()).pathname === `/api/registration-requests/${pending.id}/approve` && response.request().method() === 'POST')
  await dialog.getByRole('button', { name: 'Approve', exact: true }).click()
  const response = await responsePromise
  expect(response.status()).toBe(201)
  expect(response.request().postDataJSON()).toEqual({ role })
  expect(await response.json()).toMatchObject({ username: pending.username, role })
  await expect(dialog).not.toBeVisible()
  await expect(row).toHaveCount(0)
}

async function editRole(page: Page, username: string, role: 'admin' | 'viewer') {
  const userModal = page.getByRole('dialog', { name: 'User Management', exact: true })
  const userRow = userModal.getByRole('row').filter({ has: page.getByRole('cell', { name: username, exact: true }) })
  await userRow.getByRole('button', { name: `Edit ${username}`, exact: true }).click()
  const edit = page.getByRole('dialog', { name: 'Edit User', exact: true })
  await edit.getByRole('combobox', { name: 'Role', exact: true }).click()
  await page.getByRole('option', { name: role === 'admin' ? 'Admin' : 'Viewer', exact: true }).click()
  const responsePromise = page.waitForResponse((response) =>
    /^\/api\/users\/[^/]+$/.test(new URL(response.url()).pathname) && response.request().method() === 'PUT')
  await edit.getByRole('button', { name: 'Update', exact: true }).click()
  const response = await responsePromise
  expect(response.status()).toBe(200)
  expect(response.request().postDataJSON()).toMatchObject({ role })
  await expect(edit).not.toBeVisible()
  await expect(userRow.getByText(role === 'admin' ? 'Admin' : 'Viewer', { exact: true })).toBeVisible()
}

async function expectLoggedInRole(page: Page, api: RegistrationApi, username: string, role: 'admin' | 'viewer') {
  await new DashboardPage(page).signOut()
  await login(page, username, REGISTRATION_PASSWORD)
  const me = await page.request.get('/api/auth/me')
  expect(me.status()).toBe(200)
  expect(await me.json()).toMatchObject({ username, role, must_change_password: false })
  expect((await api.users()).find((user) => user.username === username)?.role).toBe(role)
  await page.getByRole('button', { name: 'User menu', exact: true }).click()
  await expect(page.getByRole('menuitem').filter({ hasText: 'Manage Users' })).toHaveCount(role === 'admin' ? 1 : 0)
  await expect(page.getByRole('menuitem', { name: 'Invitation Codes', exact: true })).toHaveCount(0)
  await page.keyboard.press('Escape')
}

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Registration approval workflow (${colorScheme}) @auth @dashboard`, () => {
    test.use({ colorScheme })

    test('admin approves selected roles, rejects and edits users', async ({ page, registrationApi }, testInfo) => {
      test.setTimeout(150000)
      await login(page, 'admin', 'test')
      await expect(page.locator('html')).toHaveClass(colorScheme === 'dark' ? /\bdark\b/ : /^(?!.*\bdark\b)/)
      await page.getByRole('button', { name: 'User menu', exact: true }).click()
      await expect(page.getByRole('menuitem', { name: 'Invitation Codes', exact: true })).toHaveCount(0)
      await page.keyboard.press('Escape')
      const userModal = page.getByRole('dialog', { name: 'User Management', exact: true })

      const viewerName = registrationApi.username('viewer')
      const adminName = registrationApi.username('admin')
      const rejectedName = registrationApi.username('rejected')
      const viewer = await registrationApi.submit(viewerName)
      const admin = await registrationApi.submit(adminName)
      const rejected = await registrationApi.submit(rejectedName)
      for (const username of [viewerName, adminName, rejectedName]) await registrationApi.expectCannotLogin(username)
      await page.reload()
      await expect(page.locator('#theme-toggle')).toBeVisible()
      await page.getByRole('button', { name: 'User menu', exact: true }).click()
      await expect.poll(async () => {
        const displayed = await page.getByTestId('pending-registration-count').textContent()
        return Number(displayed) === (await registrationApi.pending()).length
      }, { timeout: 15000, message: 'pending badge must match the real request count' }).toBe(true)
      await page.getByRole('menuitem').filter({ hasText: 'Manage Users' }).click()
      await expect(userModal.getByRole('region', { name: 'Pending registrations', exact: true })).toBeVisible()
      for (const pending of [viewer, admin, rejected]) {
        await expect(page.getByTestId(`registration-request-${pending.id}`)).toContainText(pending.username)
        expect(Number.isNaN(Date.parse(pending.created_at))).toBe(false)
      }
      await testInfo.attach(`pending-registrations-${colorScheme}`, {
        body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png',
      })
      await approve(page, viewer, 'viewer')
      await approve(page, admin, 'admin')
      const remainingIds = (await registrationApi.pending()).map((item) => item.id)
      expect(remainingIds).not.toContain(viewer.id)
      expect(remainingIds).not.toContain(admin.id)
      const rejectedRow = page.getByTestId(`registration-request-${rejected.id}`)
      await rejectedRow.getByRole('button', { name: `Reject ${rejectedName}`, exact: true }).click()
      const rejectDialog = page.getByRole('alertdialog', { name: 'Reject registration', exact: true })
      const rejectedResponse = page.waitForResponse((response) =>
        new URL(response.url()).pathname === `/api/registration-requests/${rejected.id}` && response.request().method() === 'DELETE')
      await rejectDialog.getByRole('button', { name: 'Reject', exact: true }).click()
      expect((await rejectedResponse).status()).toBe(204)
      await expect(rejectedRow).toHaveCount(0)
      await registrationApi.expectCannotLogin(rejectedName)
      const resubmitted = await registrationApi.submit(rejectedName)
      expect(resubmitted.id).not.toBe(rejected.id)
      await page.reload()
      await expect(page.locator('#theme-toggle')).toBeVisible()
      await openRequests(page)
      await expect(page.getByTestId(`registration-request-${resubmitted.id}`)).toContainText(rejectedName)
      await editRole(page, viewerName, 'admin')
      expect((await registrationApi.users()).find((user) => user.username === viewerName)?.role).toBe('admin')
      await editRole(page, viewerName, 'viewer')
      await testInfo.attach(`approved-user-roles-${colorScheme}`, {
        body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png',
      })
      await userModal.getByRole('button', { name: 'Close', exact: true }).click()
      await expectLoggedInRole(page, registrationApi, viewerName, 'viewer')
      await expectLoggedInRole(page, registrationApi, adminName, 'admin')
      const oldEndpoint = await registrationApi.request.get('/api/invitations', { headers: registrationApi.headers })
      expect(oldEndpoint.status()).toBe(404)
    })
  })
}
