import { test, expect } from '@playwright/test';
import { DashboardPage } from '../../pages/dashboard.page';

test.describe('Deep Linking @dashboard @navigation', () => {
  test('DL-01: Direct access to /#lb-playground when authenticated', async ({ page }) => {
    // First login via dashboard
    const dashboard = new DashboardPage(page);
    await dashboard.goto();

    // Now navigate directly to /#lb-playground
    await page.goto('/dashboard/#lb-playground');
    await page.waitForLoadState('load');
    await page.waitForTimeout(500);

    // Should show LB Playground content
    await expect(page).toHaveURL(/#lb-playground/);
    // LB Playground sidebar should be visible
    const sidebar = page.locator('#lb-playground-sidebar');
    await expect(sidebar).toBeVisible({ timeout: 10000 });
  });

  test('DL-02: Direct access to /#audit-log when authenticated as admin', async ({ page }) => {
    // First login via dashboard
    const dashboard = new DashboardPage(page);
    await dashboard.goto();

    // Navigate directly to /#audit-log
    await page.goto('/dashboard/#audit-log');
    await page.waitForLoadState('load');
    await page.waitForTimeout(500);

    await expect(page).toHaveURL(/#audit-log/);
    // Audit log content should be visible (back button or audit log heading)
    const backButton = page.getByRole('button', { name: /back/i }).first();
    await expect(backButton).toBeVisible({ timeout: 10000 });
  });

  test('DL-03: Direct access to /#playground/:id when authenticated', async ({ page, request }) => {
    // First login via dashboard
    const dashboard = new DashboardPage(page);
    await dashboard.goto();

    // Get an endpoint ID (if available)
    const { listEndpoints } = await import('../../helpers/api-helpers');
    const endpoints = await listEndpoints(request);
    test.skip(endpoints.length === 0, 'No endpoints available to test');

    const endpointId = endpoints[0].id;

    // Navigate directly to playground with endpoint ID
    await page.goto(`/dashboard/#playground/${endpointId}`);
    await page.waitForLoadState('load');
    await page.waitForTimeout(500);

    await expect(page).toHaveURL(new RegExp(`#playground/${endpointId}`));
    // Playground should show back button
    const backButton = page.getByRole('button', { name: /back/i }).first();
    await expect(backButton).toBeVisible({ timeout: 10000 });
  });

  test('DL-04: Unauthenticated access to /dashboard redirects to login', async ({ page, context }) => {
    // Clear all cookies to simulate unauthenticated state
    await context.clearCookies();

    // Try to access dashboard
    await page.goto('/dashboard');
    await page.waitForLoadState('load');
    await page.waitForTimeout(1000);

    // Should be redirected to login page
    await expect(page).toHaveURL(/login/);
  });
});
