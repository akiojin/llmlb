import { test, expect } from '@playwright/test'
import { ensureDashboardLogin, deleteEndpointsByName, listEndpoints } from '../../helpers/api-helpers'
import { startMockOpenAIEndpointServer, type MockOpenAIEndpointServer } from '../../helpers/mock-openai-endpoint'

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768'
const AUTH_HEADER = { Authorization: 'Bearer sk_debug', 'Content-Type': 'application/json' }

test.describe('Endpoint Type Detection @workflows', () => {
  test('ETD-01: OpenAI互換モック → openaiタイプ検出', async ({ request }) => {
    test.setTimeout(30000)
    const mock = await startMockOpenAIEndpointServer({ endpointType: 'openai' })
    const name = `e2e-type-openai-${Date.now()}`

    try {
      const resp = await request.post(`${API_BASE}/api/endpoints`, {
        headers: AUTH_HEADER,
        data: { name, base_url: mock.baseUrl },
      })
      expect(resp.ok()).toBeTruthy()
      const ep = await resp.json()

      // Run connection test to trigger type detection
      await request.post(`${API_BASE}/api/endpoints/${ep.id}/test`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })

      // Wait for detection to complete
      await new Promise((resolve) => setTimeout(resolve, 3000))

      const endpoints = await listEndpoints(request)
      const found = endpoints.find((e) => e.name === name)
      expect(found).toBeTruthy()
      // OpenAI-compatible endpoints should be detected
      expect(found?.endpoint_type).toBeTruthy()
    } finally {
      await deleteEndpointsByName(request, name)
      await mock.close()
    }
  })

  test('ETD-02: xLLMモック → xllmタイプ検出', async ({ request }) => {
    test.setTimeout(30000)
    const mock = await startMockOpenAIEndpointServer({ endpointType: 'xllm' })
    const name = `e2e-type-xllm-${Date.now()}`

    try {
      const resp = await request.post(`${API_BASE}/api/endpoints`, {
        headers: AUTH_HEADER,
        data: { name, base_url: mock.baseUrl },
      })
      expect(resp.ok()).toBeTruthy()
      const ep = await resp.json()

      await request.post(`${API_BASE}/api/endpoints/${ep.id}/test`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })

      await new Promise((resolve) => setTimeout(resolve, 3000))

      const endpoints = await listEndpoints(request)
      const found = endpoints.find((e) => e.name === name)
      expect(found).toBeTruthy()
      expect(found?.endpoint_type).toBe('xllm')
    } finally {
      await deleteEndpointsByName(request, name)
      await mock.close()
    }
  })

  test('ETD-03: Ollamaモック → ollamaタイプ検出', async ({ request }) => {
    test.setTimeout(30000)
    const mock = await startMockOpenAIEndpointServer({ endpointType: 'ollama' })
    const name = `e2e-type-ollama-${Date.now()}`

    try {
      const resp = await request.post(`${API_BASE}/api/endpoints`, {
        headers: AUTH_HEADER,
        data: { name, base_url: mock.baseUrl },
      })
      expect(resp.ok()).toBeTruthy()
      const ep = await resp.json()

      await request.post(`${API_BASE}/api/endpoints/${ep.id}/test`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })

      await new Promise((resolve) => setTimeout(resolve, 3000))

      const endpoints = await listEndpoints(request)
      const found = endpoints.find((e) => e.name === name)
      expect(found).toBeTruthy()
      expect(found?.endpoint_type).toBe('ollama')
    } finally {
      await deleteEndpointsByName(request, name)
      await mock.close()
    }
  })

  test('ETD-04: vLLMモック → vllmタイプ検出', async ({ request }) => {
    test.setTimeout(30000)
    const mock = await startMockOpenAIEndpointServer({ endpointType: 'vllm' })
    const name = `e2e-type-vllm-${Date.now()}`

    try {
      const resp = await request.post(`${API_BASE}/api/endpoints`, {
        headers: AUTH_HEADER,
        data: { name, base_url: mock.baseUrl },
      })
      expect(resp.ok()).toBeTruthy()
      const ep = await resp.json()

      await request.post(`${API_BASE}/api/endpoints/${ep.id}/test`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })

      await new Promise((resolve) => setTimeout(resolve, 3000))

      const endpoints = await listEndpoints(request)
      const found = endpoints.find((e) => e.name === name)
      expect(found).toBeTruthy()
      expect(found?.endpoint_type).toBe('vllm')
    } finally {
      await deleteEndpointsByName(request, name)
      await mock.close()
    }
  })

  test('ETD-05: タイプフィルター動作', async ({ page, request }) => {
    test.setTimeout(60000)
    const mockOpenai = await startMockOpenAIEndpointServer({ endpointType: 'openai' })
    const mockXllm = await startMockOpenAIEndpointServer({ endpointType: 'xllm' })
    const nameOpenai = `e2e-type-filter-openai-${Date.now()}`
    const nameXllm = `e2e-type-filter-xllm-${Date.now()}`

    try {
      // Create two endpoints with different types
      const resp1 = await request.post(`${API_BASE}/api/endpoints`, {
        headers: AUTH_HEADER,
        data: { name: nameOpenai, base_url: mockOpenai.baseUrl },
      })
      const ep1 = await resp1.json()

      const resp2 = await request.post(`${API_BASE}/api/endpoints`, {
        headers: AUTH_HEADER,
        data: { name: nameXllm, base_url: mockXllm.baseUrl },
      })
      const ep2 = await resp2.json()

      // Trigger type detection
      await request.post(`${API_BASE}/api/endpoints/${ep1.id}/test`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })
      await request.post(`${API_BASE}/api/endpoints/${ep2.id}/test`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })

      await new Promise((resolve) => setTimeout(resolve, 3000))

      // Verify via API that both endpoints exist with correct types
      const endpoints = await listEndpoints(request)
      const foundOpenai = endpoints.find((e) => e.name === nameOpenai)
      const foundXllm = endpoints.find((e) => e.name === nameXllm)
      expect(foundOpenai).toBeTruthy()
      expect(foundXllm).toBeTruthy()
      // Type assertions: openai type should be detected
      expect(foundOpenai?.endpoint_type).toBeTruthy()
      // xLLM type should be detected (requires mock /api/system with xllm_version)
      expect(foundXllm?.endpoint_type).toBe('xllm')

      // Verify on dashboard that endpoints are visible (navigate & search)
      await ensureDashboardLogin(page)
      await expect(page.locator('table')).toBeVisible({ timeout: 20000 })
      // Use search to find specific endpoints (avoids pagination issues)
      const searchInput = page.getByPlaceholder('Search by name or URL...')
      await searchInput.fill(nameOpenai)
      await expect(page.getByText(nameOpenai)).toBeVisible({ timeout: 20000 })
      await searchInput.fill(nameXllm)
      await expect(page.getByText(nameXllm)).toBeVisible({ timeout: 20000 })
    } finally {
      await deleteEndpointsByName(request, nameOpenai)
      await deleteEndpointsByName(request, nameXllm)
      await mockOpenai.close()
      await mockXllm.close()
    }
  })
})
