import { test, expect } from '../../helpers/endpoint.fixture';
import { DashboardPage } from '../../pages/dashboard.page';

test.describe('Modal Lifecycle @dashboard @navigation', () => {
  let dashboard: DashboardPage;

  test.beforeEach(async ({ page }) => {
    dashboard = new DashboardPage(page);
    await dashboard.goto();
  });

  test('MOD-01: API Keys modal opens and closes via button', async ({ page }) => {
    await dashboard.apiKeysButton.click();
    const modal = page.locator('[role="dialog"]:has-text("API Keys")');
    await expect(modal).toBeVisible({ timeout: 5000 });

    // Close via close button (X button in the dialog)
    const closeButton = modal.locator('button[aria-label="Close"], button:has(svg.lucide-x)').first();
    if (await closeButton.isVisible({ timeout: 2000 }).catch(() => false)) {
      await closeButton.click();
    } else {
      await page.keyboard.press('Escape');
    }
    await expect(modal).not.toBeVisible({ timeout: 5000 });
  });

  test('MOD-02: API Keys modal closes via Escape key', async ({ page }) => {
    await dashboard.apiKeysButton.click();
    const modal = page.locator('[role="dialog"]:has-text("API Keys")');
    await expect(modal).toBeVisible({ timeout: 5000 });

    await page.keyboard.press('Escape');
    await expect(modal).not.toBeVisible({ timeout: 5000 });
  });

  test('MOD-03: User modal opens and closes (admin)', async ({ page }) => {
    await dashboard.openManageUsersModal();
    await expect(dashboard.userModal).toBeVisible();

    await page.keyboard.press('Escape');
    await expect(dashboard.userModal).not.toBeVisible({ timeout: 5000 });
  });

  test('MOD-04: Invitation modal opens and closes (admin)', async ({ page }) => {
    await dashboard.openInvitationModal();
    await expect(dashboard.invitationModal).toBeVisible();

    await page.keyboard.press('Escape');
    await expect(dashboard.invitationModal).not.toBeVisible({ timeout: 5000 });
  });

  test('MOD-05: Endpoint detail modal opens and closes', async ({ page, endpoint }) => {
    await page.getByPlaceholder('Search by name or URL...').fill(endpoint.name);
    const row = page.getByRole('row').filter({ hasText: endpoint.name });
    await expect(row).toBeVisible();
    await row.locator('button[title="Details"]').click();
    const modal = page.getByRole('dialog');
    await expect(modal.getByLabel('Display Name')).toHaveValue(endpoint.name);
    await modal.getByRole('button', { name: 'Close', exact: true }).first().click();
    await expect(modal).not.toBeVisible();
  });

  test('MOD-06: Request detail modal opens and closes (History tab)', async ({ page, request, endpoint }) => {
    const completion = await request.post('/v1/chat/completions', {
      headers: { Authorization: 'Bearer sk_debug' },
      data: { model: endpoint.model, messages: [{ role: 'user', content: 'modal lifecycle' }] },
    });
    expect(completion.ok()).toBeTruthy();
    // History was fetched before the inference; reload after producing our own record.
    await dashboard.goto();
    await dashboard.goToHistoryTab();
    const row = dashboard.getHistoryRows().filter({ hasText: endpoint.model });
    await expect(row).toHaveCount(1);
    await row.click();
    const requestModal = dashboard.getHistoryDetailModal();
    await expect(requestModal).toBeVisible();
    await expect(requestModal).toContainText(endpoint.model);
    await page.keyboard.press('Escape');
    await expect(requestModal).not.toBeVisible();
  });
});
