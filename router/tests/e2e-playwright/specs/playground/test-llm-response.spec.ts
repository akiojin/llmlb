import { test, expect } from '@playwright/test';
import { mockChatCompletionsStream, mockOpenAIModels } from '../../helpers/mock-helpers';
import { PlaygroundPage } from '../../pages/playground.page';

test('LLM response in Playground', async ({ page }) => {
  await mockOpenAIModels(page);
  await mockChatCompletionsStream(page, 'Hello from mock');

  // Log all network requests
  page.on('request', req => {
    if (req.url().includes('chat/completions')) {
      console.log('REQUEST:', req.method(), req.url());
      console.log('REQUEST BODY:', req.postData());
    }
  });
  page.on('response', async resp => {
    if (resp.url().includes('chat/completions')) {
      console.log('RESPONSE:', resp.status(), resp.url());
      try {
        const body = await resp.text();
        console.log('RESPONSE BODY:', body.substring(0, 500));
      } catch (e) {
        console.log('RESPONSE ERROR:', e);
      }
    }
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

  // Skip test if no models are available (requires registered node with models)
  const firstOptionText = count > 0 ? await options.first().textContent() : '';
  if (count === 0 || firstOptionText?.includes('No models available')) {
    console.log('Skipping test: No models available (requires registered node)');
    test.skip();
    return;
  }

  expect(count).toBeGreaterThan(0);

  // Find and select qwen2.5-0.5b-instruct (verified working model)
  let selectedModel = '';
  for (let i = 0; i < count; i++) {
    const text = await options.nth(i).textContent();
    if (text?.includes('qwen2.5-0.5b-instruct') && !text?.includes('fp16')) {
      console.log(`Selecting: ${text}`);
      await options.nth(i).click();
      selectedModel = text;
      break;
    }
  }

  // Fallback to first enabled model if qwen not found
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
    console.log('Skipping test: No enabled models found');
    test.skip();
    return;
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
  await page.screenshot({ path: 'test-results/after-send-click.png' });

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
