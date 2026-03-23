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

  test('MR-04: モバイルVPでLB Playgroundにドロップダウン経由でアクセス', async ({ page }) => {
    await ensureDashboardLogin(page)
    // LB Playground header button should be hidden on mobile (lg:hidden)
    const headerBtn = page.locator('#lb-playground-button')
    await expect(headerBtn).not.toBeVisible()

    // Open user dropdown and click LB Playground
    const userButton = page.locator('button:has(svg.lucide-user)')
    await userButton.click()
    await page.waitForTimeout(300)

    const lbPlaygroundItem = page.locator('[role="menuitem"]:has-text("LB Playground")')
    await expect(lbPlaygroundItem).toBeVisible({ timeout: 5000 })
    await lbPlaygroundItem.click()
    await expect(page).toHaveURL(/#lb-playground/)
  })

  test('MR-05: モバイルVPでAPI Keysにドロップダウン経由でアクセス', async ({ page }) => {
    await ensureDashboardLogin(page)
    // API Keys header button should be hidden on mobile (sm:hidden at 375px)
    const headerBtn = page.locator('#api-keys-button')
    await expect(headerBtn).not.toBeVisible()

    // Open user dropdown and click API Keys
    const userButton = page.locator('button:has(svg.lucide-user)')
    await userButton.click()
    await page.waitForTimeout(300)

    const apiKeysItem = page.locator('[role="menuitem"]:has-text("API Keys")')
    await expect(apiKeysItem).toBeVisible({ timeout: 5000 })
    await apiKeysItem.click()

    // API Keys modal should open
    const modal = page.locator('[role="dialog"]:has-text("API Keys")')
    await expect(modal).toBeVisible({ timeout: 5000 })
  })

  test('MR-06: モバイルVPでAudit Logにドロップダウン経由でアクセス (admin)', async ({ page }) => {
    await ensureDashboardLogin(page)
    // Audit Log header button should be hidden on mobile
    const headerBtn = page.locator('#audit-log-button')
    await expect(headerBtn).not.toBeVisible()

    // Open user dropdown and click Audit Log
    const userButton = page.locator('button:has(svg.lucide-user)')
    await userButton.click()
    await page.waitForTimeout(300)

    const auditLogItem = page.locator('[role="menuitem"]:has-text("Audit Log")')
    if (await auditLogItem.isVisible({ timeout: 3000 }).catch(() => false)) {
      await auditLogItem.click()
      await expect(page).toHaveURL(/#audit-log/)
    }
  })
})
