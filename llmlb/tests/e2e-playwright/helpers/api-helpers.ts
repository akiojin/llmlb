/**
 * API Helper Functions for E2E Tests
 *
 * Provides utilities for:
 * - State verification (models, lifecycle status)
 * - Test setup/cleanup
 * - Workflow helpers (register, wait for completion)
 */

import { request as playwrightRequest, type APIRequestContext, type Page } from '@playwright/test';

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768';
const AUTH_HEADER = { Authorization: 'Bearer sk_debug' };

async function getDashboardJwt(): Promise<string> {
  const credentials = [
    { username: 'admin', password: 'test' },
    { username: 'admin', password: 'password123' },
  ];

  const authContext = await playwrightRequest.newContext();
  try {
    for (const cred of credentials) {
      const response = await authContext.post(`${API_BASE}/api/auth/login`, {
        headers: { 'Content-Type': 'application/json' },
        data: cred,
      });
      if (!response.ok()) continue;
      const body = (await response.json()) as { token?: string };
      if (body.token) return body.token;
    }
  } finally {
    await authContext.dispose();
  }

  throw new Error('Failed to acquire dashboard JWT for API key management');
}

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
  filename?: string;
  size_bytes?: number;
  required_memory_bytes?: number;
  lifecycle_status: LifecycleStatus;
  download_progress?: DownloadProgress;
  ready?: boolean;
}

// Legacy alias for backward compatibility
export type ModelInfo = RegisteredModel;

/**
 * Get list of models from /v1/models
 * NOTE: Per SPEC-6cd7f960 FR-6, /v1/models only returns models from online endpoints
 * Use getRegisteredModels() to get all registered models regardless of endpoint status
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
  return models.map((m: {
    id: string;
    lifecycle_status?: string;
    download_progress?: DownloadProgress;
    repo?: string;
    filename?: string;
    size_bytes?: number;
    required_memory_bytes?: number;
    ready?: boolean;
  }) => ({
    name: m.id,
    lifecycle_status: m.lifecycle_status || 'registered',
    download_progress: m.download_progress,
    repo: m.repo,
    filename: m.filename,
    size_bytes: m.size_bytes,
    required_memory_bytes: m.required_memory_bytes,
    ready: m.ready,
  }));
}

/**
 * Get list of all registered models from /api/models
 *
 * This is the model registry (metadata in DB), not the list of online endpoint models.
 * It includes models that are not attached to any online endpoint.
 */
export async function getRegisteredModels(request: APIRequestContext): Promise<RegisteredModel[]> {
  const response = await request.get(`${API_BASE}/api/models`, {
    headers: AUTH_HEADER,
  });
  if (!response.ok()) {
    return [];
  }
  const models = await response.json();
  return models.map((m: {
    name: string;
    repo?: string;
    filename?: string;
    size?: number;
    required_memory?: number;
  }) => ({
    name: m.name,
    // Registry entries are already "registered" in the DB.
    lifecycle_status: 'registered',
    repo: m.repo,
    filename: m.filename,
    size_bytes: m.size,
    required_memory_bytes: m.required_memory,
    ready: false,
  }));
}

/**
 * Get count of registered models
 */
export async function getModelCount(request: APIRequestContext): Promise<number> {
  const models = await getRegisteredModels(request);
  return models.length;
}

/**
 * Delete a model by name
 */
export async function deleteModel(
  request: APIRequestContext,
  modelName: string
): Promise<boolean> {
  const response = await request.delete(`${API_BASE}/api/models/${encodeURIComponent(modelName)}`, {
    headers: AUTH_HEADER,
  });
  return response.status() === 204 || response.status() === 200;
}

/**
 * Clear all registered models
 * Uses /api/models to get ALL registry models (not just those on online endpoints)
 */
export async function clearAllModels(request: APIRequestContext): Promise<void> {
  const models = await getRegisteredModels(request);
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
  const response = await request.get(`${API_BASE}/api/models/hub`, {
    headers: AUTH_HEADER,
  });
  if (!response.ok()) {
    return [];
  }
  return response.json();
}

/**
 * Register a model via API (HF registration flow)
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
  const payload: Record<string, unknown> = { repo };
  if (filename) payload.filename = filename;

  const response = await request.post(`${API_BASE}/api/models/register`, {
    headers: { ...AUTH_HEADER, 'Content-Type': 'application/json' },
    data: payload,
  });
  const status = response.status();

  try {
    const body = await response.json();
    return {
      taskId: body.task_id,
      modelName: body.name || body.model_name,
      registered: status === 201 || status === 200,
      status,
      error: body.error || body.message,
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
  return models.filter((m) => m.lifecycle_status === 'pending');
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

    if (model.lifecycle_status === 'registered' || model.ready) {
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
  machine_name?: string;
  name?: string;
  base_url?: string;
  status: string;
  loaded_models?: string[];
  model_count?: number;
}

export interface EndpointInfo {
  id: string;
  name: string;
  base_url: string;
  status: string;
  endpoint_type?: string;
  model_count?: number;
}

/**
 * Get list of nodes (endpoints)
 *
 * Prefer /api/endpoints here because it supports API key auth (sk_debug) in E2E/CI.
 */
