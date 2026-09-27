import { test, expect, type Page } from '@playwright/test';
import { LoginPage } from '../../pages/auth.page';

/**
 * Forgot / Reset Password screens (SPEC #580 US-008, Issue #695)
 *
 * The server never returns reset tokens to the browser (the link is written to the server
 * log), so flows that need a *valid* token stub the reset API. Contract tests cover the
 * token lifecycle itself (issue / expiry / single use).
 */

const FORGOT_URL = /\/dashboard\/forgot-password\.html/;
const RESET_URL = /\/dashboard\/reset-password\.html/;
const NEW_PASSWORD = 'ResetPassw0rd';

function resetForm(page: Page) {
  return {
    token: page.locator('#reset-token'),
    newPassword: page.locator('#new-password'),
    confirmPassword: page.locator('#confirm-password'),
    submit: page.locator('button[type="submit"]'),
  };
}

test.describe('Password Reset Pages @dashboard @auth', () => {
  let pageErrors: string[];

  test.beforeEach(async ({ page }) => {
    pageErrors = [];
    page.on('pageerror', (err) => pageErrors.push(err.message));
  });

  test.afterEach(() => {
    expect(pageErrors, 'no uncaught page errors').toEqual([]);
  });

  test('PR-01: login page links to the forgot password screen', async ({ page }) => {
    const loginPage = new LoginPage(page);
    await loginPage.goto();

    await page.getByRole('link', { name: 'Forgot password?' }).click();
    await page.waitForURL(FORGOT_URL, { timeout: 10000 });
    await expect(page.getByText('Forgot Password', { exact: true }).first()).toBeVisible();
    await expect(page.locator('#email')).toHaveAttribute('type', 'email');
  });

  test('PR-02: requesting a reset shows the same confirmation for any email', async ({ page }) => {
    let forgotRequests = 0;
    page.on('request', (req) => {
      if (req.url().includes('/api/auth/forgot-password')) forgotRequests += 1;
    });

    await page.goto('/dashboard/forgot-password.html');
    const submit = page.locator('button[type="submit"]');
    await expect(submit).toBeDisabled();

    const email = `nobody-${Date.now()}@example.com`;
    await page.locator('#email').fill(email);
    const responsePromise = page.waitForResponse('**/api/auth/forgot-password');
    await submit.click();
    const response = await responsePromise;
    expect(response.status()).toBe(202);
    const body = await response.json();
    expect(body.token, 'token must never reach the browser').toBeUndefined();

    await expect(page.getByText('Reset requested', { exact: true })).toBeVisible({ timeout: 5000 });
    await expect(
      page.getByText('If an account exists for this email, a password reset link has been issued.', {
        exact: false,
      })
    ).toBeVisible();
    expect(forgotRequests).toBe(1);

    await page.getByRole('link', { name: 'I have a reset link' }).click();
    await page.waitForURL(RESET_URL, { timeout: 10000 });
    await expect(resetForm(page).token).toBeVisible();
  });

  test('PR-03: an unknown or expired token is rejected by the server', async ({ page }) => {
    await page.goto('/dashboard/reset-password.html#token=not-a-real-token');

    const form = resetForm(page);
    // Token came from the link, so no manual token field and the fragment is stripped
    await expect(form.token).toHaveCount(0);
    await expect(page).toHaveURL(/\/dashboard\/reset-password\.html$/);

    await form.newPassword.fill(NEW_PASSWORD);
    await form.confirmPassword.fill(NEW_PASSWORD);
    const responsePromise = page.waitForResponse('**/api/auth/reset-password');
    await form.submit.click();
    expect((await responsePromise).status()).toBe(400);

    await expect(page.getByText('Reset link is invalid or expired', { exact: true })).toBeVisible({
      timeout: 5000,
    });
    await expect(page).toHaveURL(RESET_URL);
  });

  test('PR-04: weak or mismatched passwords never reach the server', async ({ page }) => {
    let resetRequests = 0;
    page.on('request', (req) => {
      if (req.url().includes('/api/auth/reset-password')) resetRequests += 1;
    });

    await page.goto('/dashboard/reset-password.html#token=some-token');
    const form = resetForm(page);

    await form.newPassword.fill('weakpass');
    await form.confirmPassword.fill('weakpass');
    await form.submit.click();
    await expect(page.getByText('Password too weak', { exact: true })).toBeVisible({ timeout: 5000 });

    await form.newPassword.fill(NEW_PASSWORD);
    await form.confirmPassword.fill(`${NEW_PASSWORD}x`);
    await form.submit.click();
    await expect(page.getByText('Passwords do not match', { exact: true })).toBeVisible({
      timeout: 5000,
    });

    expect(resetRequests).toBe(0);
  });

  test('PR-05: a valid link resets the password and leads back to sign in', async ({ page }) => {
    let sentBody: Record<string, unknown> | null = null;
    await page.route('**/api/auth/reset-password', async (route) => {
      sentBody = route.request().postDataJSON();
      await route.fulfill({ status: 204 });
    });

    await page.goto('/dashboard/reset-password.html#token=link-token-123');
    const form = resetForm(page);
    await expect(form.submit).toBeDisabled();
    await form.newPassword.fill(NEW_PASSWORD);
    await form.confirmPassword.fill(NEW_PASSWORD);
    await expect(form.submit).toBeEnabled();
    await form.submit.click();

    await expect(page.getByText('Password updated', { exact: true })).toBeVisible({ timeout: 5000 });
    expect(sentBody).toEqual({ token: 'link-token-123', new_password: NEW_PASSWORD });

    await page.getByRole('link', { name: 'Go to sign in' }).click();
    await page.waitForURL(/\/dashboard\/login\.html/, { timeout: 10000 });
  });

  test('PR-06: a token can be pasted when opening the page without a link', async ({ page }) => {
    let sentBody: Record<string, unknown> | null = null;
    await page.route('**/api/auth/reset-password', async (route) => {
      sentBody = route.request().postDataJSON();
      await route.fulfill({ status: 204 });
    });

    await page.goto('/dashboard/reset-password.html');
    const form = resetForm(page);
    await form.newPassword.fill(NEW_PASSWORD);
    await form.confirmPassword.fill(NEW_PASSWORD);
    await expect(form.submit).toBeDisabled();
    await form.token.fill('  pasted-token  ');
    await form.submit.click();

    await expect(page.getByText('Password updated', { exact: true })).toBeVisible({ timeout: 5000 });
    expect(sentBody).toEqual({ token: 'pasted-token', new_password: NEW_PASSWORD });
  });
});
