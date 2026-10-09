import { test, expect, errors, type WebSocketRoute } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768'

test.describe('System Update Banner @dashboard', () => {
  test('SUB-01: /api/system → バージョン情報', async ({ request }) => {
    // /api/system requires JWT auth (not API key).
    // Login first to obtain a session cookie.
    const loginResp = await request.post(`${API_BASE}/api/auth/login`, {
      headers: { 'Content-Type': 'application/json' },
      data: { username: 'admin', password: 'test' },
    })
    expect(loginResp.ok()).toBeTruthy()

    // The login sets a cookie. Playwright's APIRequestContext automatically
    // persists cookies, so subsequent requests will include it.
    const resp = await request.get(`${API_BASE}/api/system`)
    expect(resp.ok()).toBeTruthy()
    const json = await resp.json()
    expect(json.version || json.current_version).toBeTruthy()
  })

  test('SUB-02: ダッシュボードにシステム情報表示', async ({ page }) => {
    await ensureDashboardLogin(page)

    const systemResp = await page.request.get(`${API_BASE}/api/system`)
    expect(systemResp.ok()).toBeTruthy()
    const systemJson = await systemResp.json()
    expect(systemJson.version).toBeTruthy()

    const connectionStatus = page.locator('#connection-status')
    const currentVersion = page.locator('#current-version')

    await expect(connectionStatus).toBeVisible({ timeout: 10000 })
    await expect(connectionStatus).toContainText('Online')
    await expect(currentVersion).toBeVisible({ timeout: 10000 })
    await expect(currentVersion).toContainText(`Current v${systemJson.version}`)
  })
})

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`System Update Banner notifications (${colorScheme}) @dashboard`, () => {
    test.use({ colorScheme })

    test('SUB-03: refreshes a cancelled schedule only after a system notification (#853)', async ({ page }, testInfo) => {
      const pageErrors: string[] = []
      page.on('pageerror', (error) => pageErrors.push(error.message))
      page.on('console', (message) => {
        if (message.type() === 'error') pageErrors.push(message.text())
      })

      // Authenticate against the real server before navigation so the browser
      // never produces an expected anonymous-session 401 console error.
      const login = await page.request.post('/api/auth/login', {
        data: { username: 'admin', password: 'test' },
      })
      expect(login.ok()).toBe(true)

      let dashboardSocket: WebSocketRoute | undefined
      await page.routeWebSocket('**/ws/dashboard', (socket) => {
        dashboardSocket = socket
      })

      let scheduleCancelled = false
      let systemRequests = 0
      let cancelRequests = 0
      const systemInfo = {
        version: '6.2.0',
        pid: 4242,
        in_flight: 0,
        rollback_available: false,
        update: {
          state: 'available',
          current: '6.2.0',
          latest: '6.3.0',
          release_url: 'https://example.test/releases/v6.3.0',
          payload: { payload: 'ready', kind: 'portable' },
          checked_at: '2026-10-01T00:00:00Z',
        },
      }
      await page.route('**/api/system', async (route) => {
        expect(route.request().method()).toBe('GET')
        systemRequests++
        await route.fulfill({ json: {
          ...systemInfo,
          schedule: scheduleCancelled ? null : {
            mode: 'idle',
            scheduled_by: 'admin',
            target_version: '6.3.0',
            created_at: '2026-10-01T00:00:00Z',
          },
        } })
      })
      await page.route('**/api/system/update/schedule', async (route) => {
        expect(route.request().method()).toBe('DELETE')
        cancelRequests++
        scheduleCancelled = true
        await route.fulfill({ json: { cancelled: true } })
      })

      await page.clock.install()
      await ensureDashboardLogin(page)
      await expect.poll(() => Boolean(dashboardSocket)).toBe(true)
      if (colorScheme === 'dark') {
        await expect(page.locator('html')).toHaveClass(/\bdark\b/)
      } else {
        await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)
      }
      const schedule = page.getByText('Scheduled by admin (idle)', { exact: false })
      await expect(schedule).toBeVisible()
      expect(systemRequests).toBe(1)

      // Freeze the clock before the mutation. After the system response, only
      // advance notification timers by far less than either polling interval.
      await page.clock.pauseAt(Date.now() + 1000)
      const pausedAt = await page.evaluate(() => Date.now())

      // Observe a quiet window after the mutation, with the application clock
      // still frozen, to catch an eager/manual invalidation before any frame.
      const prematureRefresh = page.waitForRequest(
        (request) => request.method() === 'GET' && new URL(request.url()).pathname === '/api/system',
        { timeout: 500 },
      ).then(
        () => true,
        (error: unknown) => {
          if (error instanceof errors.TimeoutError) return false
          throw error
        },
      )
      await schedule.getByRole('button', { name: 'Cancel', exact: true }).click()
      await expect(page.getByText('Schedule cancelled', { exact: true })).toBeVisible()
      expect(cancelRequests).toBe(1)
      expect(await prematureRefresh).toBe(false)
      expect(systemRequests).toBe(1)
      await expect(schedule).toBeVisible()
      expect(await page.evaluate(() => Date.now())).toBe(pausedAt)

      const systemResponse = page.waitForResponse(
        (response) => response.request().method() === 'GET' && new URL(response.url()).pathname === '/api/system',
      )
      dashboardSocket!.send(JSON.stringify({ changed: 'system' }))

      await expect.poll(() => systemRequests).toBe(2)
      const response = await systemResponse
      expect(response.ok()).toBe(true)
      await response.finished()
      // React Query delivers cache updates through a zero-delay timer. Let
      // that notification render without reaching the 5s/10s polling timers.
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return schedule.count()
      }).toBe(0)
      const advancedMs = await page.evaluate(() => Date.now()) - pausedAt
      expect(advancedMs).toBeGreaterThan(0)
      expect(advancedMs).toBeLessThan(1000)
      expect(systemRequests).toBe(2)
      expect(pageErrors).toEqual([])
      await testInfo.attach(`system-update-banner-${colorScheme}`, {
        body: await page.screenshot(),
        contentType: 'image/png',
      })
    })
  })
}
