import { test, expect } from '@playwright/test';
import { DashboardPage } from '../../pages/dashboard.page';

test.describe('Deep Linking @dashboard @navigation', () => {
  test('DL-01: Direct access to /#lb-playground when authenticated', async ({ page }) => {
    const dashboard = new DashboardPage(page);
    await dashboard.goto();

    await page.goto('/dashboard/#lb-playground');
    await page.waitForLoadState('load');
    await page.waitForTimeout(500);

    await expect(page).toHaveURL(/#lb-playground/);
    const sidebar = page.locator('#lb-playground-sidebar');
    await expect(sidebar).toBeVisible({ timeout: 10000 });
  });

  test('DL-02: Direct access to /#audit-log when authenticated as admin', async ({ page }) => {
    const dashboard = new DashboardPage(page);
    await dashboard.goto();

    await page.goto('/dashboard/#audit-log');
    await page.waitForLoadState('load');
    await page.waitForTimeout(500);

    await expect(page).toHaveURL(/#audit-log/);
    // AuditLog page has a "Dashboard" back button with ChevronLeft icon
    const dashboardButton = page.getByRole('button', { name: /dashboard/i }).first();
    await expect(dashboardButton).toBeVisible({ timeout: 10000 });
  });

  test('DL-03: Direct access to /#playground/:id when authenticated', async ({ page, request }) => {
    const dashboard = new DashboardPage(page);
    await dashboard.goto();

    const { listEndpoints } = await import('../../helpers/api-helpers');
    const endpoints = await listEndpoints(request);
    test.skip(endpoints.length === 0, 'No endpoints available to test');

    const endpointId = endpoints[0].id;

    await page.goto(`/dashboard/#playground/${endpointId}`);
    await page.waitForLoadState('load');
    await page.waitForTimeout(500);

    await expect(page).toHaveURL(new RegExp(`#playground/${endpointId}`));
    // PlaygroundBase has "Back to Dashboard" button
    const backButton = page.getByRole('button', { name: /back to dashboard/i });
    await expect(backButton).toBeVisible({ timeout: 10000 });
  });

  test('DL-04: Unauthenticated access to /dashboard redirects to login', async ({ page, context }) => {
    await context.clearCookies();

    await page.goto('/dashboard');
    await page.waitForLoadState('load');
    await page.waitForTimeout(1000);

    await expect(page).toHaveURL(/login/);
  });
});