export async function getNodes(request: APIRequestContext): Promise<NodeInfo[]> {
  const response = await request.get(`${API_BASE}/api/endpoints`, {
    headers: AUTH_HEADER,
  });
  if (!response.ok()) {
    return [];
  }
  const data = await response.json();
  const endpoints = Array.isArray(data) ? data : data.endpoints || [];
  // Normalize fields for legacy tests.
  return endpoints.map((e: { id: string; name?: string; base_url?: string; status: string; model_count?: number; loaded_models?: string[] }) => ({
    id: e.id,
    machine_name: e.name,
    name: e.name,
    base_url: e.base_url,
    status: e.status,
    loaded_models: e.loaded_models,
    model_count: e.model_count,
  }));
}

/**
 * Get count of online nodes
 */
export async function getOnlineNodeCount(request: APIRequestContext): Promise<number> {
  const nodes = await getNodes(request);
  return nodes.filter((n) => n.status === 'online').length;
}

/**
 * List endpoints (raw shape) via /api/endpoints
 */
export async function listEndpoints(request: APIRequestContext): Promise<EndpointInfo[]> {
  const response = await request.get(`${API_BASE}/api/endpoints`, {
    headers: AUTH_HEADER,
  });
  if (!response.ok()) {
    return [];
  }
  const data = await response.json();
  const endpoints = Array.isArray(data) ? data : data.endpoints || [];
  return endpoints.map((e: { id: string; name: string; base_url: string; status: string; endpoint_type?: string; model_count?: number }) => ({
    id: e.id,
    name: e.name,
    base_url: e.base_url,
    status: e.status,
    endpoint_type: e.endpoint_type,
    model_count: e.model_count,
  }));
}

/**
 * Delete an endpoint by id.
 */
export async function deleteEndpoint(
  request: APIRequestContext,
  endpointId: string
): Promise<boolean> {
  const response = await request.delete(`${API_BASE}/api/endpoints/${encodeURIComponent(endpointId)}`, {
    headers: AUTH_HEADER,
  });
  return response.status() === 204 || response.status() === 200;
}

/**
 * Best-effort cleanup helper for E2E: delete endpoints matching exact name.
 */
export async function deleteEndpointsByName(
  request: APIRequestContext,
  name: string
): Promise<number> {
  const endpoints = await listEndpoints(request);
  const targets = endpoints.filter((e) => e.name === name);
  for (const ep of targets) {
    await deleteEndpoint(request, ep.id);
  }
  return targets.length;
}

// ============================================================================
// UI Workflow Helpers
// ============================================================================

/**
 * Register a model via UI (Hugging Face registration flow)
 */
export async function registerModelViaUI(
  page: Page,
  repo: string,
  filename?: string
): Promise<void> {
  // Click Register button
  await page.click('#register-model');

  // Wait for modal
  await page.waitForSelector('#register-modal', { state: 'visible' });

  // Select format (required by current UI)
  await page.click('#convert-format');
  await page.click('[data-value="gguf"]');

  // Fill form
  await page.fill('#register-repo', repo);
  if (filename) {
    await page.fill('#register-filename', filename);
  }

  // Submit
  await page.click('#register-submit');

  // Wait for modal to close or response
  await page.waitForSelector('#register-modal', { state: 'hidden', timeout: 10000 }).catch(() => {
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
      page.waitForSelector('[data-stat="total-endpoints"]', { timeout: 10000 }),
      page.waitForSelector('button[role="tab"]', { timeout: 10000 }),
    ]).catch(() => {
      // Ignore timeout, continue if we're on dashboard
    });

    // Verify we're on dashboard
    await page.waitForLoadState('networkidle');
  }
}

// ============================================================================
// User Management Helpers
// ============================================================================

export interface UserInfo {
  id: string;
  username: string;
  role: string;
}

/**
 * List all users
 */
export async function listUsers(request: APIRequestContext): Promise<UserInfo[]> {
  const response = await request.get(`${API_BASE}/api/users`, {
    headers: AUTH_HEADER,
  });
  if (!response.ok()) {
    return [];
  }
  const data = await response.json();
  return Array.isArray(data) ? data : data.users || [];
}

/**
 * Create a new user (password is auto-generated by the server)
 */
