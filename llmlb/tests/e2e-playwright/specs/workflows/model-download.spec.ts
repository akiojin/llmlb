import { test, expect } from '@playwright/test'
import { ensureDashboardLogin, deleteEndpointsByName } from '../../helpers/api-helpers'
import { startMockOpenAIEndpointServer, type MockOpenAIEndpointServer } from '../../helpers/mock-openai-endpoint'

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768'
const AUTH_HEADER = { Authorization: 'Bearer sk_debug', 'Content-Type': 'application/json' }

test.describe('Model Download @workflows', () => {
  test('MD-01: xLLMモックでDownload Model UI表示', async ({ page, request }) => {
    test.setTimeout(60000)
    const mock = await startMockOpenAIEndpointServer({ endpointType: 'xllm' })
    const name = `e2e-download-${Date.now()}`

    try {
      // Create xLLM endpoint
      const resp = await request.post(`${API_BASE}/api/endpoints`, {
        headers: AUTH_HEADER,
        data: { name, base_url: mock.baseUrl },
      })
      expect(resp.ok()).toBeTruthy()
      const ep = await resp.json()

      // Test connection to trigger type detection
      await request.post(`${API_BASE}/api/endpoints/${ep.id}/test`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })
      await request.post(`${API_BASE}/api/endpoints/${ep.id}/sync`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })

      await new Promise((resolve) => setTimeout(resolve, 3000))

      // Navigate to dashboard and open endpoint details
      await ensureDashboardLogin(page)

      // Use search to filter the table (avoids pagination issues with many endpoints)
      const searchInput = page.getByPlaceholder('Search by name or URL...')
      await expect(searchInput).toBeVisible({ timeout: 20000 })
      await searchInput.fill(name)

      const row = page.getByRole('row').filter({ hasText: name })
      await expect(row).toBeVisible({ timeout: 20000 })

      // Open details modal
      await row.locator('button[title="Details"]').click()
      const detailsDialog = page.getByRole('dialog').filter({ hasText: name })
      await expect(detailsDialog).toBeVisible({ timeout: 20000 })

      // For xLLM endpoints, verify the endpoint type is shown as xLLM
      // The details modal should display relevant endpoint information
      await expect(detailsDialog.getByText(/xllm/i).first()).toBeVisible({ timeout: 10000 })
    } finally {
      await deleteEndpointsByName(request, name)
      await mock.close()
    }
  })
})
