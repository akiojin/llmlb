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

function mockChatCompletionsStream(page: Page, assistantText: string) {
  return page.route('**/v1/chat/completions', async (route) => {
    await route.fulfill({
      status: 200,
      headers: { 'Content-Type': 'text/event-stream' },
      body:
        `data: ${JSON.stringify({ choices: [{ delta: { content: assistantText } }] })}\n\n` +
        'data: [DONE]\n\n',
    });
  });
}

test.describe('Playground Chat @playground', () => {
  test.beforeEach(async ({ page }) => {
    await mockRegisteredModels(page);
    await mockChatCompletionsStream(page, 'OK');
    await page.goto('/playground');
  });

  test('PC-01: Model select dropdown is visible', async ({ page }) => {
    await expect(page.getByTestId('playground-model-select')).toBeVisible();
  });

  test('PC-02: Model select has options', async ({ page }) => {
    const trigger = page.getByTestId('playground-model-select');
    await trigger.click();
    const option = page.getByRole('option', { name: /openai:gpt-4o/ });
    await expect(option).toBeVisible();
    await expect(option).toContainText('対応');
  });

  test('PC-03: Chat input field is visible', async ({ page }) => {
    await expect(page.getByTestId('playground-chat-input')).toBeVisible();
  });

  test('PC-04: Send button is visible', async ({ page }) => {
    await expect(page.getByTestId('playground-send')).toBeVisible();
  });

  test('PC-05: Can type in chat input', async ({ page }) => {
    const testMessage = 'Hello, this is a test message';
    const input = page.getByTestId('playground-chat-input');
    await input.fill(testMessage);
    await expect(input).toHaveValue(testMessage);
  });

  test('PC-06: Shift+Enter adds newline', async ({ page }) => {
    const input = page.getByTestId('playground-chat-input');
    await input.fill('Line 1');
    await input.press('Shift+Enter');
    await input.type('Line 2');
    await expect(input).toHaveValue(/Line 1\nLine 2/);
  });

  test('PC-07: Stop button is initially hidden', async ({ page }) => {
    await expect(page.getByTestId('playground-stop')).toHaveCount(0);
  });

  test('PC-08: Send button is disabled with empty input', async ({ page }) => {
    await expect(page.getByTestId('playground-send')).toBeDisabled();
  });

  test('PC-09: Welcome message is displayed initially', async ({ page }) => {
    await expect(page.getByText('Start a conversation')).toBeVisible();
  });

  test('PC-10: Enter key submits message', async ({ page }) => {
    const input = page.getByTestId('playground-chat-input');
    await input.fill('Test message');

    const reqPromise = page.waitForRequest('**/v1/chat/completions');
    await input.press('Enter');
    await reqPromise;

    await expect(input).toHaveValue('');
  });
});
