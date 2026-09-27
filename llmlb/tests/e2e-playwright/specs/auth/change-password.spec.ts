import { test, expect, type Page, type APIRequestContext } from '@playwright/test';
import { LoginPage } from '../../pages/auth.page';
import { createUser, deleteUser } from '../../helpers/api-helpers';

/**
 * Change Password screen (SPEC #582 US-006 / T008)
 *
 * Users created by an admin receive a server-generated password and must change
 * it on first sign-in. Each test provisions its own throwaway user so the shared
 * admin credentials are never modified.
 */

const CHANGE_PASSWORD_URL = /\/dashboard\/change-password\.html/;
const NEW_PASSWORD = 'NewPassw0rd';

interface ProvisionedUser {
  id: string;
  username: string;
  password: string;
}

async function provisionUser(request: APIRequestContext): Promise<ProvisionedUser> {
  const username = `e2e-cp-${Date.now()}-${Math.random().toString(36).slice(2, 8)}@example.com`;
  const user = await createUser(request, username, '', 'viewer');
  expect(user.id, 'user creation should succeed').toBeTruthy();
  expect(user.generated_password, 'server should return a generated password').toBeTruthy();
  return { id: user.id, username, password: user.generated_password! };
}

async function signInAndReachChangePassword(page: Page, user: ProvisionedUser) {
  const loginPage = new LoginPage(page);
  await loginPage.goto();
  await loginPage.login(user.username, user.password);
  await page.waitForURL(CHANGE_PASSWORD_URL, { timeout: 10000 });
  await expect(page.getByText('Change Password', { exact: true }).first()).toBeVisible({
    timeout: 10000,
  });
}

