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
