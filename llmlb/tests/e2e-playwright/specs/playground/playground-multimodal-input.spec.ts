import { test, expect } from '@playwright/test';
import { mockChatCompletionsStream, mockOpenAIModels } from '../../helpers/mock-helpers';

// FilePayload was removed from @playwright/test in newer versions
interface FilePayload {
  name: string;
  mimeType: string;
  buffer: Buffer;
}

const transparentPngBase64 =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+lk1kAAAAASUVORK5CYII=';

function pngFile(name = 'test.png'): FilePayload {
  return {
    name,
    mimeType: 'image/png',
    buffer: Buffer.from(transparentPngBase64, 'base64'),
  };
}

function wavFile(name = 'test.wav'): FilePayload {
  // Minimal WAV header (PCM, mono, 8kHz, 16-bit, 0 data bytes)
  const header = Buffer.from([
    0x52, 0x49, 0x46, 0x46, // RIFF
    0x24, 0x00, 0x00, 0x00, // chunk size (36 + data)
    0x57, 0x41, 0x56, 0x45, // WAVE
    0x66, 0x6d, 0x74, 0x20, // fmt
    0x10, 0x00, 0x00, 0x00, // subchunk1 size
    0x01, 0x00, // PCM
    0x01, 0x00, // mono
    0x40, 0x1f, 0x00, 0x00, // 8000 Hz
    0x80, 0x3e, 0x00, 0x00, // byte rate
    0x02, 0x00, // block align
    0x10, 0x00, // bits per sample
    0x64, 0x61, 0x74, 0x61, // data
    0x00, 0x00, 0x00, 0x00, // data size
  ]);
  return { name, mimeType: 'audio/wav', buffer: header };
}

test.describe('Playground Multimodal Input @playground', () => {
  test.beforeEach(async ({ page }) => {
    // Set up route mocks BEFORE navigation
    await mockOpenAIModels(page);
    await mockChatCompletionsStream(page, 'OK');

    // Navigate to playground
    await page.goto('/playground', { waitUntil: 'networkidle' });

    // Wait for model to be selected by checking the combobox has proper aria-expanded or value
    // Give React time to fetch and process models
    await page.waitForTimeout(1000);

    // Additional safety check: verify models were loaded
    const modelSelector = page.locator('[role="combobox"]');
    await modelSelector.waitFor({ state: 'visible', timeout: 5000 });
  });

  test('MMI-01: 画像を添付して送信できる（テキスト空でも可）', async ({ page }) => {
    const imageInput = page.getByTestId('playground-image-input');
    await imageInput.setInputFiles(pngFile());

    await expect(page.getByTestId('playground-attachment-image')).toBeVisible();
    await expect(page.getByTestId('playground-send')).toBeEnabled();

    const reqPromise = page.waitForRequest('**/v1/chat/completions');
    await page.getByTestId('playground-send').click();
    const req = await reqPromise;

    const body = req.postDataJSON() as any;
    expect(body.model).toBe('openai:gpt-4o');
    expect(body.messages?.at(-1)?.role).toBe('user');
    expect(Array.isArray(body.messages?.at(-1)?.content)).toBe(true);
    expect(
      body.messages.at(-1).content.some((p: any) => p.type === 'image_url' && p.image_url?.url)
    ).toBe(true);
  });

  test('MMI-02: 音声を添付して送信できる（テキスト空でも可）', async ({ page }) => {
    const audioInput = page.getByTestId('playground-audio-input');
    await audioInput.setInputFiles(wavFile());

    await expect(page.getByTestId('playground-attachment-audio')).toBeVisible();
    await expect(page.getByTestId('playground-send')).toBeEnabled();

    const reqPromise = page.waitForRequest('**/v1/chat/completions');
    await page.getByTestId('playground-send').click();
    const req = await reqPromise;

    const body = req.postDataJSON() as any;
    expect(body.messages?.at(-1)?.role).toBe('user');
    expect(Array.isArray(body.messages?.at(-1)?.content)).toBe(true);
    expect(
      body.messages
        .at(-1)
        .content.some((p: any) => p.type === 'input_audio' && p.input_audio?.data)
    ).toBe(true);
  });

  test('MMI-03: 画像/音声を貼り付けで添付できる', async ({ page }) => {
    const input = page.getByTestId('playground-chat-input');
    await input.click();

    // Paste image
    await page.evaluate(async ({ bytes }) => {
      const el = document.querySelector('[data-testid="playground-chat-input"]');
      if (!el) throw new Error('chat input not found');
      const dt = new DataTransfer();
      const file = new File([new Uint8Array(bytes)], 'pasted.png', { type: 'image/png' });
      dt.items.add(file);
      el.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }));
    }, { bytes: Array.from(Buffer.from(transparentPngBase64, 'base64')) });

    await expect(page.getByTestId('playground-attachment-image')).toBeVisible();
    await page.getByTestId('playground-attachment-remove').click();
    await expect(page.getByTestId('playground-attachment-preview')).toBeHidden();

    // Paste audio
    const wav = wavFile().buffer;
    const wavBytes: number[] = Array.from(wav);
    await page.evaluate(async ({ bytes }: { bytes: number[] }) => {
      const el = document.querySelector('[data-testid="playground-chat-input"]');
      if (!el) throw new Error('chat input not found');
      const dt = new DataTransfer();
      const file = new File([new Uint8Array(bytes)], 'pasted.wav', { type: 'audio/wav' });
      dt.items.add(file);
      el.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }));
    }, { bytes: wavBytes });

    await expect(page.getByTestId('playground-attachment-audio')).toBeVisible();
  });
});
