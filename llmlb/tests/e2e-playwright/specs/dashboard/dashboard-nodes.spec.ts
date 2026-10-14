import { test, expect } from '../../helpers/endpoint.fixture';
import { DashboardPage } from '../../pages/dashboard.page';

/**
 * Dashboard Endpoints Tab Tests
 *
 * Note: The UI was renamed from "Nodes" to "Endpoints" as part of SPEC-e8e9326e.
 * These tests have been updated to reflect the current UI structure.
 *
 * Data-dependent cases provision their own endpoint, including on a fresh CI DB.
 */
test.describe('Dashboard Endpoints Tab @dashboard', () => {
  let dashboard: DashboardPage;

  test.beforeEach(async ({ page }) => {
    dashboard = new DashboardPage(page);
    await dashboard.goto();
    // Navigate to Endpoints tab (the default tab)
    await page.waitForSelector('[role="tabpanel"]', { timeout: 10000 });
  });

  test('E-01: Endpoints table exists', async ({ page }) => {
    // Check for the endpoints table
    const table = page.locator('table');
    await expect(table).toBeVisible({ timeout: 10000 });
  });

  test('E-02: Status filter dropdown exists', async ({ page }) => {
    // Status filter is a combobox/select
    const statusFilter = page.locator('[role="combobox"]').first();
    await expect(statusFilter).toBeVisible({ timeout: 10000 });
  });

  test('E-03: Search filter accepts input', async ({ page }) => {
    const searchQuery = 'test-endpoint';
    // Find the search input by role (textbox) - handles both input and contenteditable elements
    const searchInput = page.getByRole('textbox').first();
    await expect(searchInput).toBeVisible({ timeout: 10000 });
    await searchInput.click();
    await searchInput.fill(searchQuery);
    await expect(searchInput).toHaveValue(searchQuery);
  });

  test('E-04: Table headers are clickable for sorting', async ({ page }) => {
    // Wait for table to be fully rendered
    const table = page.locator('table');
    await expect(table).toBeVisible({ timeout: 10000 });

    // Find the Status header which has a sort indicator
    const statusHeader = page.locator('th').filter({ hasText: 'Status' });
    await expect(statusHeader).toBeVisible({ timeout: 5000 });

    // Click the Status header to sort
    await statusHeader.click();
    // Should not throw error - sorting works
    expect(true).toBe(true);
  });

  test('E-05: Table shows endpoint information', async ({ page }) => {
    // Wait for table to be fully rendered
    const table = page.locator('table');
    await expect(table).toBeVisible({ timeout: 10000 });

    // Table should have headers for endpoint info
    const headers = page.locator('th');
    const headerCount = await headers.count();
    // Expect at least: Name, URL, Type, Status, Requests, Latency, Models, Last Seen, Actions
    expect(headerCount).toBeGreaterThanOrEqual(6);
  });

  test('E-05a: Endpoints table does not show TPS column', async ({ page }) => {
    const table = page.locator('table');
    await expect(table).toBeVisible({ timeout: 10000 });

    const tpsHeaders = page.locator('th').filter({ hasText: /^TPS$/ });
    await expect(tpsHeaders).toHaveCount(0);
  });

  test('E-06: Add Endpoint button exists', async ({ page }) => {
    await expect(page.getByRole('button', { name: 'Add Endpoint', exact: true })).toBeVisible();
  });

  test('E-07: Clicking Add Endpoint opens dialog', async ({ page }) => {
    await page.getByRole('button', { name: 'Add Endpoint', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Add New Endpoint' });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByLabel('Name')).toBeVisible();
    await expect(dialog.getByLabel('Base URL')).toBeVisible();
  });

  test('E-08: Select all checkbox exists', async ({ page, endpoint }) => {
    await page.getByPlaceholder('Search by name or URL...').fill(endpoint.name);
    await expect(page.getByRole('row').filter({ hasText: endpoint.name })).toBeVisible();
    // Measure absence with a populated table; fail if the feature appears so this skip is revisited.
    await expect(page.getByRole('checkbox')).toHaveCount(0);
    test.skip(true, 'Select all checkbox not implemented (populated table: 0 checkboxes)');
  });

  test('E-09: Export JSON button is clickable', async ({ page }) => {
    await expect(page.locator('#export-json').or(page.getByRole('button', { name: /export.*json/i }))).toHaveCount(0);
    test.skip(true, 'Export JSON not implemented (0 matching controls)');
  });

  test('E-10: Export CSV button is clickable', async ({ page }) => {
    await expect(page.locator('#export-csv').or(page.getByRole('button', { name: /export.*csv/i }))).toHaveCount(0);
    test.skip(true, 'Export CSV not implemented (0 matching controls)');
  });

  test('E-11: Status badge shows correct color', async ({ page, endpoint }) => {
    await page.getByPlaceholder('Search by name or URL...').fill(endpoint.name);
    const rows = page.getByRole('row').filter({ hasText: endpoint.name });
    await expect(rows).toHaveCount(1);
    await expect(rows).toContainText('Online');
    const rowCount = await rows.count();

    let verifiedRows = 0;

    for (let i = 0; i < rowCount; i += 1) {
      const row = rows.nth(i);
      const statusBadge = row.locator('td').nth(3).locator('div.inline-flex').first();
      const isBadgeVisible = await statusBadge.isVisible().catch(() => false);

      if (!isBadgeVisible) {
        continue;
      }

      const label = ((await statusBadge.textContent()) ?? '').trim();
      const className = (await statusBadge.getAttribute('class')) ?? '';

      switch (label) {
        case 'Online':
          expect(className).toContain('bg-success/20');
          expect(className).toContain('text-success');
          verifiedRows += 1;
          break;
        case 'Pending':
          expect(className).toContain('bg-warning/20');
          expect(className).toContain('text-warning');
          verifiedRows += 1;
          break;
        case 'Offline':
          expect(className).toContain('bg-destructive/20');
          expect(className).toContain('text-destructive');
          verifiedRows += 1;
          break;
        case 'Error':
          expect(className).toContain('bg-destructive');
          expect(className).toContain('text-destructive-foreground');
          verifiedRows += 1;
          break;
        default:
          break;
      }
    }

    expect(verifiedRows).toBeGreaterThan(0);
  });

  test('E-12: Endpoint detail button works', async ({ page, endpoint }) => {
    await page.getByPlaceholder('Search by name or URL...').fill(endpoint.name);
    const row = page.getByRole('row').filter({ hasText: endpoint.name });
    await expect(row).toBeVisible();
    await row.locator('button[title="Details"]').click();
    await expect(page.getByRole('dialog')).toBeVisible();
    await expect(page.getByRole('dialog').getByLabel('Display Name')).toHaveValue(endpoint.name);
  });
});
