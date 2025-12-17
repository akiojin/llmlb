import { test, expect } from '@playwright/test';
import { PlaygroundPage } from '../../pages/playground.page';

test.describe('Playground Settings @playground', () => {
  let playground: PlaygroundPage;

  test.beforeEach(async ({ page }) => {
    playground = new PlaygroundPage(page);
    await playground.goto();
    await playground.dismissError();
  });

  test('PST-01: Settings button opens modal', async () => {
    await playground.openSettings();
    await expect(playground.settingsModal).toBeVisible();
  });

  test('PST-02: Settings modal can be closed', async ({ page }) => {
    await playground.openSettings();
    // Close the modal using Escape key (Radix Dialog closes on Escape)
    await page.keyboard.press('Escape');
    await expect(playground.settingsModal).toBeHidden();
  });

  test('PST-03: Provider Local button is clickable', async () => {
    // Provider filter buttons not implemented in current Playground
    await playground.openSettings();
    await playground.setProvider('local');

    const isActive = await playground.providerLocal.evaluate(
      (el) => el.classList.contains('provider-btn--active')
    );
    expect(isActive).toBe(true);
  });

  test('PST-04: Provider Cloud button is clickable', async () => {
    // Provider filter buttons not implemented in current Playground
    await playground.openSettings();
    await playground.setProvider('cloud');

    const isActive = await playground.providerCloud.evaluate(
      (el) => el.classList.contains('provider-btn--active')
    );
    expect(isActive).toBe(true);
  });

  test('PST-05: Provider All button is clickable', async () => {
    // Provider filter buttons not implemented in current Playground
    await playground.openSettings();
    await playground.setProvider('all');

    const isActive = await playground.providerAll.evaluate(
      (el) => el.classList.contains('provider-btn--active')
    );
    expect(isActive).toBe(true);
  });

  test('PST-06: API Key input field exists', async () => {
    await playground.openSettings();
    await expect(playground.apiKeyInput).toBeVisible();
  });

  test('PST-07: API Key can be entered', async () => {
    await playground.openSettings();
    const testKey = 'sk-test-key-12345';
    await playground.apiKeyInput.fill(testKey);
    await expect(playground.apiKeyInput).toHaveValue(testKey);
  });

  test('PST-08: Stream toggle exists', async () => {
    await playground.openSettings();
    await expect(playground.streamToggle).toBeVisible();
  });

  test('PST-09: Stream toggle is clickable', async ({ page }) => {
    await playground.openSettings();
    // Stream toggle is a shadcn Switch component, check if it can be toggled
    const toggle = playground.streamToggle;
    const initialState = await toggle.getAttribute('data-state');
    await toggle.click();
    await page.waitForTimeout(100);
    const newState = await toggle.getAttribute('data-state');
    // Should have changed state
    expect(newState).not.toBe(initialState);
  });

  test('PST-10: System prompt field exists', async () => {
    await playground.openSettings();
    await expect(playground.systemPrompt).toBeVisible();
  });

  test('PST-11: System prompt can be edited', async () => {
    await playground.openSettings();
    const testPrompt = 'You are a helpful assistant for testing.';
    await playground.setSystemPrompt(testPrompt);
    await expect(playground.systemPrompt).toHaveValue(testPrompt);
  });

  test('PST-12: Clear Playground button exists', async () => {
    // Reset chat button not implemented in current Playground
    await playground.openSettings();
    await expect(playground.resetChat).toBeVisible();
  });

  test('PST-13: Copy cURL button exists', async () => {
    // cURL button is in the header, not in settings
    await expect(playground.copyCurl).toBeVisible();
  });

  test('PST-14: Copy cURL button is clickable', async ({ page }) => {
    // cURL button is in the header, clicks to open dialog
    await playground.copyCurl.click();

    // Wait for cURL dialog to open
    await page.waitForTimeout(300);

    // Should show cURL command dialog
    const curlDialog = page.locator('[role="dialog"]:has-text("cURL")');
    await expect(curlDialog).toBeVisible();
  });
});
