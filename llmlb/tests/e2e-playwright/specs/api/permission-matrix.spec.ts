import { test, expect, type APIRequestContext } from '@playwright/test'
import {
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

interface EndpointUnderTest {
  method: 'GET' | 'POST'
  path: string
  body?: Record<string, unknown>
}

const ALLOWED_ENDPOINTS: EndpointUnderTest[] = [
  { method: 'GET', path: '/v1/models' },
  {
    method: 'POST',
    path: '/v1/chat/completions',
    body: {
      model: 'mock-model-a',
      messages: [{ role: 'user', content: 'test' }],
    },
  },
]

const DENIED_ENDPOINTS: EndpointUnderTest[] = [
  { method: 'GET', path: '/api/endpoints' },
  { method: 'GET', path: '/api/users' },
  { method: 'GET', path: '/api/metrics/cloud' },
  { method: 'GET', path: '/api/me/api-keys' },
]

async function sendRequest(
  request: APIRequestContext,
  endpoint: EndpointUnderTest,
  apiKey: string
): Promise<number> {
  const headers: Record<string, string> = {
    Authorization: `Bearer ${apiKey}`,
  }
  if (endpoint.body) {
    headers['Content-Type'] = 'application/json'
  }

  const url = `${API_BASE}${endpoint.path}`

  if (endpoint.method === 'GET') {
    const resp = await request.get(url, { headers })
    return resp.status()
  }

  const resp = await request.post(url, { headers, data: endpoint.body })
  return resp.status()
}

test.describe('API Permission Matrix @api', () => {
  test.describe.configure({ mode: 'serial' })

  let mock: MockOpenAIEndpointServer
  const endpointName = `e2e-perm-matrix-${Date.now()}`

  test.beforeAll(async ({ request }) => {
    mock = await startMockOpenAIEndpointServer({ models: ['mock-model-a'] })

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
    await deleteEndpointsByName(request, endpointName)
    await mock.close()
  })

  test('self-issued key allows OpenAI-compatible endpoints', async ({ request }) => {
    const keyData: CreatedApiKey = await createApiKeyWithPermissions(
      request,
      `e2e-pm-allow-${Date.now()}`,
      ['openai.inference', 'openai.models.read']
    )
    expect(keyData.key).toBeTruthy()

    try {
      for (const endpoint of ALLOWED_ENDPOINTS) {
        const status = await sendRequest(request, endpoint, keyData.key)
        expect(status, `${endpoint.method} ${endpoint.path} should not be 401`).not.toBe(401)
        expect(status, `${endpoint.method} ${endpoint.path} should not be 403`).not.toBe(403)
      }
    } finally {
      await deleteApiKey(request, keyData.id)
    }
  })

  test('self-issued key cannot access internal/admin APIs', async ({ request }) => {
    const keyData: CreatedApiKey = await createApiKeyWithPermissions(
      request,
      `e2e-pm-deny-${Date.now()}`,
      ['openai.inference', 'openai.models.read']
    )
    expect(keyData.key).toBeTruthy()

    try {
      for (const endpoint of DENIED_ENDPOINTS) {
        const status = await sendRequest(request, endpoint, keyData.key)
        expect(
          [401, 403],
          `${endpoint.method} ${endpoint.path} should be unauthorized or forbidden`
        ).toContain(status)
      }
    } finally {
      await deleteApiKey(request, keyData.id)
    }
  })
})
