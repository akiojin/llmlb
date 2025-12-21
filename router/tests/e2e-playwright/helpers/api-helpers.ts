/**
 * API Helper Functions for E2E Tests
 *
 * Provides utilities for:
 * - State verification (models, convert tasks)
 * - Test setup/cleanup
 * - Workflow helpers (register, wait for completion)
 */

import type { APIRequestContext, Page } from '@playwright/test';

const API_BASE = 'http://localhost:8080';
const AUTH_HEADER = { Authorization: 'Bearer sk_debug' };

// ============================================================================
// Model API Helpers
// ============================================================================

/**
 * Lifecycle status of a registered model
 * Values from /v1/models: pending, caching, registered, error
 */
export type LifecycleStatus = 'pending' | 'caching' | 'registered' | 'error';

/**
 * Download progress information
 */
export interface DownloadProgress {
  percent: number;
  downloaded_bytes?: number;
  total_bytes?: number;
  error?: string;
}

export interface RegisteredModel {
  name: string;
  repo?: string;
  source?: string;
  path?: string;
  size_gb?: number;
  required_memory_gb?: number;
  lifecycle_status: LifecycleStatus;
  download_progress?: DownloadProgress;
}

// Legacy alias for backward compatibility
export type ModelInfo = RegisteredModel;

/**
 * Get list of registered models
 * Uses /v1/models (OpenAI-compatible endpoint with lifecycle extensions)
 */
export async function getModels(request: APIRequestContext): Promise<RegisteredModel[]> {
  const response = await request.get(`${API_BASE}/v1/models`, {
    headers: AUTH_HEADER,
  });
  if (!response.ok()) {
    return [];
  }
  const data = await response.json();
  // /v1/models returns { object: "list", data: [...] } format
  // Map 'id' to 'name' for backward compatibility with E2E tests
  const models = data.data || [];
  return models.map((m: { id: string; lifecycle_status?: string; download_progress?: DownloadProgress; path?: string }) => ({
    name: m.id,
    lifecycle_status: m.lifecycle_status || 'registered',
    download_progress: m.download_progress,
    path: m.path,
  }));
}

/**
 * Get count of registered models
 */
export async function getModelCount(request: APIRequestContext): Promise<number> {
  const models = await getModels(request);
  return models.length;
}

/**
 * Delete a model by name
 */
export async function deleteModel(
  request: APIRequestContext,
  modelName: string
): Promise<boolean> {
  const response = await request.delete(`${API_BASE}/v0/models/${encodeURIComponent(modelName)}`, {
    headers: AUTH_HEADER,
  });
  return response.status() === 204 || response.status() === 200;
}

/**
 * Clear all registered models
 */
export async function clearAllModels(request: APIRequestContext): Promise<void> {
  const models = await getModels(request);
  for (const model of models) {
    await deleteModel(request, model.name);
  }
}

/**
 * Register a model via API
 *
 * Response codes:
 * - 201: Model registered directly (cached/already downloaded)
 * - 202: ConvertTask created for download/conversion
 * - 400: Validation error (model already registered, file not found, etc.)
 */
export async function registerModel(
  request: APIRequestContext,
  repo: string,
  filename?: string
): Promise<{
  taskId?: string;
  modelName?: string;
  error?: string;
  status: number;
  registered?: boolean;
}> {
  const response = await request.post(`${API_BASE}/v0/models/register`, {
    headers: { ...AUTH_HEADER, 'Content-Type': 'application/json' },
    data: { repo, filename },
  });

  const status = response.status();

  try {
    const body = await response.json();

    // 201: Direct registration (model was cached)
    if (status === 201) {
      return {
        modelName: body.name,
        status,
        registered: true,
      };
    }

    // 202: ConvertTask created
    if (status === 202) {
      return {
        taskId: body.task_id,
        status,
        registered: false,
      };
    }

    // Error response
    return {
      error: body.error || body.message,
      status,
    };
  } catch {
    return { error: await response.text(), status };
  }
}

// ============================================================================
// Model Status Helpers (replaced Convert Task API)
// ============================================================================

/**
 * Get models that are currently downloading
 */
export async function getDownloadingModels(request: APIRequestContext): Promise<RegisteredModel[]> {
  const models = await getModels(request);
  return models.filter((m) => m.lifecycle_status === 'downloading' || m.lifecycle_status === 'pending');
}

/**
 * Get models with errors
 */
export async function getErrorModels(request: APIRequestContext): Promise<RegisteredModel[]> {
  const models = await getModels(request);
  return models.filter((m) => m.lifecycle_status === 'error');
}

/**
 * Get a specific model by name
 */
export async function getModelByName(
  request: APIRequestContext,
  modelName: string
): Promise<RegisteredModel | null> {
  const models = await getModels(request);
  return models.find((m) => m.name === modelName) || null;
}

