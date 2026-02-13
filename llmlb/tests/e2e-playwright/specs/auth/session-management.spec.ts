import { test, expect } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768'

/**
 * Helper: Open user dropdown menu and click "Sign out".
 * The user menu trigger is the last ghost icon-button in the header
 * (contains a User icon from lucide-react).
 * Radix DropdownMenuContent is rendered via a portal.
 */
async function signOut(page: import('@playwright/test').Page) {
  // The user menu trigger is the last icon button in the header actions area.
  // It is identifiable as the button right after the theme toggle (#theme-toggle).
  const userMenuTrigger = page.locator('header button').last()
  await userMenuTrigger.click()
  // Radix DropdownMenuItem renders with role="menuitem" inside a portal
  const signOutItem = page.getByRole('menuitem', { name: /sign out/i })
  await signOutItem.waitFor({ state: 'visible', timeout: 5000 })
  await signOutItem.click()
}

test.describe('Session Management @auth', () => {
  test.describe.configure({ mode: 'serial' })
  test('SM-01: Sign out -> login redirect & API 401', async ({ page, request }) => {
    await ensureDashboardLogin(page)

    // Perform sign out
    await signOut(page)

    // Should redirect to login page
    await expect(page).toHaveURL(/login/, { timeout: 10000 })

    // Dashboard API should return 401 without valid cookie
    const resp = await request.get(`${API_BASE}/api/endpoints`)
    expect(resp.status()).toBe(401)
  })

  test('SM-02: After sign out -> back button -> login redirect', async ({ page }) => {
    await ensureDashboardLogin(page)

    // Perform sign out
    await signOut(page)
    await expect(page).toHaveURL(/login/, { timeout: 10000 })

    // Navigate back
    await page.goBack()

    // Should still be on login page (or redirected back to it)
    // Wait for any redirect to settle
    await page.waitForLoadState('networkidle')
    await expect(page).toHaveURL(/login/, { timeout: 10000 })
  })

  test('SM-03: JWT cookie deletion -> reload -> login redirect', async ({ page }) => {
    await ensureDashboardLogin(page)

    // Clear all cookies (including JWT)
    await page.context().clearCookies()

    // Reload the page - may abort due to redirect, which is expected
    await page.reload().catch(() => {})

    // Wait for redirect to settle
    await page.waitForLoadState('networkidle').catch(() => {})

    // Should redirect to login page
    await expect(page).toHaveURL(/login/, { timeout: 10000 })
  })

  test('SM-04: After sign out -> direct URL -> login redirect', async ({ page }) => {
    await ensureDashboardLogin(page)

    // Perform sign out
    await signOut(page)
    await expect(page).toHaveURL(/login/, { timeout: 10000 })

    // Try to access dashboard directly
    await page.goto(`${API_BASE}/dashboard`)
    await page.waitForLoadState('networkidle')

    // Should redirect to login page
    await expect(page).toHaveURL(/login/, { timeout: 10000 })
  })
})
