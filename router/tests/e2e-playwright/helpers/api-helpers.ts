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
 * Values from /v1/models: pending, caching, registered, error, downloading, ready, cached
 */
export type LifecycleStatus =
  | 'pending'
  | 'caching'
  | 'registered'
  | 'error'
  | 'downloading'
  | 'ready'
  | 'cached';

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

// ============================================================================
// Model Hub API Helpers
// ============================================================================

/**
 * Supported model from Model Hub
 */
export interface HubModel {
  id: string;
  name: string;
  description: string;
  repo: string;
  recommended_filename: string;
  size_bytes: number;
  required_memory_bytes: number;
  tags: string[];
  capabilities: string[];
  quantization?: string;
  parameter_count?: string;
  hf_info?: {
    downloads?: number;
    likes?: number;
  };
  status: 'available' | 'downloading' | 'downloaded';
  lifecycle_status?: LifecycleStatus;
}

/**
 * Get list of supported models from Model Hub
 */
export async function getHubModels(request: APIRequestContext): Promise<HubModel[]> {
  const response = await request.get(`${API_BASE}/v0/models/hub`, {
    headers: AUTH_HEADER,
  });
  if (!response.ok()) {
    return [];
  }
  return response.json();
}

/**
 * Pull a model from Model Hub
 *
 * Response codes:
 * - 200: Pull initiated successfully
 * - 400: Validation error (invalid model_id)
 * - 404: Model not found in supported models
 */
export async function pullModel(
  request: APIRequestContext,
  modelId: string
): Promise<{
  modelId?: string;
  status: number;
  error?: string;
}> {
  const response = await request.post(`${API_BASE}/v0/models/pull`, {
    headers: { ...AUTH_HEADER, 'Content-Type': 'application/json' },
    data: { model_id: modelId },
  });

  const status = response.status();

  try {
    const body = await response.json();

    if (status === 200) {
      return {
        modelId: body.model_id,
        status,
      };
    }

    return {
      error: body.error || body.message,
      status,
    };
  } catch {
    return { error: await response.text(), status };
  }
}

/**
 * @deprecated Use pullModel() instead - /v0/models/register has been removed
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
  // Return 404 since endpoint is removed
  return { error: 'Endpoint removed - use pullModel() instead', status: 404 };
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
 * Pull a model via the Dashboard UI (Model Hub tab)
 */
export async function pullModelViaUI(
  page: Page,
  modelId: string
): Promise<void> {
  // Navigate to Model Hub tab
  await page.click('button[role="tab"]:has-text("Model Hub")');
  await page.waitForTimeout(500);

  // Find the model card and click Pull button
  const modelCard = page.locator(`[data-model-id="${modelId}"]`);
  const pullButton = modelCard.locator('button:has-text("Pull")');
  await pullButton.click();

  // Wait for the action to complete
  await page.waitForTimeout(500);
}

/**
 * @deprecated Use pullModelViaUI() instead - Register modal has been removed
 */
export async function registerModelViaUI(
  page: Page,
  repo: string,
  filename?: string
): Promise<void> {
  throw new Error('Register modal has been removed - use pullModelViaUI() instead');
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
