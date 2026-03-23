import { test, expect } from '@playwright/test';
import { DashboardPage } from '../../pages/dashboard.page';

test.describe('Back Navigation @dashboard @navigation', () => {
  let dashboard: DashboardPage;

  test.beforeEach(async ({ page }) => {
    dashboard = new DashboardPage(page);
    await dashboard.goto();
  });

  test('BB-01: LB Playground → browser Back → Dashboard', async ({ page }) => {
    // Navigate to LB Playground
    await dashboard.openPlayground();
    await expect(page).toHaveURL(/#lb-playground/);

    // Use browser back button
    await page.goBack();
    await page.waitForTimeout(500);

    // Should be back on dashboard
    expect(page.url()).toMatch(/\/dashboard\/?$/);
    await expect(page.locator('#theme-toggle')).toBeVisible({ timeout: 10000 });
  });

  test('BB-02: Audit Log → browser Back → Dashboard', async ({ page }) => {
    // Navigate to Audit Log
    await dashboard.openAuditLog();
    await expect(page).toHaveURL(/#audit-log/);

    // Use browser back button
    await page.goBack();
    await page.waitForTimeout(500);

    // Should be back on dashboard
    expect(page.url()).toMatch(/\/dashboard\/?$/);
    await expect(page.locator('#theme-toggle')).toBeVisible({ timeout: 10000 });
  });

  test('BB-03: LB Playground → UI Back button → Dashboard', async ({ page }) => {
    // Navigate to LB Playground
    await dashboard.openPlayground();
    await expect(page).toHaveURL(/#lb-playground/);

    // Click the Back button in the UI
    const backButton = page.getByRole('button', { name: /back/i }).first();
    await expect(backButton).toBeVisible({ timeout: 5000 });
    await backButton.click();
    await page.waitForTimeout(500);

    // Should be back on dashboard
    expect(page.url()).toMatch(/\/dashboard\/?$/);
    await expect(page.locator('#theme-toggle')).toBeVisible({ timeout: 10000 });
  });
});
