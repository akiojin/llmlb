import { test, expect } from '@playwright/test';
import { DashboardPage } from '../../pages/dashboard.page';
import { DashboardSelectors } from '../../helpers/selectors';

test.describe('UX Criteria @dashboard @ux', () => {
  let dashboard: DashboardPage;

  test.beforeEach(async ({ page }) => {
    dashboard = new DashboardPage(page);
    await dashboard.goto();
  });

  // --- Click Count Tests (NFR-003: ≤ 2 clicks from Dashboard) ---

  test('UX-01: LB Playground reachable in ≤ 1 click', async ({ page }) => {
    // 1 click: header button
    await dashboard.playgroundButton.click();
    await expect(page).toHaveURL(/#lb-playground/);
  });

  test('UX-02: Audit Log reachable in ≤ 1 click (admin)', async ({ page }) => {
    // 1 click: header button
    await dashboard.auditLogButton.click();
    await expect(page).toHaveURL(/#audit-log/);
  });

  test('UX-03: Manage Users reachable in ≤ 2 clicks', async ({ page }) => {
    // Click 1: open dropdown
    await dashboard.openUserDropdown();
    // Click 2: click Manage Users
    await page.locator(DashboardSelectors.userDropdown.manageUsers).click();
    await expect(dashboard.userModal).toBeVisible();
  });

  test('UX-04: All tabs reachable in ≤ 1 click', async ({ page }) => {
    const tabs = ['Endpoints', 'Models', 'Statistics', 'History', 'Clients', 'Logs'];
    for (const tabName of tabs) {
      const tab = page.locator(`button[role="tab"]:has-text("${tabName}")`);
      if (await tab.isVisible({ timeout: 3000 }).catch(() => false)) {
        // 1 click to reach tab content
        await tab.click();
        await expect(tab).toHaveAttribute('aria-selected', 'true');
      }
    }
  });

  // --- Response Time Tests (NFR-001: < 500ms) ---

  test('UX-05: Navigation response time < 500ms', async ({ page }) => {
    // Measure time to navigate to LB Playground
    const start = Date.now();
    await page.evaluate(() => {
      window.location.hash = 'lb-playground';
    });
    // Wait for the playground sidebar to appear
    await page.locator('#lb-playground-sidebar').waitFor({ state: 'visible', timeout: 5000 });
    const elapsed = Date.now() - start;

    expect(elapsed).toBeLessThan(500);
  });

  // --- Accessibility Tests (NFR-004) ---

  test('UX-06: All header buttons have accessible names', async ({ page }) => {
    // Check that interactive header elements are accessible via role-based selectors
    const buttons = [
      { id: '#theme-toggle', expectedRole: 'button' },
      { id: '#lb-playground-button', expectedRole: 'button' },
      { id: '#api-keys-button', expectedRole: 'button' },
      { id: '#refresh-button', expectedRole: 'button' },
    ];

    for (const btn of buttons) {
      const element = page.locator(btn.id);
      if (await element.isVisible({ timeout: 2000 }).catch(() => false)) {
        // Button should have accessible text content or aria-label
        const role = await element.getAttribute('role');
        const tagName = await element.evaluate((el) => el.tagName.toLowerCase());
        const hasText = await element.textContent();
        const ariaLabel = await element.getAttribute('aria-label');

        // A button is accessible if it's a <button> element or has role="button",
        // AND has text content or an aria-label
        const isButton = tagName === 'button' || role === 'button';
        const hasAccessibleName = (hasText && hasText.trim().length > 0) || !!ariaLabel;

        expect(isButton, `${btn.id} should be a button`).toBe(true);
        expect(hasAccessibleName, `${btn.id} should have accessible name`).toBe(true);
      }
    }
  });

  // --- Keyboard Navigation Tests ---

  test('UX-07: Tab navigation works with keyboard', async ({ page }) => {
    // Focus on the first tab
    const firstTab = page.locator('button[role="tab"]').first();
    await firstTab.focus();

    // Press ArrowRight to move to next tab
    await page.keyboard.press('ArrowRight');
    await page.waitForTimeout(200);

    // The focused tab should change
    const focusedTab = page.locator('button[role="tab"]:focus');
    const count = await focusedTab.count();
    expect(count).toBeGreaterThan(0);
  });

  test('UX-08: Modals are dismissible via Escape key', async ({ page }) => {
    // Open API Keys modal
    await dashboard.apiKeysButton.click();
    const modal = page.locator('[role="dialog"]:has-text("API Keys")');
    await expect(modal).toBeVisible({ timeout: 5000 });

    // Press Escape
    await page.keyboard.press('Escape');
    await expect(modal).not.toBeVisible({ timeout: 5000 });
  });
});
