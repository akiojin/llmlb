import { test, expect } from '@playwright/test';
import { DashboardPage } from '../../pages/dashboard.page';
import { DashboardSelectors } from '../../helpers/selectors';

/** Dashboard returns to /dashboard/ or /dashboard/# after back navigation */
const DASHBOARD_URL_PATTERN = /\/dashboard\/(#?)$/;

test.describe('Screen Navigation @dashboard @navigation', () => {
  let dashboard: DashboardPage;

  test.beforeEach(async ({ page }) => {
    dashboard = new DashboardPage(page);
    await dashboard.goto();
  });

  test('NAV-01: Dashboard → LB Playground → Back to Dashboard', async ({ page }) => {
    await dashboard.openPlayground();
    await expect(page).toHaveURL(/#lb-playground/);

    // PlaygroundBase uses "Back to Dashboard" button
    const backButton = page.getByRole('button', { name: /back to dashboard/i });
    await backButton.click();
    await page.waitForTimeout(500);

    expect(page.url()).toMatch(DASHBOARD_URL_PATTERN);
  });

  test('NAV-02: Dashboard → Audit Log → Back to Dashboard (admin)', async ({ page }) => {
    await expect(dashboard.auditLogButton).toBeVisible({ timeout: 5000 });

    await dashboard.openAuditLog();
    await expect(page).toHaveURL(/#audit-log/);

    // AuditLog uses "Dashboard" button with ChevronLeft icon
    const backButton = page.getByRole('button', { name: /dashboard/i }).first();
    await backButton.click();
    await page.waitForTimeout(500);

    expect(page.url()).toMatch(DASHBOARD_URL_PATTERN);
  });

  test('NAV-03: Dashboard → Endpoint Playground via detail modal', async ({ page }) => {
    const rows = page.locator('#nodes-body tr');
    const rowCount = await rows.count();
    test.skip(rowCount === 0, 'No endpoints available to test');

    const detailButton = rows.first().locator('button[title="Details"]');
    if (await detailButton.isVisible({ timeout: 3000 }).catch(() => false)) {
      await detailButton.click();
      await page.waitForTimeout(500);

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
    const tabs = ['Endpoints', 'Models', 'Usage', 'Requests', 'Traffic', 'System'];

    for (const tabName of tabs) {
      const tab = page.locator(`button[role="tab"]:has-text("${tabName}")`);
      if (await tab.isVisible({ timeout: 3000 }).catch(() => false)) {
        await tab.click();
        await expect(tab).toHaveAttribute('aria-selected', 'true');
      }
    }
  });

  test('NAV-08: Audit Log button is hidden for non-admin user', async ({ page, request }) => {
    test.setTimeout(60000);
    const { createUser, deleteUser, listUsers } = await import('../../helpers/api-helpers');
    const viewerUsername = `viewer_nav_${Date.now()}`;
    const result = await createUser(request, viewerUsername, '', 'viewer');
    const generatedPassword = (result as { generated_password?: string }).generated_password;
    test.skip(!generatedPassword, 'Failed to create viewer user');

    try {
      // Log out and log in as viewer
      await dashboard.signOut();

      await page.fill('#username', viewerUsername);
      await page.fill('#password', generatedPassword!);
      await page.click('button[type="submit"]');

      // Login lands on either the forced password change or the dashboard.
      // Wait for whichever page renders instead of a navigation event.
      const newPasswordInput = page.locator('#new-password');
      const dashboardContent = page.locator('#theme-toggle');
      await expect(newPasswordInput.or(dashboardContent)).toBeVisible({ timeout: 15000 });
      const newPassword = 'ViewerPass123!';

      // Handle password change if required (must_change_password)
      if (await newPasswordInput.isVisible()) {
        await newPasswordInput.fill(newPassword);
        await page.fill('#confirm-password', newPassword);
        await page.click('button[type="submit"]');

        // Password change redirects to the login page after a delay
        await expect(page).toHaveURL(/\/login/, { timeout: 15000 });
        await expect(page.locator('#username')).toBeVisible();

        // Re-login with the new password
        await page.fill('#username', viewerUsername);
        await page.fill('#password', newPassword);
        await page.click('button[type="submit"]');
      }

      // Wait for dashboard to fully load (whether password change happened or not)
      await expect(page).not.toHaveURL(/login|change-password/, { timeout: 15000 });
      await expect(dashboardContent).toBeVisible({ timeout: 15000 });

      // Audit Log button should NOT be visible
      const auditLogBtn = page.locator(DashboardSelectors.header.auditLogButton);
      await expect(auditLogBtn).not.toBeVisible();

      // User dropdown should NOT contain admin-only items
      await page.locator(DashboardSelectors.header.userDropdownTrigger).click();
      await page.waitForTimeout(300);
      await expect(page.locator(DashboardSelectors.userDropdown.manageUsers)).not.toBeVisible();
      await expect(page.locator(DashboardSelectors.userDropdown.invitationCodes)).not.toBeVisible();
    } finally {
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
