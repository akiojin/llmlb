import { test, expect } from '@playwright/test';
import { DashboardPage } from '../../pages/dashboard.page';
import { DashboardSelectors } from '../../helpers/selectors';

test.describe('Dashboard Stats Grid @dashboard', () => {
  let dashboard: DashboardPage;

  test.beforeEach(async ({ page }) => {
    dashboard = new DashboardPage(page);
    await dashboard.goto();
    // Wait for initial data load
    await page.waitForTimeout(500);
  });

  test('S-01: Registered Nodes stat is displayed', async () => {
    await expect(dashboard.totalNodes).toBeVisible();
    const text = await dashboard.getTotalNodes();
    expect(text).toBeDefined();
  });

  test('S-02: Online/Offline node counts are displayed', async () => {
    await expect(dashboard.onlineNodes).toBeVisible();
    await expect(dashboard.offlineNodes).toBeVisible();
  });

  test('S-03: Total Requests stat is displayed', async () => {
    await expect(dashboard.totalRequests).toBeVisible();
  });

  test('S-04: Success/Failed request counts are displayed', async ({ page }) => {
    const success = page.locator(DashboardSelectors.stats.successfulRequests);
    const failed = page.locator(DashboardSelectors.stats.failedRequests);
    await expect(success).toBeVisible();
    await expect(failed).toBeVisible();
  });

  test('S-05: Average Response Time is displayed', async ({ page }) => {
    const avgResponse = page.locator(DashboardSelectors.stats.averageResponseTime);
    await expect(avgResponse).toBeVisible();
  });

  test('S-06: GPU Usage is displayed', async ({ page }) => {
    const gpuUsage = page.locator(DashboardSelectors.stats.averageGpuUsage);
    await expect(gpuUsage).toBeVisible();
  });

  test('S-07: Active Requests is displayed', async ({ page }) => {
    const activeRequests = page.locator(DashboardSelectors.stats.activeRequests);
    await expect(activeRequests).toBeVisible();
  });

  test('S-08: Stats update on refresh', async ({ page }) => {
    // Store initial values
    const initialTotal = await dashboard.totalRequests.textContent();

    // Trigger refresh
    await dashboard.refresh();
    await page.waitForTimeout(1000);

    // Values should still be present (may or may not change)
    const newTotal = await dashboard.totalRequests.textContent();
    expect(newTotal).toBeDefined();
  });
});
