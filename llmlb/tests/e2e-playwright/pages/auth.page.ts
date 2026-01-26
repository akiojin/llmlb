import { type Page, type Locator } from '@playwright/test';

/**
 * Page Object Model for Login Page
 */
export class LoginPage {
  readonly page: Page;
  readonly usernameInput: Locator;
  readonly passwordInput: Locator;
  readonly submitButton: Locator;
  readonly registerLink: Locator;
  readonly errorToast: Locator;

  constructor(page: Page) {
    this.page = page;
    this.usernameInput = page.locator('#username');
    this.passwordInput = page.locator('#password');
    this.submitButton = page.locator('button[type="submit"]');
    this.registerLink = page.locator('a[href*="register"]');
    // Radix UI toast (data-state="open") - use first() to handle multiple matches
    this.errorToast = page.locator('[data-state="open"][data-radix-collection-item]').first();
  }

  async goto() {
    const url = '/dashboard/login.html';
    for (let attempt = 0; attempt < 2; attempt += 1) {
      try {
        await this.page.goto(url, { waitUntil: 'domcontentloaded' });
        return;
      } catch (error) {
        if (attempt === 1) {
          throw error;
        }
        await this.page.waitForTimeout(500);
      }
    }
  }

  async login(username: string, password: string) {
    await this.usernameInput.fill(username);
    await this.passwordInput.fill(password);
    await this.submitButton.click();
  }

  async waitForDashboard() {
    await this.page.waitForURL(/\/dashboard\/(?!login|register)/);
  }
}

/**
 * Page Object Model for Register Page
 */
export class RegisterPage {
  readonly page: Page;
  readonly invitationCodeInput: Locator;
  readonly usernameInput: Locator;
  readonly passwordInput: Locator;
  readonly confirmPasswordInput: Locator;
  readonly submitButton: Locator;
  readonly loginLink: Locator;
  readonly errorToast: Locator;
  readonly successMessage: Locator;

  constructor(page: Page) {
    this.page = page;
    this.invitationCodeInput = page.locator('#invitation-code');
    this.usernameInput = page.locator('#username');
    this.passwordInput = page.locator('#password');
    this.confirmPasswordInput = page.locator('#confirm-password');
    this.submitButton = page.locator('button[type="submit"]');
    this.loginLink = page.locator('a[href*="login"]');
    // Radix UI toast (data-state="open") - use first() to handle multiple matches
    this.errorToast = page.locator('[data-state="open"][data-radix-collection-item]').first();
    this.successMessage = page.locator('text=Registration Complete');
  }

  async goto() {
    const url = '/dashboard/register.html';
    for (let attempt = 0; attempt < 2; attempt += 1) {
      try {
        await this.page.goto(url, { waitUntil: 'domcontentloaded' });
        return;
      } catch (error) {
        if (attempt === 1) {
          throw error;
        }
        await this.page.waitForTimeout(500);
      }
    }
  }

  async register(invitationCode: string, username: string, password: string) {
    await this.invitationCodeInput.fill(invitationCode);
    await this.usernameInput.fill(username);
    await this.passwordInput.fill(password);
    await this.confirmPasswordInput.fill(password);
    await this.submitButton.click();
  }

  async waitForSuccess() {
    await this.successMessage.waitFor({ state: 'visible', timeout: 10000 });
  }
}
