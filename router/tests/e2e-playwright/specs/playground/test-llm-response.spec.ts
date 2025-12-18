import { test, expect } from '@playwright/test';
import { PlaygroundPage } from '../../pages/playground.page';
import { getNodes, registerModel, waitForModelReady } from '../../helpers/api-helpers';

// Tiny text-generation model (converted to ONNX by router) for real chat inference.
const TEST_REPO = 'sshleifer/tiny-gpt2';
const TEST_FILENAME: string | undefined = undefined;
const TEST_MODEL_NAME = 'sshleifer/tiny-gpt2';

test('LLM response in Playground', async ({ page, request }) => {
  const register = await registerModel(request, TEST_REPO, TEST_FILENAME);
  expect([201, 400]).toContain(register.status);
  await waitForModelReady(request, register.modelName || TEST_MODEL_NAME, { timeout: 120000 });

  // Requires at least one online node for inference.
  const nodes = await getNodes(request);
  expect(nodes.some((n) => n.status === 'online')).toBe(true);

  // Ensure UI uses the debug API key for both model list and chat requests.
  await page.addInitScript(() => {
    localStorage.setItem('llm-router-api-key', 'sk_debug');
    localStorage.setItem('playground_api_key', 'sk_debug');
  });

  const playground = new PlaygroundPage(page);
  await playground.goto();
  await page.waitForLoadState('networkidle');
  await playground.dismissError();

  // Click model dropdown
  console.log('Clicking model select...');
  await playground.modelSelect.click();
  await page.waitForTimeout(500);

  // Get options and find qwen model (known to work)
  const options = page.locator('[role="option"]');
  const count = await options.count();
  console.log(`Found ${count} model options`);

  expect(count).toBeGreaterThan(0);

  // Find and select the known-good test model
  let selectedModel = '';
  for (let i = 0; i < count; i++) {
    const text = await options.nth(i).textContent();
    if (text?.includes(TEST_MODEL_NAME)) {
      console.log(`Selecting: ${text}`);
      await options.nth(i).click();
      selectedModel = text;
      break;
    }
  }

  // Fallback to first enabled model if test model not found
  if (!selectedModel) {
    // Find first enabled option
    for (let i = 0; i < count; i++) {
      const isDisabled = await options.nth(i).getAttribute('data-disabled');
      if (isDisabled === null) {
        const text = await options.nth(i).textContent();
        console.log(`Fallback selecting: ${text}`);
        await options.nth(i).click();
        selectedModel = text ?? '';
        break;
      }
    }
  }

  // If still no model selected, skip the test
  if (!selectedModel) {
    throw new Error('No enabled models found in model selector');
  }

  await page.waitForTimeout(500);

  // Type message (use type() instead of fill() for React controlled input)
  const chatInput = page.locator('#chat-input');
  await chatInput.click();
  await chatInput.pressSequentially('Hello', { delay: 50 });
  await page.waitForTimeout(200);

  // Verify input has text
  const inputValue = await chatInput.inputValue();
  console.log('Input value:', inputValue);

  // Click send button
  const sendButton = page.locator('#send-button');
  console.log('Send button disabled:', await sendButton.isDisabled());

  // Check if selectedModel is shown
  const modelText = await playground.modelSelect.textContent();
  console.log('Model select text:', modelText);

  await expect(sendButton).toBeEnabled({ timeout: 5000 });
  console.log('Clicking send button...');
  await sendButton.click();

  // Wait a bit for the message to be processed
  await page.waitForTimeout(1000);

  // Check for error notification
  const errorNotification = page.locator('[data-testid="notification-error"]');
  if (await errorNotification.isVisible({ timeout: 1000 }).catch(() => false)) {
    const errorText = await errorNotification.textContent();
    console.log('Error notification:', errorText);
  }

  // Take screenshot for debugging
  await page.screenshot({ path: test.info().outputPath('after-send-click.png') });

  // Wait for user message to appear (uses CSS class)
  const userMessage = page.locator('.message--user');
  await expect(userMessage.first()).toBeVisible({ timeout: 10000 });
  console.log('User message visible');

  // Wait for assistant response (streaming takes time)
  const assistantMessage = page.locator('.message--assistant');
  await expect(assistantMessage.first()).toBeVisible({ timeout: 60000 });

  // Wait for streaming to complete - check for non-empty message text
  const messageText = assistantMessage.first().locator('p');
  await expect(messageText).toHaveText(/.+/, { timeout: 60000 });

  const responseText = await messageText.textContent();
  console.log('LLM Response:', responseText);

  expect(responseText).toBeTruthy();
  expect(responseText!.length).toBeGreaterThan(0);
});
