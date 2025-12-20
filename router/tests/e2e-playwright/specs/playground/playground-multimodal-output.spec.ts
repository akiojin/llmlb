import { test, expect, type Page } from '@playwright/test';

function mockOpenAIModels(page: Page) {
  const created = Math.floor(Date.now() / 1000);
  return page.route('**/v1/models', async (route) => {
    // Mock the /v1/models endpoint with OpenAI-compatible format
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        object: 'list',
        data: [
          {
            id: 'openai:gpt-4o',
            object: 'model',
            created,
            owned_by: 'openai',
            capabilities: {
              chat_completion: true,
              completion: false,
              embeddings: false,
              fine_tune: false,
              inference: true,
              text_to_speech: false,
              speech_to_text: true,
              image_generation: true,
            },
            lifecycle_status: 'registered',
            download_progress: null,
            ready: true,
          },
        ],
      }),
    });
  });
}

function mockChatCompletionsStream(page: Page, assistantText: string) {
  return page.route('**/v1/chat/completions', async (route) => {
    await route.fulfill({
      status: 200,
      headers: { 'Content-Type': 'text/event-stream' },
      body: `data: ${JSON.stringify({ choices: [{ delta: { content: assistantText } }] })}\n\n` +
        'data: [DONE]\n\n',
    });
  });
}

const transparentPngBase64 =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+lk1kAAAAASUVORK5CYII=';

function wavBase64(): string {
  const header = Buffer.from([
    0x52, 0x49, 0x46, 0x46, 0x24, 0x00, 0x00, 0x00, 0x57, 0x41, 0x56, 0x45, 0x66, 0x6d,
    0x74, 0x20, 0x10, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x40, 0x1f, 0x00, 0x00,
    0x80, 0x3e, 0x00, 0x00, 0x02, 0x00, 0x10, 0x00, 0x64, 0x61, 0x74, 0x61, 0x00, 0x00,
    0x00, 0x00,
  ]);
  return header.toString('base64');
}

test.describe('Playground Multimodal Output @playground', () => {
  async function setupPlayground(page: Page, assistantResponse: string) {
    // Set up route mocks BEFORE navigation
    await mockOpenAIModels(page);
    await mockChatCompletionsStream(page, assistantResponse);

    // Navigate to playground
    await page.goto('/playground', { waitUntil: 'networkidle' });

    // Wait for chat input to be visible and ready
    const chatInput = page.getByTestId('playground-chat-input');
    await chatInput.waitFor({ state: 'visible', timeout: 10000 });

    // Wait a bit more for React to fully render
    await page.waitForTimeout(1000);
  }

  test('MMO-01: アシスタント本文内の画像URL/データURLをプレビュー表示できる', async ({ page }) => {
    const dataUrl = `data:image/png;base64,${transparentPngBase64}`;
    await setupPlayground(page, `image: ${dataUrl}`);

    await page.getByTestId('playground-chat-input').fill('show image');
    await page.getByTestId('playground-send').click();

    const img = page.getByTestId('playground-assistant-image');
    await expect(img).toBeVisible();
    await expect(img).toHaveAttribute('src', dataUrl);
  });

  test('MMO-02: アシスタント本文内の音声URL/データURLを再生UI表示できる', async ({ page }) => {
    const dataUrl = `data:audio/wav;base64,${wavBase64()}`;
    await setupPlayground(page, `audio: ${dataUrl}`);

    await page.getByTestId('playground-chat-input').fill('show audio');
    await page.getByTestId('playground-send').click();

    const audio = page.getByTestId('playground-assistant-audio');
    await expect(audio).toBeVisible();
    await expect(audio).toHaveAttribute('src', dataUrl);
  });
});
