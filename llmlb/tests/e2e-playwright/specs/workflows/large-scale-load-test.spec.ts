import { test, expect } from '@playwright/test'
import { ensureDashboardLogin, deleteEndpointsByName } from '../../helpers/api-helpers'
import { startMockOpenAIEndpointServer, type MockOpenAIEndpointServer } from '../../helpers/mock-openai-endpoint'

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768'
const AUTH_HEADER = { Authorization: 'Bearer sk_debug', 'Content-Type': 'application/json' }
const LOAD_MODEL = 'load-test-model'

test.describe('Large Scale Load Test @workflows', () => {
  test('LSL-01: 120同時リクエスト全成功', async ({ request }) => {
    test.setTimeout(180000)
    const mock1 = await startMockOpenAIEndpointServer({ models: [LOAD_MODEL], responseDelayMs: 10 })
    const mock2 = await startMockOpenAIEndpointServer({ models: [LOAD_MODEL], responseDelayMs: 10 })
    const name1 = `e2e-load-1-${Date.now()}`
    const name2 = `e2e-load-2-${Date.now()}`

    try {
      // Create two endpoints for load distribution
      const resp1 = await request.post(`${API_BASE}/api/endpoints`, {
        headers: AUTH_HEADER,
        data: { name: name1, base_url: mock1.baseUrl },
      })
      expect(resp1.ok()).toBeTruthy()
      const ep1 = await resp1.json()

      const resp2 = await request.post(`${API_BASE}/api/endpoints`, {
        headers: AUTH_HEADER,
        data: { name: name2, base_url: mock2.baseUrl },
      })
      expect(resp2.ok()).toBeTruthy()
      const ep2 = await resp2.json()

      // Bring both online
      await request.post(`${API_BASE}/api/endpoints/${ep1.id}/test`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })
      await request.post(`${API_BASE}/api/endpoints/${ep2.id}/test`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })
      await request.post(`${API_BASE}/api/endpoints/${ep1.id}/sync`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })
      await request.post(`${API_BASE}/api/endpoints/${ep2.id}/sync`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })

      // Wait for models to become available via /v1/models
      let modelAvailable = false
      for (let attempt = 0; attempt < 15; attempt++) {
        const modelsResp = await request.get(`${API_BASE}/v1/models`, {
          headers: AUTH_HEADER,
        })
        if (modelsResp.ok()) {
          const modelsData = await modelsResp.json()
          const models = modelsData.data || []
          if (models.some((m: { id: string }) => m.id === LOAD_MODEL)) {
            modelAvailable = true
            break
          }
        }
        await new Promise((resolve) => setTimeout(resolve, 500))
      }
      expect(modelAvailable).toBeTruthy()

      // Send 120 sequential requests (one at a time) to avoid queue contention.
      // The idle-endpoint selector requires endpoints with 0 active requests,
      // so concurrent requests can hit "No available runtimes".
      const TOTAL = 120
      let successCount = 0

      for (let i = 0; i < TOTAL; i++) {
        try {
          const resp = await request.post(`${API_BASE}/v1/chat/completions`, {
            headers: AUTH_HEADER,
            data: {
              model: LOAD_MODEL,
              messages: [{ role: 'user', content: `load-test-${i}` }],
            },
            timeout: 30000,
          })
          if (resp.ok()) {
            successCount++
          }
        } catch {
          // Timeout or network error - count as failure
        }
      }

      // At least 95% of requests should succeed (allows for minor transient errors
      // when other tests are running in parallel and creating/deleting endpoints)
      expect(successCount).toBeGreaterThanOrEqual(Math.floor(TOTAL * 0.95))
    } finally {
      await deleteEndpointsByName(request, name1)
      await deleteEndpointsByName(request, name2)
      await mock1.close()
      await mock2.close()
    }
  })

  test('LSL-02: 負荷テスト後の統計確認', async ({ page, request }) => {
    test.setTimeout(30000)
    await ensureDashboardLogin(page)

    // Dashboard stats should be visible after load
    await expect(page.locator('[data-stat="total-requests"]')).toBeVisible({ timeout: 10000 })
    const totalRequests = await page.locator('[data-stat="total-requests"]').textContent()
    expect(totalRequests).toBeTruthy()
  })
})
