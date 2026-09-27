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
  const username = `e2e-cp-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
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
    newPassword: page.locator('#new-password'),
    confirmPassword: page.locator('#confirm-password'),
    submit: page.locator('button[type="submit"]'),
  };
}

test.describe('Change Password Page @dashboard @auth', () => {
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
    await expect(form.newPassword).toBeVisible();
    await expect(form.newPassword).toHaveAttribute('type', 'password');
    await expect(form.confirmPassword).toBeVisible();
    await expect(form.confirmPassword).toHaveAttribute('type', 'password');
    await expect(form.submit).toHaveText('Change Password');
    // Submit stays disabled until both fields are filled
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

  test('CP-06: unauthenticated access redirects to login', async ({ page }) => {
    await page.goto('/dashboard/change-password.html');
    await page.waitForURL(/\/dashboard\/login\.html/, { timeout: 10000 });
  });
});
