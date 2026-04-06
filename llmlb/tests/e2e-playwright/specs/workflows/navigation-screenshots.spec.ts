import { test, expect } from '@playwright/test';
import { DashboardPage } from '../../pages/dashboard.page';
import { captureScreen } from '../../helpers/screenshot-helpers';

/**
 * Navigation Screenshots - Captures screenshots of all major screens.
 *
 * Run with: pnpm exec playwright test --project=screenshots --headed
 * Screenshots are saved to reports/screenshots/
 */
test.describe('Navigation Screenshots @screenshots', () => {
  let dashboard: DashboardPage;

  test.beforeEach(async ({ page }) => {
    dashboard = new DashboardPage(page);
    await dashboard.goto();
  });

  test('SS-01: Login page screenshot', async ({ page, context }) => {
    // Clear cookies to show login page
    await context.clearCookies();
    await page.goto('/dashboard/login.html');
    await page.waitForLoadState('load');
    await page.waitForTimeout(500);
    await captureScreen(page, 'ss-01-login');
  });

  test('SS-02: Dashboard - Endpoints tab screenshot', async ({ page }) => {
    // Default tab is Endpoints
    await page.waitForTimeout(500);
    await captureScreen(page, 'ss-02-dashboard-endpoints');
  });

  test('SS-03: Dashboard - Models tab screenshot', async ({ page }) => {
    await dashboard.goToTab('Models');
    await page.waitForTimeout(500);
    await captureScreen(page, 'ss-03-dashboard-models');
  });

  test('SS-04: Dashboard - Statistics tab screenshot', async ({ page }) => {
    await dashboard.goToStatisticsTab();
    await page.waitForTimeout(500);
    await captureScreen(page, 'ss-04-dashboard-statistics');
  });

  test('SS-05: Dashboard - History tab screenshot', async ({ page }) => {
    await dashboard.goToHistoryTab();
    await page.waitForTimeout(500);
    await captureScreen(page, 'ss-05-dashboard-history');
  });

  test('SS-06: Dashboard - Logs tab screenshot', async ({ page }) => {
    await dashboard.goToLogsTab();
    await page.waitForTimeout(500);
    await captureScreen(page, 'ss-06-dashboard-logs');
  });

  test('SS-07: LB Playground screenshot', async ({ page }) => {
    await dashboard.openPlayground();
    await page.waitForTimeout(500);
    await captureScreen(page, 'ss-07-lb-playground');
  });

  test('SS-08: Audit Log screenshot (admin)', async ({ page }) => {
    await dashboard.openAuditLog();
    await page.waitForTimeout(500);
    await captureScreen(page, 'ss-08-audit-log');
  });

  test('SS-09: API Keys modal screenshot', async ({ page }) => {
    await dashboard.openApiKeys();
    await page.waitForTimeout(500);
    await captureScreen(page, 'ss-09-api-keys-modal');
  });

  test('SS-10: Dark theme screenshot', async ({ page }) => {
    // Ensure dark theme
    const isDark = await page.evaluate(() =>
      document.documentElement.classList.contains('dark')
    );
    if (!isDark) {
      await dashboard.toggleTheme();
      await page.waitForTimeout(300);
    }
    await captureScreen(page, 'ss-10-dark-theme');
  });

  test('SS-11: Mobile viewport screenshot', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });
    await page.waitForTimeout(500);
    await captureScreen(page, 'ss-11-mobile-viewport');
  });
});
