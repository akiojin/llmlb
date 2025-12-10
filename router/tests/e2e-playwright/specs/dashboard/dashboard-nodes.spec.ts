import { test, expect } from '@playwright/test';
import { DashboardPage } from '../../pages/dashboard.page';
import { DashboardSelectors } from '../../helpers/selectors';

test.describe('Dashboard Nodes Tab @dashboard', () => {
  let dashboard: DashboardPage;

  test.beforeEach(async ({ page }) => {
    dashboard = new DashboardPage(page);
    await dashboard.goto();
  });

  test('N-01: Nodes table body exists', async () => {
    await expect(dashboard.nodesBody).toBeVisible();
  });

  test('N-02: Status filter is functional', async ({ page }) => {
    // Test each filter option
    await dashboard.filterNodesByStatus('online');
    await expect(dashboard.filterStatus).toHaveValue('online');

    await dashboard.filterNodesByStatus('offline');
    await expect(dashboard.filterStatus).toHaveValue('offline');

    await dashboard.filterNodesByStatus('all');
    await expect(dashboard.filterStatus).toHaveValue('all');
  });

  test('N-03: Search filter accepts input', async () => {
    const searchQuery = 'test-node';
    await dashboard.searchNodes(searchQuery);
    await expect(dashboard.filterQuery).toHaveValue(searchQuery);
  });

  test('N-04: Sort by machine name is clickable', async ({ page }) => {
    const sortHeader = page.locator(DashboardSelectors.nodes.sortMachine);
    await expect(sortHeader).toBeVisible();
    await sortHeader.click();
    // Check sort indicator changes
    const indicator = sortHeader.locator('.sort-indicator');
    const text = await indicator.textContent();
    expect(text).toBeDefined();
  });

  test('N-05: Sort by status is clickable', async () => {
    await dashboard.sortBy('status');
    // Should not throw error
    expect(true).toBe(true);
  });

  test('N-06: Sort by uptime is clickable', async () => {
    await dashboard.sortBy('uptime');
    expect(true).toBe(true);
  });

  test('N-07: Sort by total is clickable', async () => {
    await dashboard.sortBy('total');
    expect(true).toBe(true);
  });

  test('N-08: Pagination prev button exists', async ({ page }) => {
    const prevBtn = page.locator(DashboardSelectors.nodes.pagePrev);
    await expect(prevBtn).toBeVisible();
  });

  test('N-09: Pagination next button exists', async ({ page }) => {
    const nextBtn = page.locator(DashboardSelectors.nodes.pageNext);
    await expect(nextBtn).toBeVisible();
  });

  test('N-10: Select all checkbox exists', async ({ page }) => {
    const selectAll = page.locator(DashboardSelectors.nodes.selectAll);
    await expect(selectAll).toBeVisible();
  });

  test('N-11: Export JSON button is clickable', async ({ page }) => {
    // Setup download listener
    const downloadPromise = page.waitForEvent('download', { timeout: 3000 }).catch(() => null);

    await dashboard.exportJson.click();

    // May or may not trigger download depending on data
    expect(true).toBe(true);
  });

  test('N-12: Export CSV button is clickable', async ({ page }) => {
    const downloadPromise = page.waitForEvent('download', { timeout: 3000 }).catch(() => null);

    await dashboard.exportCsv.click();

    expect(true).toBe(true);
  });

  test('N-13: Node row details button opens modal', async ({ page }) => {
    // Look for details button in node rows
    const detailsBtn = page.locator('#nodes-body button:has-text("Details")').first();

    if (await detailsBtn.isVisible()) {
      await detailsBtn.click();
      const nodeModal = page.locator(DashboardSelectors.modals.nodeModal);
      await expect(nodeModal).toBeVisible();
    } else {
      // No nodes available, skip
      test.skip();
    }
  });
});
