import { test, expect } from '@playwright/test';
import { DashboardPage } from '../../pages/dashboard.page';
import { DashboardSelectors } from '../../helpers/selectors';

test.describe('Screen Navigation @dashboard @navigation', () => {
  let dashboard: DashboardPage;

  test.beforeEach(async ({ page }) => {
    dashboard = new DashboardPage(page);
    await dashboard.goto();
  });

  test('NAV-01: Dashboard → LB Playground → Back to Dashboard', async ({ page }) => {
    // Navigate to LB Playground via header button
    await dashboard.openPlayground();
    await expect(page).toHaveURL(/#lb-playground/);

    // Click back button to return to dashboard
    const backButton = page.getByRole('button', { name: /back/i }).first();
    await backButton.click();
    await page.waitForTimeout(500);

    // Should be back on dashboard (hash is empty or absent)
    expect(page.url()).toMatch(/\/dashboard\/?$/);
  });

  test('NAV-02: Dashboard → Audit Log → Back to Dashboard (admin)', async ({ page }) => {
    // Audit Log button should be visible for admin user
    await expect(dashboard.auditLogButton).toBeVisible({ timeout: 5000 });

    // Navigate to Audit Log
    await dashboard.openAuditLog();
    await expect(page).toHaveURL(/#audit-log/);

    // Click back button to return to dashboard
    const backButton = page.getByRole('button', { name: /back/i }).first();
    await backButton.click();
    await page.waitForTimeout(500);

    expect(page.url()).toMatch(/\/dashboard\/?$/);
  });

  test('NAV-03: Dashboard → Endpoint Playground via detail modal', async ({ page }) => {
    // Check if there are endpoints in the table
    const rows = page.locator('#nodes-body tr');
    const rowCount = await rows.count();
    test.skip(rowCount === 0, 'No endpoints available to test');

    // Click on first endpoint row to open detail modal
    const detailButton = rows.first().locator('button[title="Details"]');
    if (await detailButton.isVisible({ timeout: 3000 }).catch(() => false)) {
      await detailButton.click();
      await page.waitForTimeout(500);

      // Look for "Open Playground" button in the modal
      const openPlayground = page.getByRole('button', { name: /open playground/i }).first();
      if (await openPlayground.isVisible({ timeout: 3000 }).catch(() => false)) {
        await openPlayground.click();
        await expect(page).toHaveURL(/#playground\//);
      }
    }
  });

  test('NAV-04: User dropdown → Manage Users opens modal (admin)', async ({ page }) => {
    await dashboard.openManageUsersModal();
    await expect(dashboard.userModal).toBeVisible();
  });

  test('NAV-05: User dropdown → Invitation Codes opens modal (admin)', async ({ page }) => {
    await dashboard.openInvitationModal();
    await expect(dashboard.invitationModal).toBeVisible();
  });

  test('NAV-06: User dropdown → Sign out → Login page', async ({ page }) => {
    await dashboard.signOut();
    await expect(page).toHaveURL(/login/);
  });

  test('NAV-07: Tab switching cycles through all tabs', async ({ page }) => {
    const tabs = ['Endpoints', 'Models', 'Statistics', 'History', 'Clients', 'Logs'];

    for (const tabName of tabs) {
      const tab = page.locator(`button[role="tab"]:has-text("${tabName}")`);
      if (await tab.isVisible({ timeout: 3000 }).catch(() => false)) {
        await tab.click();
        await expect(tab).toHaveAttribute('aria-selected', 'true');
      }
    }
  });

  test('NAV-08: Audit Log button is hidden for non-admin user', async ({ page, request }) => {
    // Create a viewer user via API
    const { createUser, deleteUser, listUsers } = await import('../../helpers/api-helpers');
    const viewerUsername = `viewer_nav_${Date.now()}`;
    const result = await createUser(request, viewerUsername, '', 'viewer');
    const generatedPassword = (result as { generated_password?: string }).generated_password;
    test.skip(!generatedPassword, 'Failed to create viewer user');

    try {
      // Log out and log in as viewer
      await dashboard.signOut();

      // Login as viewer
      await page.fill('#username', viewerUsername);
      await page.fill('#password', generatedPassword!);
      await page.click('button[type="submit"]');

      // Handle password change if required
      if (page.url().includes('change-password')) {
        const newPassword = 'ViewerPass123!';
        await page.fill('#new-password', newPassword);
        await page.fill('#confirm-password', newPassword);
        await page.click('button[type="submit"]');
        await page.waitForFunction(() => !window.location.href.includes('change-password'), {
          timeout: 10000,
        });
      }

      await page.waitForSelector('#theme-toggle', { timeout: 10000 });

      // Audit Log button should NOT be visible
      const auditLogBtn = page.locator(DashboardSelectors.header.auditLogButton);
      await expect(auditLogBtn).not.toBeVisible();

      // User dropdown should NOT contain admin-only items
      await page.locator(DashboardSelectors.header.userDropdownTrigger).click();
      await page.waitForTimeout(300);
      await expect(page.locator(DashboardSelectors.userDropdown.manageUsers)).not.toBeVisible();
      await expect(page.locator(DashboardSelectors.userDropdown.invitationCodes)).not.toBeVisible();
    } finally {
      // Cleanup: delete the viewer user
      if (result.id) {
        await deleteUser(request, result.id);
      } else {
        const users = await listUsers(request);
        const viewer = users.find((u) => u.username === viewerUsername);
        if (viewer) await deleteUser(request, viewer.id);
      }
    }
  });
});
