import { test, expect } from '@playwright/test';
import { DashboardPage } from '../../pages/dashboard.page';

/** Dashboard returns to /dashboard/ or /dashboard/# after back navigation */
const DASHBOARD_URL_PATTERN = /\/dashboard\/(#?)$/;

test.describe('Back Navigation @dashboard @navigation', () => {
  let dashboard: DashboardPage;

  test.beforeEach(async ({ page }) => {
    dashboard = new DashboardPage(page);
    await dashboard.goto();
  });

  test('BB-01: LB Playground → browser Back → Dashboard', async ({ page }) => {
    await dashboard.openPlayground();
    await expect(page).toHaveURL(/#lb-playground/);

    await page.goBack();
    await page.waitForTimeout(500);

    expect(page.url()).toMatch(DASHBOARD_URL_PATTERN);
    await expect(page.locator('#theme-toggle')).toBeVisible({ timeout: 10000 });
  });

  test('BB-02: Audit Log → browser Back → Dashboard', async ({ page }) => {
    await dashboard.openAuditLog();
    await expect(page).toHaveURL(/#audit-log/);

    await page.goBack();
    await page.waitForTimeout(500);

    expect(page.url()).toMatch(DASHBOARD_URL_PATTERN);
    await expect(page.locator('#theme-toggle')).toBeVisible({ timeout: 10000 });
  });

  test('BB-03: LB Playground → UI Back button → Dashboard', async ({ page }) => {
    await dashboard.openPlayground();
    await expect(page).toHaveURL(/#lb-playground/);

    // PlaygroundBase uses "Back to Dashboard" button
    const backButton = page.getByRole('button', { name: /back to dashboard/i });
    await expect(backButton).toBeVisible({ timeout: 5000 });
    await backButton.click();
    await page.waitForTimeout(500);

    expect(page.url()).toMatch(DASHBOARD_URL_PATTERN);
    await expect(page.locator('#theme-toggle')).toBeVisible({ timeout: 10000 });
  });
});
