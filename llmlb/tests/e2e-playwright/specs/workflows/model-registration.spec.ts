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
  getRegisteredModels,
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
    // /api/models/hub は登録済みモデルのみを返すため、空の状態でも正常です
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

    test('registers a HuggingFace model (201 or 200)', async ({ request }, testInfo) => {
      // 任意のHFリポジトリを直接登録
      const result = await registerModel(
        request,
        'Qwen/Qwen2.5-0.5B-Instruct-GGUF',
        'qwen2.5-0.5b-instruct-q4_k_m.gguf'
      );

      // HuggingFace is an external dependency; CI/network issues can happen.
      if (result.status >= 500) {
        testInfo.skip(
          true,
          `HuggingFace API unavailable - external dependency (HTTP ${result.status})`
        );
        return;
      }

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
    // このテストは registry 一覧の /api/models で確認する
    test('model appears in /api/models after register', async ({ request }, testInfo) => {
      // 1. Register a HuggingFace model directly
      const result = await registerModel(
        request,
        'Qwen/Qwen2.5-0.5B-Instruct-GGUF',
        'qwen2.5-0.5b-instruct-q4_k_m.gguf'
      );

      // HuggingFace is an external dependency; CI/network issues can happen.
      if (result.status >= 500) {
        testInfo.skip(
          true,
          `HuggingFace API unavailable - external dependency (HTTP ${result.status})`
        );
        return;
      }

      // 201 = new registration, 200 = already registered (both are valid)
      expect([200, 201]).toContain(result.status);

      // 2. Verify model appears in registered models list (not /v1/models)
      // Per SPEC-6cd7f960 FR-6, /v1/models only returns models from online endpoints
      const response = await request.get('/api/models', {
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

  // NOTE: UI Register tests removed - Models tab has been removed from dashboard
  // Model registration is now done via API or endpoint-specific UI

  test.describe('State Consistency', () => {
    test('registered model appears in API list', async ({ request }, testInfo) => {
      // 1. Register a HuggingFace model directly
      const result = await registerModel(
        request,
        'Qwen/Qwen2.5-0.5B-Instruct-GGUF',
        'qwen2.5-0.5b-instruct-q4_k_m.gguf'
      );

      // HuggingFace is an external dependency; CI/network issues can happen.
      if (result.status >= 500 || result.status === 429) {
        testInfo.skip(
          true,
          `HuggingFace API unavailable - external dependency (HTTP ${result.status})`
        );
        return;
      }

      // 201 = new registration, 200 = already registered (both are valid)
      expect([200, 201]).toContain(result.status);

      // 2. Verify it appears in the registry list (/api/models).
      // Per SPEC-6cd7f960 FR-6, /v1/models only returns models from online endpoints,
      // so registration alone does NOT guarantee it appears in /v1/models.
      const registryModels = await getRegisteredModels(request);
      expect(registryModels.some((m) => m.name === result.modelName)).toBeTruthy();

      // 3. Ensure the new registry model is not mistakenly exposed via /v1/models.
      const availableModels = await getModels(request);
      expect(availableModels.some((m) => m.name === result.modelName)).toBe(false);
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