function changePasswordForm(page: Page) {
  return {
    currentPassword: page.locator('#current-password'),
    newPassword: page.locator('#new-password'),
    confirmPassword: page.locator('#confirm-password'),
    submit: page.locator('button[type="submit"]'),
  };
}

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Change Password Page (${colorScheme}) @dashboard @auth`, () => {
    test.use({ colorScheme });

    let user: ProvisionedUser;
    let pageErrors: string[];

    test.beforeEach(async ({ page, request }) => {
      pageErrors = [];
      page.on('pageerror', (err) => pageErrors.push(err.message));
      user = await provisionUser(request);
    });

    test.afterEach(async ({ request }) => {
      if (user?.id) {
        await deleteUser(request, user.id);
      }
      expect(pageErrors, 'no uncaught page errors').toEqual([]);
    });

    test('CP-01: first sign-in redirects to the change password screen', async ({ page }) => {
      await signInAndReachChangePassword(page, user);

      const form = changePasswordForm(page);
      await expect(page.getByText('You must change your password before continuing.')).toBeVisible();
      await expect(form.currentPassword).toBeVisible();
      await expect(form.currentPassword).toHaveAttribute('type', 'password');
      await expect(form.newPassword).toBeVisible();
      await expect(form.newPassword).toHaveAttribute('type', 'password');
      await expect(form.confirmPassword).toBeVisible();
      await expect(form.confirmPassword).toHaveAttribute('type', 'password');
      await expect(form.submit).toHaveText('Change Password');
      // Forced change: no way back to the dashboard
      await expect(page.getByRole('link', { name: 'Back to dashboard' })).toHaveCount(0);
      // Submit stays disabled until all three fields are filled
      await expect(form.submit).toBeDisabled();
      await form.currentPassword.fill(user.password);
      await expect(form.submit).toBeDisabled();
      await form.newPassword.fill(NEW_PASSWORD);
      await expect(form.submit).toBeDisabled();
      await form.confirmPassword.fill(NEW_PASSWORD);
      await expect(form.submit).toBeEnabled();
    });

    test('CP-02: weak password is rejected without leaving the page', async ({ page }) => {
      await signInAndReachChangePassword(page, user);

      let changeRequests = 0;
      page.on('request', (req) => {
        if (req.url().includes('/api/auth/change-password')) changeRequests += 1;
      });

      const form = changePasswordForm(page);
      await form.currentPassword.fill(user.password);
      await form.newPassword.fill('weakpass');
      await form.confirmPassword.fill('weakpass');
      await form.submit.click();

      await expect(page.getByText('Password too weak', { exact: true })).toBeVisible({ timeout: 5000 });
      await expect(page).toHaveURL(CHANGE_PASSWORD_URL);
      expect(changeRequests).toBe(0);
    });

    test('CP-03: mismatched confirmation is rejected', async ({ page }) => {
      await signInAndReachChangePassword(page, user);

      const form = changePasswordForm(page);
      await form.currentPassword.fill(user.password);
      await form.newPassword.fill(NEW_PASSWORD);
      await form.confirmPassword.fill(`${NEW_PASSWORD}x`);
      await form.submit.click();

      await expect(page.getByText('Passwords do not match', { exact: true })).toBeVisible({ timeout: 5000 });
      await expect(page).toHaveURL(CHANGE_PASSWORD_URL);
    });

    test('CP-04: server failure shows an error and keeps the user on the page', async ({ page }) => {
      await signInAndReachChangePassword(page, user);

      await page.route('**/api/auth/change-password', (route) =>
        route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ error: 'internal error' }),
        })
      );

      const form = changePasswordForm(page);
      await form.currentPassword.fill(user.password);
      await form.newPassword.fill(NEW_PASSWORD);
      await form.confirmPassword.fill(NEW_PASSWORD);
      await form.submit.click();

      await expect(page.getByText('Failed to change password', { exact: true })).toBeVisible({
        timeout: 5000,
      });
      await expect(page).toHaveURL(CHANGE_PASSWORD_URL);
      await expect(form.submit).toBeEnabled();
    });

    test('CP-05: successful change redirects to login and the new password works', async ({
      page,
    }) => {
      await signInAndReachChangePassword(page, user);

      const form = changePasswordForm(page);
      await form.currentPassword.fill(user.password);
      await form.newPassword.fill(NEW_PASSWORD);
      await form.confirmPassword.fill(NEW_PASSWORD);
      await form.submit.click();

      await expect(page.getByText('Password changed', { exact: true })).toBeVisible({ timeout: 5000 });
      await page.waitForURL(/\/dashboard\/login\.html/, { timeout: 10000 });

      // Old password no longer works
      const loginPage = new LoginPage(page);
      await loginPage.login(user.username, user.password);
      await expect(page.getByText('Login failed', { exact: true })).toBeVisible({ timeout: 5000 });
      await expect(page).toHaveURL(/\/dashboard\/login\.html/);

      // New password signs in straight to the dashboard (no forced change anymore)
      await loginPage.goto();
      await loginPage.login(user.username, NEW_PASSWORD);
      await page.waitForURL(
        (url) =>
          url.pathname.startsWith('/dashboard') &&
          !url.pathname.includes('login') &&
          !url.pathname.includes('change-password'),
        { timeout: 10000 }
      );
    });

    test('CP-07: wrong current password is rejected and the old password keeps working', async ({
      page,
    }) => {
      await signInAndReachChangePassword(page, user);

      const form = changePasswordForm(page);
      await form.currentPassword.fill('WrongPassw0rd');
      await form.newPassword.fill(NEW_PASSWORD);
      await form.confirmPassword.fill(NEW_PASSWORD);
      await form.submit.click();

      await expect(page.getByText('Current password is incorrect', { exact: true })).toBeVisible({
        timeout: 5000,
      });
      await expect(page).toHaveURL(CHANGE_PASSWORD_URL);

      // Password is unchanged: signing in again with the original password still works
      const loginPage = new LoginPage(page);
      await loginPage.goto();
      await loginPage.login(user.username, user.password);
      await page.waitForURL(CHANGE_PASSWORD_URL, { timeout: 10000 });
    });

    test('CP-08: signed-in users can change their password from the user menu', async ({
      page,
    }) => {
      // Complete the forced change first so the user lands on the dashboard
      await signInAndReachChangePassword(page, user);
      let form = changePasswordForm(page);
      await form.currentPassword.fill(user.password);
      await form.newPassword.fill(NEW_PASSWORD);
      await form.confirmPassword.fill(NEW_PASSWORD);
      await form.submit.click();
      await page.waitForURL(/\/dashboard\/login\.html/, { timeout: 10000 });

      const loginPage = new LoginPage(page);
      await loginPage.login(user.username, NEW_PASSWORD);
      await page.waitForSelector('#theme-toggle', { timeout: 15000 });

      await page.getByRole('button', { name: 'User menu' }).click();
      await page.getByRole('menuitem', { name: 'Change Password' }).click();
      await page.waitForURL(CHANGE_PASSWORD_URL, { timeout: 10000 });
      await expect(page.getByText('Update the password you use to sign in.')).toBeVisible();
      await expect(page.getByRole('link', { name: 'Back to dashboard' })).toBeVisible();

      const secondPassword = 'SecondPassw0rd';
      form = changePasswordForm(page);
      await form.currentPassword.fill(NEW_PASSWORD);
      await form.newPassword.fill(secondPassword);
      await form.confirmPassword.fill(secondPassword);
      await form.submit.click();
      await expect(page.getByText('Password changed', { exact: true })).toBeVisible({ timeout: 5000 });
      await page.waitForURL(/\/dashboard\/login\.html/, { timeout: 10000 });

      await loginPage.login(user.username, secondPassword);
      await page.waitForSelector('#theme-toggle', { timeout: 15000 });
    });

    test('CP-06: unauthenticated access redirects to login', async ({ page }) => {
      await page.goto('/dashboard/change-password.html');
      await page.waitForURL(/\/dashboard\/login\.html/, { timeout: 10000 });
    });
  });
}
