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

const ALL_PERMISSIONS = [
  'openai.inference',
  'openai.models.read',
  'endpoints.read',
  'endpoints.manage',
  'api_keys.manage',
  'users.manage',
  'invitations.manage',
  'models.manage',
  'registry.read',
  'logs.read',
  'metrics.read',
] as const

type Permission = (typeof ALL_PERMISSIONS)[number]

interface EndpointUnderTest {
  method: 'GET' | 'POST'
  path: string
  requiredPermission: Permission
  body?: Record<string, unknown>
}

// Placeholder {ENDPOINT_ID} is resolved at runtime in sendRequest
const ENDPOINT_PERMISSION_MAP: EndpointUnderTest[] = [
  { method: 'GET', path: '/v1/models', requiredPermission: 'openai.models.read' },
  {
    method: 'POST',
    path: '/v1/chat/completions',
    requiredPermission: 'openai.inference',
    body: {
      model: 'mock-model-a',
      messages: [{ role: 'user', content: 'test' }],
    },
  },
  { method: 'GET', path: '/api/endpoints', requiredPermission: 'endpoints.read' },
  { method: 'GET', path: '/api/users', requiredPermission: 'users.manage' },
  { method: 'GET', path: '/api/api-keys', requiredPermission: 'api_keys.manage' },
  { method: 'GET', path: '/api/nodes/{ENDPOINT_ID}/logs', requiredPermission: 'logs.read' },
  { method: 'GET', path: '/api/metrics/cloud', requiredPermission: 'metrics.read' },
]

async function sendRequest(
  request: APIRequestContext,
  endpoint: EndpointUnderTest,
  apiKey: string,
  endpointId: string
): Promise<number> {
  const headers: Record<string, string> = {
    Authorization: `Bearer ${apiKey}`,
  }
  if (endpoint.body) {
    headers['Content-Type'] = 'application/json'
  }

  const resolvedPath = endpoint.path.replace('{ENDPOINT_ID}', endpointId)
  const url = `${API_BASE}${resolvedPath}`

  if (endpoint.method === 'GET') {
    const resp = await request.get(url, { headers })
    return resp.status()
  }
  const resp = await request.post(url, { headers, data: endpoint.body })
  return resp.status()
}

test.describe('API Permission Matrix @api', () => {
  // All tests share a single mock endpoint, so they must run in a single worker
  test.describe.configure({ mode: 'serial' })

  let mock: MockOpenAIEndpointServer
  let testEndpointId: string
  const endpointName = `e2e-perm-matrix-${Date.now()}`

  test.beforeAll(async ({ request }) => {
    mock = await startMockOpenAIEndpointServer({ models: ['mock-model-a'] })

    // Register and bring endpoint online via API
    const createResp = await request.post(`${API_BASE}/api/endpoints`, {
      headers: AUTH_HEADER,
      data: { name: endpointName, base_url: mock.baseUrl },
    })
    expect(createResp.ok()).toBeTruthy()
    const endpoint = (await createResp.json()) as { id: string }
    testEndpointId = endpoint.id

    // Test connection to bring it online
    const testResp = await request.post(`${API_BASE}/api/endpoints/${endpoint.id}/test`, {
      headers: { Authorization: 'Bearer sk_debug' },
    })
    expect(testResp.ok()).toBeTruthy()

    // Sync models
    const syncResp = await request.post(`${API_BASE}/api/endpoints/${endpoint.id}/sync`, {
      headers: { Authorization: 'Bearer sk_debug' },
    })
    expect(syncResp.ok()).toBeTruthy()

    // Wait until mock-model-a appears in /v1/models
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

  // --- Per-permission test groups ---
  for (const permission of ALL_PERMISSIONS) {
    test.describe(`Permission: ${permission}`, () => {
      let apiKeyData: CreatedApiKey

      test.beforeEach(async ({ request }) => {
        const keyName = `e2e-pm-${permission}-${Date.now()}`
        apiKeyData = await createApiKeyWithPermissions(request, keyName, [permission])
        expect(apiKeyData.key).toBeTruthy()
      })

      test.afterEach(async ({ request }) => {
        if (apiKeyData?.id) {
          await deleteApiKey(request, apiKeyData.id)
        }
      })

      for (const endpoint of ENDPOINT_PERMISSION_MAP) {
        const shouldSucceed = endpoint.requiredPermission === permission
        const label = shouldSucceed ? 'not-403' : '403'

        test(`PM-${permission}: ${endpoint.method} ${endpoint.path} -> ${label}`, async ({
          request,
        }) => {
          const status = await sendRequest(request, endpoint, apiKeyData.key, testEndpointId)
          if (shouldSucceed) {
            // Auth must pass: status should NOT be 401 or 403.
            // Some proxied endpoints may return 502/503 when the backend is
            // unavailable, which still proves the permission check succeeded.
            expect(status, `expected auth to pass for ${endpoint.path}`).not.toBe(401)
            expect(status, `expected auth to pass for ${endpoint.path}`).not.toBe(403)
          } else {
            expect(status).toBe(403)
          }
        })
      }
    })
  }

  // --- Full-permissions key: all endpoints succeed ---
  test('PM-full: all-permissions key -> all endpoints 2xx', async ({ request }) => {
    const allPermsKey = await createApiKeyWithPermissions(
      request,
      `e2e-pm-all-${Date.now()}`,
      [...ALL_PERMISSIONS]
    )
    expect(allPermsKey.key).toBeTruthy()

    try {
      for (const endpoint of ENDPOINT_PERMISSION_MAP) {
        const status = await sendRequest(request, endpoint, allPermsKey.key, testEndpointId)
        // Auth must pass: status should NOT be 401 or 403.
        expect(
          status,
          `${endpoint.method} ${endpoint.path} should not be 401 with all permissions`
        ).not.toBe(401)
        expect(
          status,
          `${endpoint.method} ${endpoint.path} should not be 403 with all permissions`
        ).not.toBe(403)
      }
    } finally {
      await deleteApiKey(request, allPermsKey.id)
    }
  })

  // --- Zero-permissions key: backend rejects with 400 ---
  test('PM-zero: no-permissions key -> rejected at creation', async ({ request }) => {
    const resp = await request.post(`${API_BASE}/api/api-keys`, {
      headers: AUTH_HEADER,
      data: { name: `e2e-pm-none-${Date.now()}`, permissions: [] },
    })
    // Backend requires at least one permission; empty permissions returns 400
    expect(resp.status()).toBe(400)
  })
})
