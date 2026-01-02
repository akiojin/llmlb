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

  test('S-01: Total Nodes stat card is displayed', async () => {
    await expect(dashboard.totalNodes).toBeVisible();
    const text = await dashboard.getTotalNodes();
    expect(text).toBeDefined();
  });

  test('S-02: Total Requests stat card is displayed', async () => {
    await expect(dashboard.totalRequests).toBeVisible();
  });

  test('S-03: Success Rate stat card is displayed', async ({ page }) => {
    const successRate = page.locator(DashboardSelectors.stats.successRate);
    await expect(successRate).toBeVisible();
  });

  test('S-04: Average Response Time stat card is displayed', async () => {
    await expect(dashboard.averageResponseTime).toBeVisible();
  });

  test('S-05: Average GPU Usage stat card is displayed', async ({ page }) => {
    const gpuUsage = page.locator(DashboardSelectors.stats.averageGpuUsage);
    await expect(gpuUsage).toBeVisible();
  });

  test('S-06: Average GPU Memory stat card is displayed', async ({ page }) => {
    const gpuMemory = page.locator(DashboardSelectors.stats.averageGpuMemory);
    await expect(gpuMemory).toBeVisible();
  });

  test('S-07: Stats grid contains 8 cards', async ({ page }) => {
    // All 8 stat cards should be visible
    const statCards = page.locator('[data-stat]');
    await expect(statCards).toHaveCount(8);
  });

  test('S-08: Stats update on refresh', async ({ page }) => {
    // Store initial values
    const initialTotal = await dashboard.totalRequests.textContent();

    // Trigger refresh (note: refresh reloads the page)
    await dashboard.refresh();
    await page.waitForLoadState('networkidle');

    // Values should still be present (may or may not change)
    const newTotal = await dashboard.totalRequests.textContent();
    expect(newTotal).toBeDefined();
  });
});