export async function createUser(
  request: APIRequestContext,
  username: string,
  _password: string,
  role: 'admin' | 'viewer'
): Promise<UserInfo & { generated_password?: string }> {
  const response = await request.post(`${API_BASE}/api/users`, {
    headers: { ...AUTH_HEADER, 'Content-Type': 'application/json' },
    data: { username, role },
  });
  if (!response.ok()) {
    return { id: '', username: '', role: '' };
  }
  const body = await response.json();
  // API returns { user: {...}, generated_password: "..." }
  if (body.user) {
    return { ...body.user, generated_password: body.generated_password };
  }
  return body;
}

/**
 * Update a user's role
 */
export async function updateUserRole(
  request: APIRequestContext,
  userId: string,
  role: 'admin' | 'viewer'
): Promise<void> {
  await request.put(`${API_BASE}/api/users/${encodeURIComponent(userId)}`, {
    headers: { ...AUTH_HEADER, 'Content-Type': 'application/json' },
    data: { role },
  });
}

/**
 * Delete a user by id
 */
export async function deleteUser(
  request: APIRequestContext,
  userId: string
): Promise<boolean> {
  const response = await request.delete(`${API_BASE}/api/users/${encodeURIComponent(userId)}`, {
    headers: AUTH_HEADER,
  });
  return response.status() === 204 || response.status() === 200;
}

// ============================================================================
// API Key Management Helpers
// ============================================================================

export interface ApiKeyInfo {
  id: string;
  name: string;
  key_prefix: string;
  permissions: string[];
  expires_at: string | null;
}

export interface CreatedApiKey {
  id: string;
  key: string;
}

/**
 * Create an API key for the logged-in user.
 */
export async function createApiKeyWithPermissions(
  request: APIRequestContext,
  name: string,
  permissions: string[],
  expiresAt?: string
): Promise<CreatedApiKey> {
  const token = await getDashboardJwt();
  const payload: Record<string, unknown> = { name };
  if (expiresAt) {
    payload.expires_at = expiresAt;
  }
  if (permissions.length > 0) {
    payload.permissions = permissions;
  }
  const response = await request.post(`${API_BASE}/api/me/api-keys`, {
    headers: {
      Authorization: `Bearer ${token}`,
      'Content-Type': 'application/json',
    },
    data: payload,
  });
  if (!response.ok()) {
    return { id: '', key: '' };
  }
  return response.json();
}

/**
 * Delete an API key by id
 */
export async function deleteApiKey(
  request: APIRequestContext,
  keyId: string
): Promise<boolean> {
  const token = await getDashboardJwt();
  const response = await request.delete(
    `${API_BASE}/api/me/api-keys/${encodeURIComponent(keyId)}`,
    {
      headers: { Authorization: `Bearer ${token}` },
    }
  );
  return response.status() === 204 || response.status() === 200;
}

/**
 * List all API keys
 */
export async function listApiKeys(request: APIRequestContext): Promise<ApiKeyInfo[]> {
  const token = await getDashboardJwt();
  const response = await request.get(`${API_BASE}/api/me/api-keys`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!response.ok()) {
    return [];
  }
  const data = await response.json();
  return Array.isArray(data) ? data : data.api_keys || [];
}

// ============================================================================
// Logs, Metrics & System Helpers
// ============================================================================

export interface LogEntry {
  timestamp: string;
  level: string;
  message: string;
}

/**
 * Get load balancer logs
 */
export async function getLbLogs(request: APIRequestContext): Promise<LogEntry[]> {
  const response = await request.get(`${API_BASE}/api/dashboard/logs/lb`, {
    headers: AUTH_HEADER,
  });
  if (!response.ok()) {
    return [];
  }
  const data = await response.json();
  return Array.isArray(data) ? data : data.logs || [];
}

/**
 * Get logs for a specific endpoint
 */
export async function getEndpointLogs(
  request: APIRequestContext,
  endpointId: string
): Promise<LogEntry[]> {
  const response = await request.get(`${API_BASE}/api/nodes/${encodeURIComponent(endpointId)}/logs`, {
    headers: AUTH_HEADER,
  });
  if (!response.ok()) {
    return [];
  }
  const data = await response.json();
  return Array.isArray(data) ? data : data.logs || [];
}

/**
 * Get Prometheus metrics (raw text format)
 */
export async function getPrometheusMetrics(request: APIRequestContext): Promise<string> {
  const response = await request.get(`${API_BASE}/api/metrics/cloud`, {
    headers: AUTH_HEADER,
  });
  if (!response.ok()) {
    return '';
  }
  return response.text();
}

/**
 * Get system information
 */
export async function getSystemInfo(
  request: APIRequestContext
): Promise<{ version?: string; [key: string]: unknown }> {
  const response = await request.get(`${API_BASE}/api/system`, {
    headers: AUTH_HEADER,
  });
  if (!response.ok()) {
    return {};
  }
  return response.json();
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
