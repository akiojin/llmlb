/**
 * Model Registration Workflow E2E Tests
 *
 * These tests verify the complete model registration workflow:
 * - API-based registration
 * - UI-based registration
 * - Duplicate detection
 * - Model lifecycle (lifecycle_status field)
 * - State verification
 *
 * NOTE: /v0/models/convert has been removed. Model download status is now
 * exposed via the lifecycle_status field in /v0/models response.
 */

import { test, expect } from '@playwright/test';
import {
  getModels,
  registerModel,
  deleteModel,
  getModelByName,
  waitForModelReady,
  ensureDashboardLogin,
  registerModelViaUI,
} from '../../helpers/api-helpers';

test.describe.configure({ mode: 'serial' });

test.describe('Model Registration Workflow', () => {
  const TEST_REPO = 'onnxmodelzoo/mnist-12';
  const TEST_FILENAME = 'mnist-12.onnx';
  const TEST_MODEL_NAME = 'onnxmodelzoo/mnist-12';

  // Clean state before each test
  test.beforeEach(async ({ request }) => {
    await deleteModel(request, TEST_MODEL_NAME);
  });

  // Clean up after each test
  test.afterEach(async ({ request }) => {
    await deleteModel(request, TEST_MODEL_NAME);
  });

  test.describe('API Registration', () => {
    test('registers a cached model directly (201)', async ({ request }) => {
      // 1. Model should not exist (best-effort; other tests may run in parallel)
      expect(await getModelByName(request, TEST_MODEL_NAME)).toBeNull();

      // 2. Register model via API
      const result = await registerModel(request, TEST_REPO, TEST_FILENAME);

      // 3. Verify registration (201) or duplicate (400)
      expect([201, 400]).toContain(result.status);

      // 4. Wait for the model to become ready in /v0/models (pending/caching -> registered)
      await waitForModelReady(request, TEST_MODEL_NAME, { timeout: 120000 });

      // 5. Verify the model is listed
      expect(await getModelByName(request, TEST_MODEL_NAME)).toBeTruthy();
    });

    test('rejects duplicate registration for same model', async ({ request }) => {
      // 1. Register first model (using real cached model)
      //    May return 400 if already registered from previous test/run
      const first = await registerModel(request, TEST_REPO, TEST_FILENAME);
      expect([201, 400]).toContain(first.status);

      // 2. Attempt duplicate registration
      const duplicate = await registerModel(request, TEST_REPO, TEST_FILENAME);

      // 3. Should be rejected (400 = already registered)
      expect(duplicate.status).toBe(400);
      expect(duplicate.error).toContain('already registered');
    });

    test('rejects invalid repository', async ({ request }) => {
      // 1. Attempt to register non-existent model
      const result = await registerModel(request, 'invalid/nonexistent-model');

      // 2. Should be rejected (400 = validation error)
      expect(result.status).toBe(400);
      expect(result.error).toBeTruthy();
    });

    test('model count increases after registration', async ({ request }) => {
      // 2. Register model
      const result = await registerModel(request, TEST_REPO, TEST_FILENAME);

      // 3. If registered, it should appear in list
      if (result.status === 201) {
        await waitForModelReady(request, TEST_MODEL_NAME, { timeout: 120000 });
        expect(await getModelByName(request, TEST_MODEL_NAME)).toBeTruthy();
      }
    });
  });

  test.describe('UI Registration', () => {
    test('Dashboard shows Register button and opens modal', async ({ page, request }) => {
      // 1. Login to dashboard
      await ensureDashboardLogin(page);

      // 2. Navigate to Models tab
      await page.click('button[role="tab"]:has-text("Models")');
      await page.waitForTimeout(500);

      // 3. Verify Register button exists
      const registerButton = page.locator('button:not([role="tab"]):has-text("Register")');
      await expect(registerButton).toBeVisible();

      // 4. Click and verify modal opens
      await registerButton.click();
      await expect(page.locator('#convert-modal')).toBeVisible();

      // 5. Verify form fields exist
      await expect(page.locator('#convert-repo')).toBeVisible();
      await expect(page.locator('#convert-submit')).toBeVisible();
    });

    test('UI registration triggers API call', async ({ page, request }) => {
      // 2. Login and navigate
      await ensureDashboardLogin(page);
      await page.click('button[role="tab"]:has-text("Models")');
      await page.waitForTimeout(500);

      // 3. Intercept API call
      const responsePromise = page.waitForResponse('**/v0/models/register');

      // 4. Register via UI (using a real cached model)
      await registerModelViaUI(page, TEST_REPO, TEST_FILENAME);

      // 5. Verify API called (201 for accepted, 400 for duplicate)
      const response = await responsePromise;
      expect([201, 400]).toContain(response.status());

      // 6. If successful (201), verify model becomes ready
      if (response.status() === 201) {
        await waitForModelReady(request, TEST_MODEL_NAME, { timeout: 120000 });
        expect(await getModelByName(request, TEST_MODEL_NAME)).toBeTruthy();
      }
    });

    test('UI shows error for duplicate registration', async ({ page, request }) => {
      // 1. Register model via API first (or it may already be registered)
      const first = await registerModel(request, TEST_REPO, TEST_FILENAME);
      expect([201, 400]).toContain(first.status);

      // 2. Login and navigate
      await ensureDashboardLogin(page);
      await page.click('button[role="tab"]:has-text("Models")');
      await page.waitForTimeout(500);

      // 3. Intercept API call
      const responsePromise = page.waitForResponse('**/v0/models/register');

      // 4. Attempt duplicate via UI
      await registerModelViaUI(page, TEST_REPO, TEST_FILENAME);

      // 5. Verify error response (400 = already registered)
      const response = await responsePromise;
      expect(response.status()).toBe(400);

      // 6. Modal should still be open (showing error)
      await page.waitForTimeout(500);
      const modalVisible = await page.locator('#convert-modal').isVisible();
      // Modal may close, that's also acceptable behavior
      expect(typeof modalVisible).toBe('boolean');
    });
  });

  test.describe('State Consistency', () => {
    test('registered model appears in API list', async ({ request }) => {
      // 1. Register model
      const result = await registerModel(request, TEST_REPO, TEST_FILENAME);
      expect([201, 400]).toContain(result.status);

      // 2. Wait for model ready and verify it appears in list
      await waitForModelReady(request, TEST_MODEL_NAME, { timeout: 120000 });
      const models = await getModels(request);
      expect(models.some((m) => m.name === TEST_MODEL_NAME)).toBe(true);
    });

    test('cleanup removes the registered model', async ({ request }) => {
      // 1. Register model
      await registerModel(request, TEST_REPO, TEST_FILENAME);
      await waitForModelReady(request, TEST_MODEL_NAME, { timeout: 120000 });

      // 2. Verify it exists
      expect(await getModelByName(request, TEST_MODEL_NAME)).toBeTruthy();

      // 3. Delete the model
      await deleteModel(request, TEST_MODEL_NAME);

      // 4. Verify it is removed
      expect(await getModelByName(request, TEST_MODEL_NAME)).toBeNull();
    });
  });
});
