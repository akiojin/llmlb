import { test, expect } from '../../helpers/endpoint.fixture'
import { type APIRequestContext, type Page } from '@playwright/test'
import { ensureDashboardLogin, deleteEndpointsByName, listEndpoints } from '../../helpers/api-helpers'
import { startMockOpenAIEndpointServer } from '../../helpers/mock-openai-endpoint'
import { DashboardSelectors } from '../../helpers/selectors'

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768'
const AUTH_HEADER = { Authorization: 'Bearer sk_debug', 'Content-Type': 'application/json' }

test.describe('Endpoint Edit @dashboard', () => {
  async function openDetails(page: Page, name: string) {
    await ensureDashboardLogin(page)
    await page.getByPlaceholder('Search by name or URL...').fill(name)
    const row = page.getByRole('row').filter({ hasText: name })
    await expect(row).toBeVisible()
    await row.locator('button[title="Details"]').click()
    const modal = page.getByRole('dialog')
    await expect(modal).toBeVisible()
    return modal
  }

  async function details(request: APIRequestContext, id: string) {
    const response = await request.get(`${API_BASE}/api/endpoints/${id}`, { headers: AUTH_HEADER })
    expect(response.ok()).toBeTruthy()
    return response.json()
  }

  async function save(page: Page, id: string) {
    const updated = page.waitForResponse((response) =>
      new URL(response.url()).pathname === `/api/endpoints/${id}` && response.request().method() === 'PUT'
    )
    await page.getByRole('dialog').getByRole('button', { name: 'Save', exact: true }).click()
    expect((await updated).ok()).toBeTruthy()
  }

  test('EE-01: Display Name change is reflected in the endpoint list', async ({ page, request, endpoint }) => {
    const modal = await openDetails(page, endpoint.name)
    const newName = `${endpoint.name}-renamed`
    await modal.getByLabel('Display Name').fill(newName)
    await save(page, endpoint.id)
    await expect.poll(async () => (await details(request, endpoint.id)).name).toBe(newName)
    // Saving opens a toast that can consume Escape before the dialog.
    await modal.getByRole('button', { name: 'Close', exact: true }).first().click()
    await expect(modal).not.toBeVisible()
    await page.getByPlaceholder('Search by name or URL...').fill(newName)
    await expect(page.getByRole('row').filter({ hasText: newName })).toBeVisible()
  })

  test('EE-02: Health Check Interval change is reflected in API', async ({ page, request, endpoint }) => {
    const modal = await openDetails(page, endpoint.name)
    await modal.getByLabel('Health Check Interval (sec)').fill('45')
    await save(page, endpoint.id)
    await expect.poll(async () => (await details(request, endpoint.id)).health_check_interval_secs).toBe(45)
  })

  test('EE-03: Inference Timeout change is reflected in API', async ({ page, request, endpoint }) => {
    const modal = await openDetails(page, endpoint.name)
    await expect(modal.getByLabel('Inference Timeout (sec)')).not.toHaveValue('240')
    await modal.getByLabel('Inference Timeout (sec)').fill('240')
    await save(page, endpoint.id)
    await expect.poll(async () => (await details(request, endpoint.id)).inference_timeout_secs).toBe(240)
  })

  test('EE-04: Notes change persists after reopening modal', async ({ page, request, endpoint }) => {
    const modal = await openDetails(page, endpoint.name)
    const notesText = `Test notes ${endpoint.id}`
    await modal.getByLabel('Notes').fill(notesText)
    await save(page, endpoint.id)
    await expect.poll(async () => (await details(request, endpoint.id)).notes).toBe(notesText)
    await modal.getByRole('button', { name: 'Close', exact: true }).first().click()
    await expect(modal).not.toBeVisible()
    const reopened = await openDetails(page, endpoint.name)
    await expect(reopened.getByLabel('Notes')).toHaveValue(notesText)
  })

  test('EE-05: Inference Timeout minimum value 10 saves successfully', async ({ page, request, endpoint }) => {
    const modal = await openDetails(page, endpoint.name)
    await modal.getByLabel('Inference Timeout (sec)').fill('10')
    await save(page, endpoint.id)
    await expect.poll(async () => (await details(request, endpoint.id)).inference_timeout_secs).toBe(10)
    await expect(page.locator(DashboardSelectors.errorBanner)).not.toBeVisible()
  })

  test('EE-06: Inference Timeout maximum value 600 saves successfully', async ({ page, request, endpoint }) => {
    const modal = await openDetails(page, endpoint.name)
    await modal.getByLabel('Inference Timeout (sec)').fill('600')
    await save(page, endpoint.id)
    await expect.poll(async () => (await details(request, endpoint.id)).inference_timeout_secs).toBe(600)
    await expect(page.locator(DashboardSelectors.errorBanner)).not.toBeVisible()
  })

  test('EE-07: Add Endpoint dialog shows timeout guidance', async ({ page }) => {
    await ensureDashboardLogin(page)

    await page.getByRole('button', { name: 'Add Endpoint' }).click()

    const dialog = page.getByRole('dialog').filter({ hasText: 'Add New Endpoint' })
    await expect(dialog).toBeVisible({ timeout: 10000 })
    await expect(
      dialog.getByText('Local runtimes (xLLM, Ollama, LM Studio) default to 600 seconds.')
    ).toBeVisible()
    await expect(
      dialog.getByText('vLLM, llama.cpp, and OpenAI-compatible endpoints default to 120 seconds.')
    ).toBeVisible()
  })

  test('EE-08: Ollama endpoints default to 600 seconds and show a recommendation', async ({
    page,
    request,
  }) => {
    const ollamaEndpointName = `e2e-edit-ollama-${Date.now()}`
    const ollamaMock = await startMockOpenAIEndpointServer({ endpointType: 'ollama' })

    try {
      const createResponse = await request.post(`${API_BASE}/api/endpoints`, {
        headers: AUTH_HEADER,
        data: { name: ollamaEndpointName, base_url: ollamaMock.baseUrl },
      })
      expect(createResponse.ok()).toBeTruthy()

      const createdEndpoint = (await createResponse.json()) as {
        id: string
        inference_timeout_secs: number
      }
      expect(createdEndpoint.inference_timeout_secs).toBe(600)

      await expect
        .poll(
          async () => {
            const endpoints = await listEndpoints(request)
            return endpoints.some((endpoint) => endpoint.name === ollamaEndpointName)
          },
          { timeout: 10000, intervals: [200, 500, 1000] }
        )
        .toBe(true)

      await ensureDashboardLogin(page)
      await page.getByPlaceholder('Search by name or URL...').fill(ollamaEndpointName)
      await page.waitForTimeout(500)

      const row = page.locator('tbody tr').filter({ hasText: ollamaEndpointName })
      await expect(row).toBeVisible({ timeout: 10000 })
      await row.locator('button[title="Details"]').click()

      const modal = page.locator('[role="dialog"]').filter({ hasText: ollamaEndpointName })
      await expect(modal).toBeVisible({ timeout: 10000 })
      await expect(modal.locator('#inferenceTimeout')).toHaveValue('600')
      await expect(modal.getByText('Recommended for Ollama: 600 seconds')).toBeVisible()
    } finally {
      await deleteEndpointsByName(request, ollamaEndpointName)
      await ollamaMock.close()
    }
  })
})
