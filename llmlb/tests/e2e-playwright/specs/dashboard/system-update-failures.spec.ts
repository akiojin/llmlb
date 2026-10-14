import { test, expect, type Route, type WebSocketRoute } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'
import type { HistoryEntry, SystemInfo } from '../../../../src/web/dashboard/src/lib/api/system'

const failureReason = 'Administrator privileges are required. Run the installer on the server.'

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`System update failures (${colorScheme}) @dashboard`, () => {
    test.use({ colorScheme })

    test('shows failure history and ends Applying when the server does not return (#907)', async ({ page }, testInfo) => {
      const pageErrors: string[] = []
      page.on('pageerror', (error) => pageErrors.push(error.message))
      page.on('console', (message) => {
        if (message.type() === 'error') pageErrors.push(message.text())
      })

      // Login and session validation use the real server. Only update data is
      // controlled; no installer or shutdown request reaches the server.
      const login = await page.request.post('/api/auth/login', {
        data: { username: 'admin', password: 'test' },
      })
      expect(login.ok()).toBe(true)
      let system: SystemInfo = {
        version: '6.2.0', pid: 1, in_flight: 0,
        update: { state: 'failed', latest: '6.3.0', message: failureReason, failed_at: '2026-10-01T00:00:00Z' },
      }
      const history: HistoryEntry[] = [
        { kind: 'applied', version: '6.2.0', timestamp: '2026-09-29T00:00:00Z' },
        { kind: 'failed', version: '6.3.0', message: failureReason, timestamp: '2026-10-01T00:00:00Z' },
        { kind: 'rollback', version: '6.2.0', timestamp: '2026-09-30T00:00:00Z' },
      ]
      let historyRequests = 0
      await page.route('**/api/system/update/history', async (route) => {
        historyRequests += 1
        await route.fulfill({ json: { history } })
      })
      let serverResponding = true
      const pendingSystemRequests: Route[] = []
      await page.route('**/api/system', async (route) => {
        if (!serverResponding) {
          // Keep the last successful Applying snapshot cached, as happens
          // while the helper owns the update and the server is unreachable.
          pendingSystemRequests.push(route)
          return
        }
        await route.fulfill({ json: system })
      })
      let socket: WebSocketRoute | undefined
      await page.routeWebSocket('**/ws/dashboard', (ws) => { socket = ws })
      await page.clock.install()
      await ensureDashboardLogin(page)
      await expect.poll(() => socket !== undefined).toBe(true)
      if (colorScheme === 'dark') await expect(page.locator('html')).toHaveClass(/\bdark\b/)
      else await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)

      await expect(page.getByText(failureReason, { exact: true })).toBeVisible()
      expect(historyRequests).toBe(0)
      await page.getByRole('button', { name: 'Settings', exact: true }).click()
      const dialog = page.getByRole('dialog', { name: 'Settings', exact: true })
      await dialog.getByRole('tab', { name: 'History', exact: true }).click()
      const entries = dialog.getByRole('list', { name: 'Update history' }).getByRole('listitem')
      await expect(entries).toHaveCount(3)
      expect(historyRequests).toBe(1)
      await expect(entries.nth(0)).toContainText('Failed')
      await expect(entries.nth(0)).toContainText(failureReason)
      await expect(entries.nth(1)).toContainText('Rolled back')
      await expect(entries.nth(2)).toContainText('Applied')
      await testInfo.attach(`update-history-${colorScheme}`, {
        body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png',
      })
      await page.keyboard.press('Escape')

      system = { ...system, update: { state: 'applying', latest: '6.3.0', method: 'installer' } }
      socket!.send(JSON.stringify({ changed: 'system' }))
      const applyingTitle = page.getByText('Applying update: v6.3.0', { exact: true })
      await expect(applyingTitle).toBeVisible()
      const banner = page.locator('section').filter({ has: applyingTitle })
      await expect(banner.locator('.animate-spin')).toHaveCount(2)
      serverResponding = false

      await page.clock.fastForward(300_001)

      await expect.poll(() => pendingSystemRequests.length).toBeGreaterThan(0)
      await expect(page.getByText('Update needs attention', { exact: true })).toBeVisible()
      await expect(page.getByText(/Check the server.*manually run the installer/)).toBeVisible()
      await expect(page.getByRole('button', { name: 'Applying update...', exact: true })).toHaveCount(0)
      const attentionBanner = page.locator('section').filter({ has: page.getByText('Update needs attention', { exact: true }) })
      await expect(attentionBanner.locator('.animate-spin')).toHaveCount(0)
      await expect(page.getByRole('button', { name: 'Check for updates', exact: true })).toBeEnabled()
      await testInfo.attach(`update-needs-attention-${colorScheme}`, {
        body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png',
      })

      system = { ...system, version: '6.3.0', update: { state: 'up_to_date', checked_at: null } }
      serverResponding = true
      for (const route of pendingSystemRequests) await route.fulfill({ json: system })
      socket!.send(JSON.stringify({ changed: 'system' }))
      await expect(page.getByText('Up to date', { exact: true })).toBeVisible()
      await expect(page.getByText('Update needs attention', { exact: true })).toHaveCount(0)
      expect(pageErrors).toEqual([])
    })
  })
}
