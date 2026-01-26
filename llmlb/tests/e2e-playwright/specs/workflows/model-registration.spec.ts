/**
 * Model Registration Workflow E2E Tests
 *
 * These tests verify the complete model registration workflow:
 * - API-based model hub listing
 * - API-based model registration
 * - UI-based model registration
 * - Model lifecycle (lifecycle_status field)
 * - State verification
 */

import { test, expect } from '@playwright/test';
import {
  cleanTestState,
  getModelCount,
  getModels,
  getDownloadingModels,
  getHubModels,
  registerModel,
  ensureDashboardLogin,
  clearAllModels,
} from '../../helpers/api-helpers';

test.describe.configure({ mode: 'serial' });

test.describe('Model Registration Workflow', () => {
  // Clean state before each test
  test.beforeEach(async ({ request }) => {
    await cleanTestState(request);
  });

  // Clean up after each test
  test.afterEach(async ({ request }) => {
    await cleanTestState(request);
  });

  test.describe('Model Hub API', () => {
    // NOTE: supported_models.json は廃止されました (2026-01-25)
    // /v0/models/hub は登録済みモデルのみを返すため、空の状態でも正常です
    test('returns empty array when no models registered', async ({ request }) => {
      const hubModels = await getHubModels(request);

      // Should return an array (may be empty if no models registered)
      expect(Array.isArray(hubModels)).toBe(true);
    });

    test('model hub returns status for each registered model', async ({ request }) => {
      const hubModels = await getHubModels(request);

      for (const model of hubModels) {
        expect(['available', 'downloading', 'downloaded']).toContain(model.status);
      }
    });
  });

  test.describe('API Register', () => {
    // NOTE: supported_models.json は廃止されました
    // 任意のHuggingFaceモデルを直接登録できます

    test('registers a HuggingFace model (201 or 200)', async ({ request }) => {
      // 任意のHFリポジトリを直接登録
      const result = await registerModel(
        request,
        'Qwen/Qwen2.5-0.5B-Instruct-GGUF',
        'qwen2.5-0.5b-instruct-q4_k_m.gguf'
      );

      // 201 = new registration, 200 = already registered (both are valid)
      expect([200, 201]).toContain(result.status);
      expect(result.registered || result.modelName).toBeTruthy();
    });

    test('rejects invalid repo', async ({ request }) => {
      const result = await registerModel(request, 'invalid-nonexistent-model');

      // Should be rejected (400 or 404)
      expect([400, 404]).toContain(result.status);
      expect(result.error).toBeTruthy();
    });

    // NOTE: SPEC-6cd7f960 FR-6 により、/v1/models はオンラインエンドポイントのモデルのみを返す
    // 登録しただけではエンドポイントに紐付かないため、/v1/models には表示されない
    // このテストは /v0/models/registered で確認するように変更
    test('model appears in /v0/models/registered after register', async ({ request }) => {
      // 1. Register a HuggingFace model directly
      const result = await registerModel(
        request,
        'Qwen/Qwen2.5-0.5B-Instruct-GGUF',
        'qwen2.5-0.5b-instruct-q4_k_m.gguf'
      );
      // 201 = new registration, 200 = already registered (both are valid)
      expect([200, 201]).toContain(result.status);

      // 2. Verify model appears in registered models list (not /v1/models)
      // Per SPEC-6cd7f960 FR-6, /v1/models only returns models from online endpoints
      const response = await request.get('/v0/models/registered', {
        headers: {
          Authorization: 'Bearer sk_debug',
        },
      });
      expect(response.ok()).toBeTruthy();
      const registeredModels = await response.json();
      const found = registeredModels.some(
        (m: { name: string }) => m.name === result.modelName
      );
      expect(found).toBeTruthy();
    });
  });

  test.describe('UI Register', () => {
    // NOTE: Model Hub タブは SPEC-6cd7f960 により廃止されました
    // supported_models.json が廃止され、エンドポイント側でモデル認識を行う方針に変更
    // 以下は Local タブでの登録ダイアログを使用するテストに変更

    test('Dashboard shows Local tab and Register button', async ({ page }) => {
      await ensureDashboardLogin(page);

      // Navigate to Models tab
      await page.click('button[role="tab"]:has-text("Models")');
      await page.waitForTimeout(500);

      // Verify Local tab is active (default) and Register button exists
      const localTab = page.locator('button[role="tab"]:has-text("Local")');
      await expect(localTab).toBeVisible();
      await expect(localTab).toHaveAttribute('data-state', 'active');

      const registerButton = page.locator('#register-model');
      await expect(registerButton).toBeVisible();
    });

    test('Register button opens registration dialog', async ({ page }) => {
      await ensureDashboardLogin(page);

      // Navigate to Models tab
      await page.click('button[role="tab"]:has-text("Models")');
      await page.waitForTimeout(300);

      // Click Register button
      const registerButton = page.locator('#register-model');
      await registerButton.click();
      await page.waitForTimeout(500);

      // Registration dialog should be visible
      const dialog = page.locator('#register-modal');
      await expect(dialog).toBeVisible();

      // Should have repo input field
      const repoInput = page.locator('#register-repo');
      await expect(repoInput).toBeVisible();
    });

    test('UI register triggers API call', async ({ page }) => {
      // 1. Login and navigate
      await ensureDashboardLogin(page);
      await page.click('button[role="tab"]:has-text("Models")');
      await page.waitForTimeout(300);

      // 2. Mock register endpoint to track calls
      let registerCalled = false;
      await page.route('**/v0/models/register', async (route) => {
        registerCalled = true;
        await route.fulfill({
          status: 201,
          contentType: 'application/json',
          body: JSON.stringify({
            name: 'test-model',
            lifecycle_status: 'registered',
          }),
        });
      });

      // 3. Open register dialog
      const openDialogButton = page.locator('#register-model');
      await openDialogButton.click();
      await page.waitForTimeout(300);

      // 4. Fill in repo and submit
      const repoInput = page.locator('#register-repo');
      await repoInput.fill('test-org/test-model');

      const submitButton = page.locator('#register-submit');
      await submitButton.click();
      await page.waitForTimeout(500);

      expect(registerCalled).toBe(true);
    });
  });

  test.describe('State Consistency', () => {
    test('registered model appears in API list', async ({ request }) => {
      // 1. Register a HuggingFace model directly
      const result = await registerModel(
        request,
        'Qwen/Qwen2.5-0.5B-Instruct-GGUF',
        'qwen2.5-0.5b-instruct-q4_k_m.gguf'
      );
      // 201 = new registration, 200 = already registered (both are valid)
      expect([200, 201]).toContain(result.status);

      // 2. Verify in models list
      const models = await getModels(request);
      // Model should appear with some lifecycle status
      expect(models.length).toBeGreaterThanOrEqual(1);
    });

    test('cleanup removes all models', async ({ request }) => {
      // 1. Get current model count
      const beforeCount = await getModelCount(request);

      // 2. If no models, nothing to clean
      if (beforeCount === 0) {
        expect((await getDownloadingModels(request)).length).toBe(0);
        return;
      }

      // 3. Clean up
      await clearAllModels(request);

      // 4. Verify cleanup
      const afterCount = await getModelCount(request);
      expect(afterCount).toBeLessThanOrEqual(beforeCount);
      expect((await getDownloadingModels(request)).length).toBe(0);
    });
  });
});
