import { test, expect } from '@playwright/test'
import { deleteEndpointsByName } from '../../helpers/api-helpers'
import { startMockOpenAIEndpointServer, type MockOpenAIEndpointServer } from '../../helpers/mock-openai-endpoint'

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768'
const AUTH_HEADER = { Authorization: 'Bearer sk_debug', 'Content-Type': 'application/json' }

test.describe.configure({ mode: 'serial' })

test.describe('Responses API @api', () => {
  let mock: MockOpenAIEndpointServer
  const endpointName = `e2e-responses-${Date.now()}`

  test.beforeAll(async ({ request }) => {
    // Use unique model name to avoid LB routing to another test's endpoint
    // that doesn't support /v1/responses
    mock = await startMockOpenAIEndpointServer({ supportResponses: true, models: ['mock-responses-model'] })

    const resp = await request.post(`${API_BASE}/api/endpoints`, {
      headers: AUTH_HEADER,
      data: { name: endpointName, base_url: mock.baseUrl },
    })
    expect(resp.ok()).toBeTruthy()
    const ep = (await resp.json()) as { id: string }

    const testResp = await request.post(`${API_BASE}/api/endpoints/${ep.id}/test`, {
      headers: { Authorization: 'Bearer sk_debug' },
    })
    expect(testResp.ok()).toBeTruthy()

    const syncResp = await request.post(`${API_BASE}/api/endpoints/${ep.id}/sync`, {
      headers: { Authorization: 'Bearer sk_debug' },
    })
    expect(syncResp.ok()).toBeTruthy()

    // Wait for model to appear in /v1/models
    await expect
      .poll(
        async () => {
          const r = await request.get(`${API_BASE}/v1/models`, {
            headers: { Authorization: 'Bearer sk_debug' },
          })
          if (!r.ok()) return [] as string[]
          const json = (await r.json()) as { data?: Array<{ id: string }> }
          return (json.data ?? []).map((m) => m.id)
        },
        { timeout: 30000 }
      )
      .toEqual(expect.arrayContaining(mock.models))
  })

  test.afterAll(async ({ request }) => {
    await deleteEndpointsByName(request, endpointName)
    await mock.close()
  })

  test('RA-01: /v1/responses -> ResponsesAPIResponse', async ({ request }) => {
    test.setTimeout(30000)

    const resp = await request.post(`${API_BASE}/v1/responses`, {
      headers: AUTH_HEADER,
      data: { model: mock.models[0], input: 'Hello from responses API' },
    })
    expect(resp.ok()).toBeTruthy()
    const json = await resp.json()
    expect(json.output).toBeTruthy()
  })

  test('RA-02: /v1/responses without auth -> 401', async ({ request }) => {
    const resp = await request.post(`${API_BASE}/v1/responses`, {
      data: { model: 'any', input: 'test' },
    })
    expect(resp.status()).toBe(401)
  })
})
