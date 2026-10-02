import { DashboardPage } from '../../pages/dashboard.page';
import { DashboardSelectors } from '../../helpers/selectors';
import { DASHBOARD_SCREEN, expect, navigationCases, test } from '../../helpers/screen-registry';

test.describe('Screen Navigation @dashboard @navigation', () => {
  // Cases come from the canonical screen registry (SPEC #582 US-006):
  // every page, hash route, tab and modal in it is reachable.
  for (const { title, screen } of navigationCases()) {
    test(title, async ({ visit }) => {
      await visit(screen);
    });
  }

  test('NAV-06: User dropdown → Sign out → Login page', async ({ page, visit }) => {
    await visit(DASHBOARD_SCREEN);
    await new DashboardPage(page).signOut();
    await expect(page).toHaveURL(/login/);
  });

  test('NAV-08: Audit Log button is hidden for non-admin user', async ({ page, request, visit }) => {
    test.setTimeout(60000);
    await visit(DASHBOARD_SCREEN);
    const dashboard = new DashboardPage(page);
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
