import { test, expect, type Page } from '@playwright/test';

function mockRegisteredModels(page: Page) {
  return page.route('**/api/models/registered', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify([
        {
          name: 'openai:gpt-4o',
          state: 'ready',
          capabilities: { input_image: 'supported', input_audio: 'supported' },
        },
      ]),
    });
  });
}

test.describe('Playground Settings @playground', () => {
  test.beforeEach(async ({ page }) => {
    await mockRegisteredModels(page);
    await page.goto('/playground');
  });

  test('PST-01: Settings button opens modal', async ({ page }) => {
    await page.getByTestId('playground-open-settings').click();
    await expect(page.getByRole('dialog')).toBeVisible();
    await expect(page.getByRole('dialog').getByText('Settings')).toBeVisible();
  });

  test('PST-02: Settings modal can be closed', async ({ page }) => {
    await page.getByTestId('playground-open-settings').click();
    await expect(page.getByRole('dialog')).toBeVisible();
    await page.getByRole('button', { name: 'Done' }).click();
    await expect(page.getByRole('dialog')).toBeHidden();
  });

  test('PST-03: Stream toggle exists and is clickable', async ({ page }) => {
    await page.getByTestId('playground-open-settings').click();
    const dialog = page.getByRole('dialog');
    const toggle = dialog.getByRole('switch');
    await expect(toggle).toBeVisible();

    const initial = await toggle.getAttribute('aria-checked');
    await toggle.click();
    const next = await toggle.getAttribute('aria-checked');
    expect(next).not.toBe(initial);
  });

  test('PST-04: System prompt field exists and can be edited', async ({ page }) => {
    await page.getByTestId('playground-open-settings').click();
    const prompt = page.getByPlaceholder('You are a helpful assistant...');
    await expect(prompt).toBeVisible();
    await prompt.fill('You are a helpful assistant for testing.');
    await expect(prompt).toHaveValue('You are a helpful assistant for testing.');
  });

  test('PST-05: Temperature slider exists', async ({ page }) => {
    await page.getByTestId('playground-open-settings').click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.locator('input[type="range"]')).toBeVisible();
  });

  test('PST-06: Max tokens input exists and can be edited', async ({ page }) => {
    await page.getByTestId('playground-open-settings').click();
    const dialog = page.getByRole('dialog');
    const maxTokens = dialog.locator('input[type="number"]').first();
    await expect(maxTokens).toBeVisible();
    await maxTokens.fill('2048');
    await expect(maxTokens).toHaveValue('2048');
  });
});
