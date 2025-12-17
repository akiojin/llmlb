import { test, expect } from '@playwright/test';
import { DashboardPage } from '../../pages/dashboard.page';

test.describe('Dashboard Header Controls @dashboard', () => {
  let dashboard: DashboardPage;

  test.beforeEach(async ({ page }) => {
    dashboard = new DashboardPage(page);
    await dashboard.goto();
  });

  test('H-01: Theme toggle cycles through themes', async ({ page }) => {
    // Get initial theme from documentElement (html element)
    const initialTheme = await page.evaluate(() => document.documentElement.classList.contains('dark'));

    // Click theme toggle
    await dashboard.toggleTheme();
    const theme1 = await page.evaluate(() => document.documentElement.classList.contains('dark'));
    expect(theme1).not.toBe(initialTheme);

    // Click again to toggle back
    await dashboard.toggleTheme();
    const theme2 = await page.evaluate(() => document.documentElement.classList.contains('dark'));
    expect(theme2).toBe(initialTheme);
  });

  test('H-02: Playground button opens Playground in new tab', async ({ context }) => {
    // Playground opens in a new tab, not a modal
    const playgroundPage = await dashboard.openPlayground();
    expect(playgroundPage.url()).toContain('playground');
    await playgroundPage.close();
  });

  test('H-03: API Keys button opens API Keys modal', async ({ page }) => {
    // Click the API Keys button
    await dashboard.apiKeysButton.click();
    // Wait for the modal to appear - use role selector as Dialog uses Radix UI
    const modal = page.locator('[role="dialog"]:has-text("API Keys")');
    await expect(modal).toBeVisible({ timeout: 5000 });
  });

  test('H-04: Refresh button updates data', async ({ page }) => {
    // Refresh button triggers page reload
    await dashboard.refresh();
    // Wait for page reload to complete
    await page.waitForLoadState('networkidle');
    // Page should still be on dashboard
    expect(page.url()).toContain('dashboard');
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
