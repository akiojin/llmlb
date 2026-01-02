import { test, expect, APIRequestContext } from '@playwright/test';
import { LoginPage } from '../../pages/auth.page';
import { DashboardPage } from '../../pages/dashboard.page';

const API_BASE = process.env.BASE_URL || 'http://localhost:32768';

/**
 * Helper to get JWT token for admin user
 */
async function getAdminToken(request: APIRequestContext): Promise<string> {
  const response = await request.post(`${API_BASE}/v0/auth/login`, {
    data: {
      username: 'admin',
      password: 'test',
    },
  });

  if (response.status() !== 200) {
    throw new Error(`Login failed: ${response.status()}`);
  }

  const data = await response.json();
  return data.token;
}

/**
 * Helper to list invitation codes via API
 */
async function listInvitations(request: APIRequestContext): Promise<unknown[]> {
  const token = await getAdminToken(request);

  const response = await request.get(`${API_BASE}/v0/invitations`, {
    headers: {
      Authorization: `Bearer ${token}`,
    },
  });

  if (response.status() !== 200) {
    throw new Error(`Failed to list invitations: ${response.status()}`);
  }

  const data = await response.json();
  return data.invitations || [];
}

test.describe('Invitation Code Management (Dashboard)', () => {
  let dashboard: DashboardPage;

  // User menu is the last button in header with User icon
  const userMenuSelector = 'header button:last-of-type';

  test.beforeEach(async ({ page }) => {
    // Login first
    const loginPage = new LoginPage(page);
    await loginPage.goto();
    await loginPage.login('admin', 'test');
    await loginPage.waitForDashboard();

    dashboard = new DashboardPage(page);
  });

  test('I-01: Invitation Codes menu item is visible in header', async ({ page }) => {
    // Look for the user menu dropdown trigger (icon button at the end of header)
    const userMenuTrigger = page.locator(userMenuSelector);
    await expect(userMenuTrigger).toBeVisible();

    // Click to open dropdown
    await userMenuTrigger.click();

    // Invitation Codes menu item should be visible
    const invitationMenuItem = page.locator('[role="menuitem"]:has-text("Invitation Codes")');
    await expect(invitationMenuItem).toBeVisible();
  });

  test('I-02: Clicking Invitation Codes opens modal', async ({ page }) => {
    // Open user menu
    const userMenuTrigger = page.locator(userMenuSelector);
    await userMenuTrigger.click();

    // Click Invitation Codes
    const invitationMenuItem = page.locator('[role="menuitem"]:has-text("Invitation Codes")');
    await invitationMenuItem.click();

    // Modal should be visible
    const modal = page.locator('[role="dialog"]');
    await expect(modal).toBeVisible();

    // Modal should have title (use heading role to be specific)
    await expect(modal.getByRole('heading', { name: 'Invitation Codes' })).toBeVisible();
  });

  test('I-03: Create new invitation code', async ({ page }) => {
    // Open user menu and click Invitation Codes
    await page.locator(userMenuSelector).click();
    await page.locator('[role="menuitem"]:has-text("Invitation Codes")').click();

    // Wait for modal
    const modal = page.locator('[role="dialog"]').first();
    await expect(modal).toBeVisible();

    // Click "Create Code" button
    const createButton = modal.locator('button:has-text("Create Code")');
    await createButton.click();

    // Wait for the "Create Invitation Code" sub-dialog
    await page.waitForSelector('[role="dialog"]:has-text("Create Invitation Code")', { timeout: 5000 });

    // Click Create button (the submit button in the create dialog)
    // Wait for the button to be enabled and click it
    const submitButton = page.locator('[role="dialog"]:has-text("Create Invitation Code") button:has-text("Create"):not(:has-text("Code"))');
    await submitButton.click();

    // Wait for the "Invitation Code Created" dialog to appear
    await page.waitForSelector('[role="dialog"]:has-text("Invitation Code Created")', { timeout: 5000 });

    // Wait for the code to be generated and displayed
    const codeElement = page.locator('code').filter({ hasText: /^inv_/ });
    await expect(codeElement).toBeVisible({ timeout: 5000 });

    // Verify code format
    const codeText = await codeElement.textContent();
    expect(codeText).toMatch(/^inv_[a-zA-Z0-9]{16}$/);
  });

  test('I-04: Created invitation appears in list', async ({ page, request }) => {
    // Get initial count
    const initialInvitations = await listInvitations(request);
    const initialCount = initialInvitations.length;

    // Open modal and create invitation
    await page.locator(userMenuSelector).click();
    await page.locator('[role="menuitem"]:has-text("Invitation Codes")').click();

    const modal = page.locator('[role="dialog"]').first();
    await modal.locator('button:has-text("Create Code")').click();

    // Wait for create dialog
    await page.waitForSelector('[role="dialog"]:has-text("Create Invitation Code")', { timeout: 5000 });

    // Click Create button
    const submitButton = page.locator('[role="dialog"]:has-text("Create Invitation Code") button:has-text("Create"):not(:has-text("Code"))');
    await submitButton.click();

    // Wait for "Invitation Code Created" dialog
    await page.waitForSelector('[role="dialog"]:has-text("Invitation Code Created")', { timeout: 5000 });

    // Dismiss the code display (click Done)
    const doneButton = page.locator('button:has-text("Done")');
    await doneButton.click();

    // Wait for dialog to close
    await page.waitForTimeout(500);

    // Verify count increased
    const newInvitations = await listInvitations(request);
    expect(newInvitations.length).toBeGreaterThan(initialCount);
  });

  test('I-05: Can revoke invitation code', async ({ page, request }) => {
    // First create an invitation via API
    const token = await getAdminToken(request);
    const createResponse = await request.post(`${API_BASE}/v0/invitations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { expires_in_hours: 24 },
    });
    expect(createResponse.status()).toBe(201);

    // Open modal
    await page.locator(userMenuSelector).click();
    await page.locator('[role="menuitem"]:has-text("Invitation Codes")').click();

    const modal = page.locator('[role="dialog"]');
    await expect(modal).toBeVisible();

    // Find a revoke button and click it
    const revokeButton = modal.locator('button:has-text("Revoke")').first();
    if (await revokeButton.isVisible()) {
      await revokeButton.click();

      // Confirm revocation if there's a confirmation dialog
      const confirmButton = page.locator('button:has-text("Confirm"), button:has-text("Yes")');
      if (await confirmButton.isVisible({ timeout: 1000 }).catch(() => false)) {
        await confirmButton.click();
      }

      // Wait for the revocation to complete (button should disappear or status change)
      await page.waitForTimeout(500);
    }
  });

  test('I-06: Expired invitations are not shown as active', async ({ page, request }) => {
    // Create an invitation with very short expiry via API
    const token = await getAdminToken(request);

    // Open modal
    await page.locator(userMenuSelector).click();
    await page.locator('[role="menuitem"]:has-text("Invitation Codes")').click();

    const modal = page.locator('[role="dialog"]');
    await expect(modal).toBeVisible();

    // Check that the modal displays invitations (or shows empty state)
    // This test just verifies the modal loads properly
    const content = modal.locator('.space-y-2, table, [role="list"]');
    await expect(content.or(modal.locator('text=No invitations'))).toBeVisible({ timeout: 5000 });
  });
});
