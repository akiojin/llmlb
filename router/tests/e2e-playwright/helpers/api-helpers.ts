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

export interface ModelInfo {
  name: string;
  repo?: string;
  status?: string;
  path?: string;
}

export interface ModelsResponse {
  data: ModelInfo[];
  object: string;
}

/**
 * Get list of registered models
 */
export async function getModels(request: APIRequestContext): Promise<ModelInfo[]> {
  const response = await request.get(`${API_BASE}/v0/models`, {
    headers: AUTH_HEADER,
  });
  const data = (await response.json()) as ModelsResponse;
  return data.data || [];
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
// Convert Task API Helpers
// ============================================================================

export interface ConvertTask {
  id: string;
  repo: string;
  filename: string;
  status: 'queued' | 'in_progress' | 'completed' | 'failed';
  progress: number;
  error?: string;
  path?: string;
  created_at: string;
  updated_at: string;
}

/**
 * Get list of convert tasks
 */
export async function getConvertTasks(request: APIRequestContext): Promise<ConvertTask[]> {
  const response = await request.get(`${API_BASE}/v0/models/convert`, {
    headers: AUTH_HEADER,
  });
  return (await response.json()) as ConvertTask[];
}

/**
 * Get a specific convert task by ID
 */
export async function getConvertTask(
  request: APIRequestContext,
  taskId: string
): Promise<ConvertTask | null> {
  const response = await request.get(`${API_BASE}/v0/models/convert/${taskId}`, {
    headers: AUTH_HEADER,
  });
  if (response.status() === 404) {
    return null;
  }
  return (await response.json()) as ConvertTask;
}

/**
 * Delete a convert task
 */
export async function deleteConvertTask(
  request: APIRequestContext,
  taskId: string
): Promise<boolean> {
  const response = await request.delete(`${API_BASE}/v0/models/convert/${taskId}`, {
    headers: AUTH_HEADER,
  });
  return response.status() === 204;
}

/**
 * Clear all convert tasks
 */
export async function clearAllConvertTasks(request: APIRequestContext): Promise<void> {
  const tasks = await getConvertTasks(request);
  for (const task of tasks) {
    await deleteConvertTask(request, task.id);
  }
}

/**
 * Get the latest convert task
 */
export async function getLatestTask(request: APIRequestContext): Promise<ConvertTask | null> {
  const tasks = await getConvertTasks(request);
  if (tasks.length === 0) return null;
  // Tasks are sorted by updated_at desc
  return tasks[0];
}

/**
 * Wait for a convert task to complete
 */
export async function waitForConvertTaskComplete(
  request: APIRequestContext,
  taskId: string,
  options: { timeout?: number; pollInterval?: number } = {}
): Promise<ConvertTask> {
  const { timeout = 300000, pollInterval = 2000 } = options;
  const startTime = Date.now();

  while (Date.now() - startTime < timeout) {
    const task = await getConvertTask(request, taskId);
    if (!task) {
      throw new Error(`Task ${taskId} not found`);
    }

    if (task.status === 'completed') {
      return task;
    }

    if (task.status === 'failed') {
      throw new Error(`Task ${taskId} failed: ${task.error}`);
    }

    await new Promise((resolve) => setTimeout(resolve, pollInterval));
  }

  throw new Error(`Task ${taskId} did not complete within ${timeout}ms`);
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
  await clearAllConvertTasks(request);
}

/**
 * Verify clean state
 */
export async function verifyCleanState(request: APIRequestContext): Promise<{
  models: number;
  tasks: number;
}> {
  const models = await getModelCount(request);
  const tasks = (await getConvertTasks(request)).length;
  return { models, tasks };
}
