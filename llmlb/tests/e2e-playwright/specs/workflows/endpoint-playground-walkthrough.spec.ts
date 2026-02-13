import { test, expect } from '@playwright/test'
import { ensureDashboardLogin, deleteEndpointsByName } from '../../helpers/api-helpers'
import { startMockOpenAIEndpointServer, type MockOpenAIEndpointServer } from '../../helpers/mock-openai-endpoint'

test.describe.configure({ mode: 'serial' })

test.describe('Endpoint Playground Walkthrough @playground', () => {
  let mock: MockOpenAIEndpointServer
  let endpointName = ''

  test.beforeAll(async () => {
    mock = await startMockOpenAIEndpointServer()
  })

  test.afterAll(async () => {
    await mock.close()
  })

  test.afterEach(async ({ request }) => {
    if (endpointName) {
      await deleteEndpointsByName(request, endpointName)
    }
  })

  test('register -> test -> sync -> playground streaming chat -> delete', async ({ page, request }) => {
    endpointName = `e2e-mock-openai-${Date.now()}-${Math.random().toString(16).slice(2)}`

    await ensureDashboardLogin(page)

    // Create endpoint via UI (JWT + CSRF).
    await page.getByRole('button', { name: 'Add Endpoint' }).click()
    await page.fill('#endpoint-name', endpointName)
    await page.fill('#endpoint-url', mock.baseUrl)
    await page.getByRole('button', { name: 'Create Endpoint' }).click()

    // Use search to filter the table to our endpoint (avoids pagination issues)
    const searchInput = page.getByPlaceholder('Search by name or URL...')
    await expect(searchInput).toBeVisible({ timeout: 20000 })
    await searchInput.fill(endpointName)

    // The endpoints table should include the newly created endpoint.
    const row = page.getByRole('row').filter({ hasText: endpointName })
    await expect(row).toBeVisible({ timeout: 20000 })

    // Type detection should classify this as OpenAI-compatible.
    // The table shows a shortened label ("OpenAI") for openai_compatible endpoints.
    await expect(row.locator('td').nth(2).getByText('OpenAI', { exact: true })).toBeVisible({
      timeout: 20000,
    })

    // Run an explicit connection test to deterministically bring it online.
    await row.locator('button[title="Test Connection"]').click()
    await expect(row.getByText('Online')).toBeVisible({ timeout: 20000 })

    // Sync models (the "Open Playground" button requires models).
    await row.locator('button[title="Sync Models"]').click()

    // Wait for the row to stabilize after sync (React re-renders the table)
    await page.waitForTimeout(2000)

    // Re-locate the row after potential DOM re-render (search is still active)
    const rowAfterSync = page.getByRole('row').filter({ hasText: endpointName })
    await expect(rowAfterSync).toBeVisible({ timeout: 10000 })

    // Open detail modal, then open Playground.
    await rowAfterSync.locator('button[title="Details"]').click()
    const detailsDialog = page.getByRole('dialog').filter({ hasText: endpointName })
    await expect(detailsDialog).toBeVisible({ timeout: 20000 })

    // Wait until at least one model is visible in the modal.
    await expect(detailsDialog.getByText(mock.models[0])).toBeVisible({ timeout: 20000 })

    await detailsDialog.getByRole('button', { name: 'Open Playground' }).click()

    // Playground should load.
    await expect(page.getByText('Start a conversation')).toBeVisible({ timeout: 20000 })
    await expect(page.getByText('Streaming')).toBeVisible({ timeout: 20000 })

    // Select a deterministic model.
    const modelSelect = page.getByRole('combobox').first()
    await modelSelect.click()
    await page.getByRole('option', { name: mock.models[0] }).click()

    // Send a message and verify streaming response is rendered (no 502).
    const input = page.getByPlaceholder('Type a message or attach files...')
    const userMessage = 'Hello from Playwright E2E'
    await input.fill(userMessage)
    await page.getByRole('button', { name: 'Send' }).click()

    await expect(page.getByText('MOCK_OK')).toBeVisible({ timeout: 20000 })

    // Go back to dashboard and delete via UI to cover the deletion flow.
    await page.getByRole('button', { name: 'Back to Dashboard' }).click()
    // Use search to find our endpoint (avoids pagination issues)
    const searchAfterReturn = page.getByPlaceholder('Search by name or URL...')
    await expect(searchAfterReturn).toBeVisible({ timeout: 20000 })
    await searchAfterReturn.fill(endpointName)
    const rowAfterReturn = page.getByRole('row').filter({ hasText: endpointName })
    await expect(rowAfterReturn).toBeVisible({ timeout: 20000 })

    await rowAfterReturn.locator('button[title="Delete"]').click()
    // Delete confirmation is an AlertDialog (role=alertdialog).
    const deleteDialog = page.getByRole('alertdialog').filter({ hasText: 'Delete Endpoint?' })
    await expect(deleteDialog).toBeVisible({ timeout: 20000 })
    await deleteDialog.getByRole('button', { name: 'Delete' }).click()

    // Confirm endpoint is removed from UI (best-effort; also cleaned up via API in afterEach).
    await expect(page.getByText(endpointName)).toBeHidden({ timeout: 20000 })

    // Also confirm via API.
    const deletedCount = await deleteEndpointsByName(request, endpointName)
    expect(deletedCount).toBe(0)
  })
})