/**
 * Wait for a model to reach ready or cached status
 */
export async function waitForModelReady(
  request: APIRequestContext,
  modelName: string,
  options: { timeout?: number; pollInterval?: number } = {}
): Promise<RegisteredModel> {
  const { timeout = 300000, pollInterval = 2000 } = options;
  const startTime = Date.now();

  while (Date.now() - startTime < timeout) {
    const model = await getModelByName(request, modelName);
    if (!model) {
      throw new Error(`Model ${modelName} not found`);
    }

    if (model.lifecycle_status === 'ready' || model.lifecycle_status === 'cached') {
      return model;
    }

    if (model.lifecycle_status === 'error') {
      throw new Error(`Model ${modelName} failed: ${model.download_progress?.error || 'Unknown error'}`);
    }

    await new Promise((resolve) => setTimeout(resolve, pollInterval));
  }

  throw new Error(`Model ${modelName} did not become ready within ${timeout}ms`);
}

// ============================================================================
// Deprecated Convert Task API Helpers (for backward compatibility)
// NOTE: /v0/models/convert endpoint has been removed
// ============================================================================

/**
 * @deprecated Use getDownloadingModels() instead
 */
export async function getConvertTasks(request: APIRequestContext): Promise<RegisteredModel[]> {
  return getDownloadingModels(request);
}

/**
 * @deprecated Use clearAllModels() instead
 */
export async function clearAllConvertTasks(request: APIRequestContext): Promise<void> {
  // Clearing models will also cancel any pending downloads
  await clearAllModels(request);
}

// ============================================================================
// Node API Helpers
// ============================================================================

export interface NodeInfo {
  id: string;
  machine_name: string;
  status: string;
  loaded_models: string[];
}

/**
 * Get list of nodes
 */
export async function getNodes(request: APIRequestContext): Promise<NodeInfo[]> {
  const response = await request.get(`${API_BASE}/v0/nodes`, {
    headers: AUTH_HEADER,
  });
  const data = await response.json();
  // Handle both array and { nodes: [] } response formats
  return Array.isArray(data) ? data : data.nodes || [];
}

/**
 * Get count of online nodes
 */
export async function getOnlineNodeCount(request: APIRequestContext): Promise<number> {
  const nodes = await getNodes(request);
  return nodes.filter((n) => n.status === 'online').length;
}

// ============================================================================
// UI Workflow Helpers
// ============================================================================

/**
 * Register a model via the Dashboard UI
 */
export async function registerModelViaUI(
  page: Page,
  repo: string,
  filename?: string
): Promise<void> {
  // Click Register button
  await page.click('button:not([role="tab"]):has-text("Register")');

  // Wait for modal
  await page.waitForSelector('#convert-modal', { state: 'visible' });

  // Fill form
  await page.fill('#convert-repo', repo);
  if (filename) {
    await page.fill('#convert-filename', filename);
  }

  // Submit
  await page.click('#convert-submit');

  // Wait for modal to close or response
  await page.waitForSelector('#convert-modal', { state: 'hidden', timeout: 10000 }).catch(() => {
    // Modal may stay open with error, that's ok
  });
}

/**
 * Navigate to Dashboard and login if needed
 */
export async function ensureDashboardLogin(page: Page): Promise<void> {
  await page.goto(`${API_BASE}/dashboard`);
  await page.waitForLoadState('networkidle');

  // Check if login form is present
  const loginForm = page.locator('form').filter({ hasText: 'Sign in' });
  if (await loginForm.isVisible({ timeout: 2000 }).catch(() => false)) {
    await page.fill('input[type="text"], input[name="username"], #username', 'admin');
    await page.fill('input[type="password"], input[name="password"], #password', 'test');
    await page.click('button[type="submit"]');

    // Wait for either dashboard content or URL change
    await Promise.race([
      page.waitForURL('**/dashboard/**', { timeout: 10000 }),
      page.waitForSelector('[data-stat="total-nodes"]', { timeout: 10000 }),
      page.waitForSelector('button[role="tab"]', { timeout: 10000 }),
    ]).catch(() => {
      // Ignore timeout, continue if we're on dashboard
    });

    // Verify we're on dashboard
    await page.waitForLoadState('networkidle');
  }
}

// ============================================================================
// Test Setup/Cleanup
// ============================================================================

/**
 * Clean state before test
 */
export async function cleanTestState(request: APIRequestContext): Promise<void> {
  await clearAllModels(request);
  // NOTE: clearAllConvertTasks is now a no-op since tasks are integrated into models
}

/**
 * Verify clean state
 */
export async function verifyCleanState(request: APIRequestContext): Promise<{
  models: number;
  /** @deprecated tasks count is now always derived from models */
  tasks: number;
}> {
  const models = await getModelCount(request);
  const downloadingModels = await getDownloadingModels(request);
  return { models, tasks: downloadingModels.length };
}
