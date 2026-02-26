import { test, expect } from '@playwright/test'
import {
  ensureDashboardLogin,
  createApiKeyWithPermissions,
  deleteApiKey,
  deleteEndpointsByName,
  type CreatedApiKey,
} from '../../helpers/api-helpers'
import {
  startMockOpenAIEndpointServer,
  type MockOpenAIEndpointServer,
} from '../../helpers/mock-openai-endpoint'

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768'
const AUTH_HEADER = { Authorization: 'Bearer sk_debug', 'Content-Type': 'application/json' }

test.describe('API Key Lifecycle @api', () => {
  test.describe.configure({ mode: 'serial' })

  let mock: MockOpenAIEndpointServer
  const endpointName = `e2e-akl-${Date.now()}`
  const createdKeyIds: string[] = []

  test.beforeAll(async ({ request }) => {
    mock = await startMockOpenAIEndpointServer({ models: ['mock-model-a'] })

    // Register mock endpoint via API and bring online
    const createResp = await request.post(`${API_BASE}/api/endpoints`, {
      headers: AUTH_HEADER,
      data: { name: endpointName, base_url: mock.baseUrl },
    })
    expect(createResp.ok()).toBeTruthy()
    const endpoint = (await createResp.json()) as { id: string }

    const testResp = await request.post(`${API_BASE}/api/endpoints/${endpoint.id}/test`, {
      headers: { Authorization: 'Bearer sk_debug' },
    })
    expect(testResp.ok()).toBeTruthy()

    const syncResp = await request.post(`${API_BASE}/api/endpoints/${endpoint.id}/sync`, {
      headers: { Authorization: 'Bearer sk_debug' },
    })
    expect(syncResp.ok()).toBeTruthy()

    // Wait for model to be available
    await expect
      .poll(
        async () => {
          const resp = await request.get(`${API_BASE}/v1/models`, {
            headers: { Authorization: 'Bearer sk_debug' },
          })
          if (!resp.ok()) return [] as string[]
          const json = (await resp.json()) as { data?: Array<{ id: string }> }
          return (json.data ?? []).map((m) => m.id)
        },
        { timeout: 30000 }
      )
      .toContain('mock-model-a')
  })

  test.afterAll(async ({ request }) => {
    // Clean up all API keys created during tests
    for (const keyId of createdKeyIds) {
      await deleteApiKey(request, keyId).catch(() => {})
    }
    await deleteEndpointsByName(request, endpointName)
    await mock.close()
  })

  test('AKL-01: create API key via dashboard -> API access succeeds', async ({
    page,
    request,
  }) => {
    test.setTimeout(90_000)
    await page.setViewportSize({ width: 1280, height: 900 })

    await ensureDashboardLogin(page)

    // Open API Keys modal
    await page.click('#api-keys-button')
    const apiKeysModal = page.locator('#api-keys-modal')
    await expect(apiKeysModal).toBeVisible({ timeout: 10000 })

    // Create API key
    await apiKeysModal.locator('#create-api-key').click()
    const createDialog = page.getByRole('dialog').filter({ hasText: 'Create API Key' })
    await expect(createDialog).toBeVisible({ timeout: 10000 })

    const keyName = `e2e-akl01-${Date.now()}`
    await createDialog.locator('#api-key-name').fill(keyName)

    const createApiKeyResponse = page.waitForResponse(
      (response) =>
        response.url().includes('/api/me/api-keys') &&
        response.request().method() === 'POST' &&
        response.status() >= 200 &&
        response.status() < 300
    )

    await createDialog.getByRole('button', { name: 'Create', exact: true }).click()
    // Wait for onSuccess (dialog auto-closes on success).
    await expect(createDialog).toBeHidden({ timeout: 20000 })
    const createResp = await createApiKeyResponse
    const createRespBody = (await createResp.json()) as { id?: string; key?: string }
    const apiKey = createRespBody.key?.trim() || ''
    expect(apiKey).toMatch(/^sk_/)
    if (createRespBody.id) {
      createdKeyIds.push(createRespBody.id)
    }

    // Read the plaintext key (create dialog auto-closes on success)
    const createdAlert = apiKeysModal.getByText('API Key Created Successfully').locator('..')
    await expect(createdAlert).toBeVisible({ timeout: 10000 })
    await createdAlert.locator('#copy-api-key').click()
    await expect(page.getByText('Failed to copy')).toHaveCount(0)
    await createdAlert.locator('code').waitFor({ state: 'visible', timeout: 10000 })

    // Use the key to access /v1/models
    const modelsResp = await request.get(`${API_BASE}/v1/models`, {
      headers: { Authorization: `Bearer ${apiKey}` },
    })
    expect(modelsResp.ok()).toBeTruthy()
    const modelsJson = await modelsResp.json()
    const modelIds = (modelsJson?.data || []).map((m: { id: string }) => m.id)
    expect(modelIds).toContain('mock-model-a')

    // Use the key to call /v1/chat/completions
    const chatResp = await request.post(`${API_BASE}/v1/chat/completions`, {
      data: {
        model: 'mock-model-a',
        messages: [{ role: 'user', content: 'AKL-01 test' }],
        stream: false,
      },
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${apiKey}`,
      },
    })
    expect(chatResp.ok()).toBeTruthy()
    const chatJson = await chatResp.json()
    expect(chatJson?.choices?.[0]?.message?.content).toContain('MOCK_OK')

  })

  test('AKL-02: expired API key -> 401', async ({ request }) => {
    // Create a key with past expiration
    const pastDate = new Date(Date.now() - 86400000).toISOString() // 1 day ago
    const expiredKey = await createApiKeyWithPermissions(
      request,
      `e2e-akl02-expired-${Date.now()}`,
      ['openai.inference', 'openai.models.read'],
      pastDate
    )
    expect(expiredKey.key).toBeTruthy()
    createdKeyIds.push(expiredKey.id)

    // Attempt API access with expired key
    const resp = await request.get(`${API_BASE}/v1/models`, {
      headers: { Authorization: `Bearer ${expiredKey.key}` },
    })
    expect(resp.status()).toBe(401)
  })

  test('AKL-03: delete API key -> access denied', async ({ request }) => {
    // Create a valid key
    const validKey = await createApiKeyWithPermissions(
      request,
      `e2e-akl03-delete-${Date.now()}`,
      ['openai.inference', 'openai.models.read']
    )
    expect(validKey.key).toBeTruthy()

    // Verify access works
    const successResp = await request.get(`${API_BASE}/v1/models`, {
      headers: { Authorization: `Bearer ${validKey.key}` },
    })
    expect(successResp.ok()).toBeTruthy()

    // Delete the key
    const deleted = await deleteApiKey(request, validKey.id)
    expect(deleted).toBeTruthy()

    // Verify access is now denied
    const deniedResp = await request.get(`${API_BASE}/v1/models`, {
      headers: { Authorization: `Bearer ${validKey.key}` },
    })
    expect(deniedResp.status()).toBe(401)
  })

  test('AKL-04: Expired badge display in dashboard', async ({ page, request }) => {
    test.setTimeout(60_000)
    await page.setViewportSize({ width: 1280, height: 900 })

    // Create an expired key
    const pastDate = new Date(Date.now() - 86400000).toISOString()
    const expiredKey = await createApiKeyWithPermissions(
      request,
      `e2e-akl04-expired-${Date.now()}`,
      ['openai.models.read'],
      pastDate
    )
    expect(expiredKey.id).toBeTruthy()
    createdKeyIds.push(expiredKey.id)

    // Create a valid (non-expired) key
    const futureDate = new Date(Date.now() + 86400000 * 30).toISOString() // 30 days from now
    const validKey = await createApiKeyWithPermissions(
      request,
      `e2e-akl04-valid-${Date.now()}`,
      ['openai.models.read'],
      futureDate
    )
    expect(validKey.id).toBeTruthy()
    createdKeyIds.push(validKey.id)

    // Navigate to dashboard and open API Keys modal
    await ensureDashboardLogin(page)
    await page.click('#api-keys-button')
    const apiKeysModal = page.locator('#api-keys-modal')
    await expect(apiKeysModal).toBeVisible({ timeout: 10000 })

    // Verify that an "Expired" badge is visible for the expired key
    await expect(apiKeysModal.locator('.bg-destructive:has-text("Expired")').first()).toBeVisible({ timeout: 10000 })
  })
})
