import { test, expect } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Audit Log ViewModel (${colorScheme}) @dashboard`, () => {
    test.use({ colorScheme })

    test('preserves listing, filters, paging, verification and navigation (#821 T002)', async ({ page }, testInfo) => {
      const errors: string[] = []
      page.on('pageerror', (error) => errors.push(error.message))
      page.on('console', (message) => {
        if (message.type() === 'error') errors.push(message.text())
      })

      // Real authentication; only the audit data is controlled by this test.
      const login = await page.request.post('/api/auth/login', {
        data: { username: 'admin', password: 'test' },
      })
      expect(login.ok()).toBe(true)
      const listRequests: URL[] = []
      await page.route(/\/api\/dashboard\/audit-logs(?:\?.*)?$/, async (route) => {
        const url = new URL(route.request().url())
        listRequests.push(url)
        const currentPage = Number(url.searchParams.get('page') || 1)
        await route.fulfill({ json: {
          items: [{
            id: currentPage, timestamp: '2026-10-01T00:00:00Z', http_method: 'POST',
            request_path: `/api/pilot-${url.searchParams.get('search') || 'entry'}`,
            status_code: 201, actor_type: 'user', actor_id: 'user-admin', actor_username: 'admin',
            api_key_owner_id: null, client_ip: '192.0.2.10', duration_ms: 0, input_tokens: null,
            output_tokens: null, total_tokens: 0, model_name: null, endpoint_id: null,
            detail: null, is_migrated: false,
          }], total: 120, page: currentPage, per_page: 50,
        } })
      })
      let releaseVerification!: () => void
      const verificationReady = new Promise<void>((resolve) => { releaseVerification = resolve })
      let verifyRequests = 0
      await page.route('**/api/dashboard/audit-logs/verify', async (route) => {
        verifyRequests++
        expect(route.request().method()).toBe('POST')
        await verificationReady
        await route.fulfill({ json: { valid: true, batches_checked: 3, tampered_batch: null, message: null } })
      })

      await ensureDashboardLogin(page)
      await page.locator('#audit-log-button').click()
      await expect(page.getByRole('heading', { name: 'Audit Log', exact: true })).toBeVisible()
      if (colorScheme === 'dark') {
        await expect(page.locator('html')).toHaveClass(/\bdark\b/)
      } else {
        await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)
      }
      await expect(page.getByRole('table')).toContainText('/api/pilot-entry')
      await expect(page.getByRole('link', { name: '192.0.2.10' })).toHaveAttribute('href', '?tab=clients&ip=192.0.2.10')
      await expect(page.getByRole('table')).toContainText('0ms')
      await expect(page.getByText('Page 1 / 3', { exact: true })).toBeVisible()
      await page.getByRole('button', { name: 'Next page' }).click()
      await expect(page.getByText('Page 2 / 3', { exact: true })).toBeVisible()
      await expect.poll(() => listRequests.at(-1)?.searchParams.get('page')).toBe('2')

      await page.getByPlaceholder('Search...').fill('login')
      await expect(page.getByRole('table')).toContainText('/api/pilot-login')
      await expect(page.getByText('Page 1 / 3', { exact: true })).toBeVisible()
      await expect.poll(() => listRequests.at(-1)?.searchParams.get('search')).toBe('login')
      await page.getByRole('combobox').nth(2).click()
      await page.getByRole('option', { name: '201 Created' }).click()
      await expect.poll(() => listRequests.at(-1)?.searchParams.get('status_code')).toBe('201')
      await page.getByRole('button', { name: 'Next page' }).click()
      await expect.poll(() => listRequests.at(-1)?.searchParams.get('page')).toBe('2')
      expect(listRequests.at(-1)?.searchParams.get('search')).toBe('login')
      expect(listRequests.at(-1)?.searchParams.get('status_code')).toBe('201')

      const verifyButton = page.getByRole('button', { name: 'Verify Hash Chain' })
      await verifyButton.click()
      await expect(verifyButton).toBeDisabled()
      releaseVerification()
      await expect(page.getByText('Verified (3 batches)', { exact: true })).toBeVisible()
      await expect(verifyButton).toBeEnabled()
      expect(verifyRequests).toBe(1)
      await testInfo.attach(`audit-log-${colorScheme}`, { body: await page.screenshot(), contentType: 'image/png' })

      await page.getByRole('button', { name: 'Dashboard', exact: true }).click()
      await expect(page.locator('#theme-toggle')).toBeVisible()
      expect(errors).toEqual([])
    })
  })
}
