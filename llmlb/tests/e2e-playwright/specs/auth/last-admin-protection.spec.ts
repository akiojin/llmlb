import { test, expect } from '../../helpers/isolated-registration.fixture'
import { LoginPage } from '../../pages/auth.page'
import { DashboardPage } from '../../pages/dashboard.page'

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Last administrator protection (${colorScheme}) @auth @dashboard`, () => {
    test.use({ colorScheme })

    test('a real isolated server rejects demotion/deletion and disables both UI actions', async ({ page, registrationApi }, testInfo) => {
      const admins = (await registrationApi.users()).filter((user) => user.role === 'admin')
      expect(admins).toHaveLength(1)
      expect(admins[0].username).toBe('admin')
      const login = new LoginPage(page)
      await login.goto()
      await login.login('admin', 'test')
      await login.waitForDashboard()
      await expect(page.locator('#theme-toggle')).toBeVisible()
      await expect(page.locator('html')).toHaveClass(colorScheme === 'dark' ? /\bdark\b/ : /^(?!.*\bdark\b)/)
      await new DashboardPage(page).openManageUsersModal()
      const userModal = page.getByRole('dialog', { name: 'User Management', exact: true })
      const adminRow = userModal.getByRole('row').filter({ has: page.getByRole('cell', { name: 'admin', exact: true }) })
      await expect(adminRow.getByRole('button', { name: 'Delete admin', exact: true })).toBeDisabled()
      await adminRow.getByRole('button', { name: 'Edit admin', exact: true }).click()
      const edit = page.getByRole('dialog', { name: 'Edit User', exact: true })
      await edit.getByRole('combobox', { name: 'Role', exact: true }).click()
      await expect(page.getByRole('option', { name: 'Viewer', exact: true })).toHaveAttribute('aria-disabled', 'true')
      await testInfo.attach(`last-admin-protection-${colorScheme}`, {
        body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png',
      })
      await page.keyboard.press('Escape')
      await edit.getByRole('button', { name: 'Cancel', exact: true }).click()
      for (const method of ['put', 'delete'] as const) {
        const response = await registrationApi.request[method](`/api/users/${admins[0].id}`, {
          headers: registrationApi.headers,
          ...(method === 'put' ? { data: { role: 'viewer' } } : {}),
        })
        expect(response.status()).toBe(400)
      }
      expect((await registrationApi.users()).find((user) => user.id === admins[0].id)?.role).toBe('admin')
    })
  })
}
