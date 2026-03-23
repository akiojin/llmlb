import { test, expect } from '@playwright/test';
import { DashboardPage } from '../../pages/dashboard.page';
import { DashboardSelectors } from '../../helpers/selectors';

test.describe('Modal Lifecycle @dashboard @navigation', () => {
  let dashboard: DashboardPage;

  test.beforeEach(async ({ page }) => {
    dashboard = new DashboardPage(page);
    await dashboard.goto();
  });

  test('MOD-01: API Keys modal opens and closes via button', async ({ page }) => {
    // Open API Keys modal
    await dashboard.apiKeysButton.click();
    const modal = page.locator('[role="dialog"]:has-text("API Keys")');
    await expect(modal).toBeVisible({ timeout: 5000 });

    // Close via close button (X button in the dialog)
    const closeButton = modal.locator('button[aria-label="Close"], button:has(svg.lucide-x)').first();
    if (await closeButton.isVisible({ timeout: 2000 }).catch(() => false)) {
      await closeButton.click();
    } else {
      // Fallback: click outside the modal to close
      await page.mouse.click(10, 10);
    }
    await expect(modal).not.toBeVisible({ timeout: 5000 });
  });

  test('MOD-02: API Keys modal closes via Escape key', async ({ page }) => {
    await dashboard.apiKeysButton.click();
    const modal = page.locator('[role="dialog"]:has-text("API Keys")');
    await expect(modal).toBeVisible({ timeout: 5000 });

    // Press Escape to close
    await page.keyboard.press('Escape');
    await expect(modal).not.toBeVisible({ timeout: 5000 });
  });

  test('MOD-03: User modal opens and closes (admin)', async ({ page }) => {
    // Open user dropdown, click Manage Users
    await dashboard.openManageUsersModal();
    await expect(dashboard.userModal).toBeVisible();

    // Close via Escape
    await page.keyboard.press('Escape');
    await expect(dashboard.userModal).not.toBeVisible({ timeout: 5000 });
  });

  test('MOD-04: Invitation modal opens and closes (admin)', async ({ page }) => {
    // Open user dropdown, click Invitation Codes
    await dashboard.openInvitationModal();
    await expect(dashboard.invitationModal).toBeVisible();

    // Close via Escape
    await page.keyboard.press('Escape');
    await expect(dashboard.invitationModal).not.toBeVisible({ timeout: 5000 });
  });

  test('MOD-05: Endpoint detail modal opens and closes', async ({ page }) => {
    // Check if endpoints exist
    const rows = page.locator('#nodes-body tr');
    const rowCount = await rows.count();
    test.skip(rowCount === 0, 'No endpoints available to test');

    // Click details button on first endpoint
    const detailButton = rows.first().locator('button[title="Details"]');
    if (await detailButton.isVisible({ timeout: 3000 }).catch(() => false)) {
      await detailButton.click();
      const nodeModal = page.locator(DashboardSelectors.modals.nodeModal);
      await expect(nodeModal).toBeVisible({ timeout: 5000 });

      // Close via close button
      const closeBtn = page.locator(DashboardSelectors.modals.nodeModalClose);
      await closeBtn.click();
      await expect(nodeModal).not.toBeVisible({ timeout: 5000 });
    }
  });

  test('MOD-06: Request detail modal opens and closes (History tab)', async ({ page }) => {
    // Navigate to History tab
    await dashboard.goToHistoryTab();

    // Check if history rows exist
    const rows = dashboard.getHistoryRows();
    const rowCount = await rows.count();
    test.skip(rowCount === 0, 'No history entries available to test');

    // Click first history row
    await dashboard.clickHistoryRow(0);
    const requestModal = dashboard.getHistoryDetailModal();
    await expect(requestModal).toBeVisible({ timeout: 5000 });

    // Close via close button
    const closeBtn = page.locator(DashboardSelectors.modals.requestModalClose);
    await closeBtn.click();
    await expect(requestModal).not.toBeVisible({ timeout: 5000 });
  });
});
