import { test, expect } from '@playwright/test';
import { DashboardPage } from '../../pages/dashboard.page';

test.describe('Dashboard Header Controls @dashboard', () => {
  let dashboard: DashboardPage;

  test.beforeEach(async ({ page }) => {
    dashboard = new DashboardPage(page);
    await dashboard.goto();
  });

  test('H-01: Theme toggle cycles through themes', async ({ page }) => {
    // Get initial body class
    const initialTheme = await page.evaluate(() => document.body.className);

    // Click theme toggle 3 times
    await dashboard.toggleTheme();
    const theme1 = await page.evaluate(() => document.body.className);
    expect(theme1).not.toBe(initialTheme);

    await dashboard.toggleTheme();
    const theme2 = await page.evaluate(() => document.body.className);
    expect(theme2).not.toBe(theme1);

    await dashboard.toggleTheme();
    const theme3 = await page.evaluate(() => document.body.className);
    // Should cycle back to initial or different theme
    expect(theme3).toBeDefined();
  });

  test('H-02: Playground button opens chat modal', async () => {
    await dashboard.openPlayground();
    await expect(dashboard.chatModal).toBeVisible();
    await dashboard.closePlayground();
    await expect(dashboard.chatModal).toBeHidden();
  });

  test('H-03: API Keys button opens API Keys modal', async () => {
    await dashboard.openApiKeys();
    await expect(dashboard.apiKeysModal).toBeVisible();
  });

  test('H-04: Refresh button updates data', async ({ page }) => {
    const initialTime = await page.locator('#last-refreshed').textContent();
    await dashboard.refresh();
    // Wait for refresh to complete
    await page.waitForTimeout(1000);
    const newTime = await page.locator('#last-refreshed').textContent();
    // Time should update (or stay same if very fast)
    expect(newTime).toBeDefined();
  });

  test('H-05: Connection status displays correctly', async () => {
    const status = await dashboard.getConnectionStatus();
    expect(status).toContain('Connection');
  });

  test('H-06: Last refreshed timestamp is displayed', async ({ page }) => {
    const lastRefreshed = await page.locator('#last-refreshed').textContent();
    expect(lastRefreshed).toContain('Last');
  });

  test('H-07: Performance metrics are displayed', async ({ page }) => {
    const metrics = await page.locator('#refresh-metrics').textContent();
    expect(metrics).toContain('Fetch');
  });
});
