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

test.describe('Playground Sidebar @playground', () => {
  test.beforeEach(async ({ page }) => {
    await mockRegisteredModels(page);
    await page.goto('/playground');
  });

  test('PS-01: Sidebar is visible on load', async ({ page }) => {
    await expect(page.getByTestId('playground-sidebar')).toBeVisible();
    await expect(page.getByText('LLM Router')).toBeVisible();
  });

  test('PS-02: New Chat button is visible', async ({ page }) => {
    await expect(page.getByRole('button', { name: 'New Chat' })).toBeVisible();
  });

  test('PS-03: Session list container exists', async ({ page }) => {
    await expect(page.getByTestId('playground-session-list')).toBeVisible();
  });

  test('PS-04: New chat creates a session entry', async ({ page }) => {
    const sessionList = page.getByTestId('playground-session-list');
    const initialCount = await sessionList.locator('[data-testid^="playground-session-"]').count();

    await page.getByRole('button', { name: 'New Chat' }).click();

    await expect(sessionList.locator('[data-testid^="playground-session-"]')).toHaveCount(
      initialCount + 1
    );
  });
});
