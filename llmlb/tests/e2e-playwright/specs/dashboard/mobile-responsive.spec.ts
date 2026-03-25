import { expect, test } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'

test.describe('Mobile Responsive @dashboard', () => {
  test.use({ viewport: { width: 375, height: 667 } })

  test('MR-01: mobile viewport shows dashboard content', async ({ page }) => {
    await ensureDashboardLogin(page)
    await expect(page.locator('[data-stat="total-endpoints"]')).toBeVisible({ timeout: 10000 })
  })

  test('MR-02: mobile viewport can open the user dropdown', async ({ page }) => {
    await ensureDashboardLogin(page)

    const userButton = page.locator('button:has(svg.lucide-user)').first()
    await expect(userButton).toBeVisible({ timeout: 5000 })
    await userButton.click()

    await expect(page.getByRole('menuitem', { name: /sign out/i })).toBeVisible({
      timeout: 5000,
    })
  })

  test('MR-03: mobile viewport can reach Playground when present', async ({ page }) => {
    await ensureDashboardLogin(page)

    const playgroundLink = page.getByRole('link', { name: /playground/i }).first()
    if (await playgroundLink.isVisible({ timeout: 5000 }).catch(() => false)) {
      await playgroundLink.click()
      await page.waitForLoadState('networkidle')
      await expect(page.locator('body')).toBeVisible()
    }
  })

  test('MR-04: mobile viewport can access LB Playground from the dropdown', async ({ page }) => {
    await ensureDashboardLogin(page)

    const headerBtn = page.locator('#lb-playground-button')
    await expect(headerBtn).not.toBeVisible()

    const userButton = page.locator('button:has(svg.lucide-user)')
    await userButton.click()
    await page.waitForTimeout(300)

    const lbPlaygroundItem = page.locator('[role="menuitem"]:has-text("LB Playground")')
    await expect(lbPlaygroundItem).toBeVisible({ timeout: 5000 })
    await lbPlaygroundItem.click()
    await expect(page).toHaveURL(/#lb-playground/)
  })

  test('MR-05: mobile viewport can access API Keys from the dropdown', async ({ page }) => {
    await ensureDashboardLogin(page)

    const headerBtn = page.locator('#api-keys-button')
    await expect(headerBtn).not.toBeVisible()

    const userButton = page.locator('button:has(svg.lucide-user)')
    await userButton.click()
    await page.waitForTimeout(300)

    const apiKeysItem = page.locator('[role="menuitem"]:has-text("API Keys")')
    await expect(apiKeysItem).toBeVisible({ timeout: 5000 })
    await apiKeysItem.click()

    const modal = page.locator('[role="dialog"]:has-text("API Keys")')
    await expect(modal).toBeVisible({ timeout: 5000 })
  })

  test('MR-06: mobile viewport can access Audit Log from the dropdown', async ({ page }) => {
    await ensureDashboardLogin(page)

    const headerBtn = page.locator('#audit-log-button')
    await expect(headerBtn).not.toBeVisible()

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
