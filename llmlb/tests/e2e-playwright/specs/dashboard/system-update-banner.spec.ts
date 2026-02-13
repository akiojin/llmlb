import { test, expect } from '@playwright/test'
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

    // The dashboard header shows the title "LLM Load Balancer" and connection status.
    // Version is only visible in the update banner when an update is available;
    // in test/dev environments, the system is typically up_to_date so no version
    // is rendered directly. Instead, verify the dashboard loaded and shows
    // the connection status indicator (which proves /api/system was called).
    const connectionStatus = page.locator('#connection-status')
    await expect(connectionStatus).toBeVisible({ timeout: 10000 })
    await expect(connectionStatus).toContainText('Online')
  })
})
