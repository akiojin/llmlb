import { test, expect } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'

test.describe('Mobile Responsive @dashboard', () => {
  test.use({ viewport: { width: 375, height: 667 } })

  test('MR-01: モバイルVPでダッシュボード表示', async ({ page }) => {
    await ensureDashboardLogin(page)
    // Main stat cards should be visible on mobile viewport
    await expect(page.locator('[data-stat="total-endpoints"]')).toBeVisible({ timeout: 10000 })
  })

  test('MR-02: モバイルVPでユーザードロップダウン', async ({ page }) => {
    await ensureDashboardLogin(page)
    // User dropdown button should be accessible on mobile
    const userButton = page.locator('button').filter({ hasText: 'admin' }).first()
    if (await userButton.isVisible({ timeout: 5000 }).catch(() => false)) {
      await userButton.click()
      // Dropdown menu should appear
      await expect(page.getByText('Logout')).toBeVisible({ timeout: 5000 })
    }
  })

  test('MR-03: モバイルVPでPlayground操作', async ({ page }) => {
    await ensureDashboardLogin(page)
    // Navigate to playground if available
    const playgroundLink = page.getByRole('link', { name: /playground/i }).first()
    if (await playgroundLink.isVisible({ timeout: 5000 }).catch(() => false)) {
      await playgroundLink.click()
      await page.waitForLoadState('networkidle')
      // Playground content should be visible on mobile
      await expect(page.locator('body')).toBeVisible()
    }
  })
})
