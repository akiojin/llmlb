import { test, expect, type APIRequestContext } from '@playwright/test'
import { deleteApiKey, deleteEndpointsByName, listApiKeys } from '../../helpers/api-helpers'
import { DashboardPage } from '../../pages/dashboard.page'
import {
  startMockOpenAIEndpointServer,
  type MockOpenAIEndpointServer,
} from '../../helpers/mock-openai-endpoint'

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768'
async function deleteApiKeysByName(
  request: APIRequestContext,
  name: string
): Promise<number> {
  const apiKeys = await listApiKeys(request)
  const targets = apiKeys.filter((k: { name?: string }) => k?.name === name)

  for (const key of targets) {
    if (!key?.id) continue
    await deleteApiKey(request, key.id)
  }

  return targets.length
}

test.describe.configure({ mode: 'serial' })

test.describe('API Key Create + OpenAI API Calls @api-keys', () => {
  let mock: MockOpenAIEndpointServer
  let endpointName = ''
  let apiKeyName = ''

  test.beforeAll(async () => {
    mock = await startMockOpenAIEndpointServer()
  })

  test.afterAll(async () => {
    await mock.close()
  })

  test.afterEach(async ({ request }) => {
    if (endpointName) {
      await deleteEndpointsByName(request, endpointName)
      endpointName = ''
    }
    if (apiKeyName) {
      await deleteApiKeysByName(request, apiKeyName)
      apiKeyName = ''
    }
  })

  test('create API key via UI, then call /v1/models and /v1/chat/completions with that key', async ({
    page,
    request,
  }) => {
    test.setTimeout(90_000)
    // Keep the viewport wide enough for desktop-only controls like the header "API Keys" button.
    await page.setViewportSize({ width: 1280, height: 900 })

    endpointName = `e2e-mock-openai-${Date.now()}-${Math.random().toString(16).slice(2)}`
    apiKeyName = `e2e-api-key-${Date.now()}-${Math.random().toString(16).slice(2)}`

    const dashboard = new DashboardPage(page)
    await dashboard.goto()

    // 1) Register an endpoint with deterministic models/behavior.
    await page.getByRole('button', { name: 'Add Endpoint' }).click()
    await page.fill('#endpoint-name', endpointName)
    await page.fill('#endpoint-url', mock.baseUrl)
    await page.getByRole('button', { name: 'Create Endpoint' }).click()

    const row = page.getByRole('row').filter({ hasText: endpointName })
    await expect(row).toBeVisible({ timeout: 20000 })

    // Deterministically bring it online.
    await row.locator('button[title="Test Connection"]').click()
    await expect(row.getByText('Online')).toBeVisible({ timeout: 20000 })

    // Sync models so /v1/models and routing work.
    await row.locator('button[title="Sync Models"]').click()

    // Confirm at least one model is visible in the endpoint detail modal.
    await row.locator('button[title="Details"]').click()
    const detailsDialog = page.getByRole('dialog').filter({ hasText: endpointName })
    await expect(detailsDialog).toBeVisible({ timeout: 20000 })
    await expect(detailsDialog.getByText(mock.models[0])).toBeVisible({ timeout: 20000 })
    // The endpoint detail modal can exceed the viewport height, making close buttons unclickable.
    // Escape is the most reliable close action for E2E.
    await page.keyboard.press('Escape')
    await expect(detailsDialog).toBeHidden({ timeout: 20000 })

    // 2) Create an API key via the dashboard UI.
    await page.click('#api-keys-button')
    const apiKeysModal = page.locator('#api-keys-modal')
    await expect(apiKeysModal).toBeVisible({ timeout: 10000 })

    await apiKeysModal.locator('#create-api-key').click()
    const createDialog = page.getByRole('dialog').filter({ hasText: 'Create API Key' })
    await expect(createDialog).toBeVisible({ timeout: 10000 })

    await createDialog.locator('#api-key-name').fill(apiKeyName)
    await createDialog.getByRole('button', { name: 'Create', exact: true }).click()

    // Wait for onSuccess (name is cleared only on success).
    await expect(createDialog.locator('#api-key-name')).toHaveValue('', { timeout: 20000 })

    // Create dialog auto-closes on success
    const createdAlert = apiKeysModal.getByText('API Key Created Successfully').locator('..')
    await expect(createdAlert).toBeVisible({ timeout: 10000 })

    // Reveal and read the plaintext key (only shown at creation time).
    await createdAlert.locator('button:not(#copy-api-key)').first().click()
    const apiKeyCode = createdAlert.locator('code')
    await expect(apiKeyCode).toContainText('sk_', { timeout: 10000 })
    const apiKey = (await apiKeyCode.textContent())?.trim() || ''
    expect(apiKey).toMatch(/^sk_/)
    expect(apiKey).not.toContain('•')

    // 3) Use the created key to call real APIs.
    const modelsResp = await request.get(`${API_BASE}/v1/models`, {
      headers: { Authorization: `Bearer ${apiKey}` },
    })
    expect(modelsResp.ok()).toBeTruthy()
    const modelsJson = await modelsResp.json()
    const modelIds = (modelsJson?.data || []).map((m: { id: string }) => m.id)
    expect(modelIds).toContain(mock.models[0])

    const userText = 'Hello from API key E2E'
    const chatResp = await request.post(`${API_BASE}/v1/chat/completions`, {
      data: {
        model: mock.models[0],
        messages: [{ role: 'user', content: userText }],
        stream: false,
      },
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${apiKey}`,
      },
    })
    expect(chatResp.ok()).toBeTruthy()
    const chatJson = await chatResp.json()
    const assistant = chatJson?.choices?.[0]?.message?.content || ''
    expect(assistant).toContain('MOCK_OK')
    expect(assistant).toContain(`model=${mock.models[0]}`)
    expect(assistant).toContain(`user=${userText}`)
  })
})
