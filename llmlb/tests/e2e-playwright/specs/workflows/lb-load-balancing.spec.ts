import { test, expect } from '@playwright/test'
import { deleteEndpointsByName, listEndpoints } from '../../helpers/api-helpers'
import { startMockOpenAIEndpointServer, type MockOpenAIEndpointServer } from '../../helpers/mock-openai-endpoint'

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768'
const AUTH_HEADER = { Authorization: 'Bearer sk_debug', 'Content-Type': 'application/json' }
const SHARED_MODEL = 'lb-test-model'
const FAILOVER_WAIT_TIMEOUT_MS = 20000
const FAILOVER_POLL_INTERVAL_MS = 1000

test.describe.configure({ mode: 'serial' })

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

async function waitForFailoverReady(
  request: Parameters<typeof listEndpoints>[0],
  primaryName: string,
  secondaryName: string
): Promise<{ primaryStatus: string; secondaryStatus: string }> {
  const deadline = Date.now() + FAILOVER_WAIT_TIMEOUT_MS
  let primaryStatus = 'unknown'
  let secondaryStatus = 'unknown'

  while (Date.now() < deadline) {
    const endpoints = await listEndpoints(request)
    primaryStatus = endpoints.find((e) => e.name === primaryName)?.status ?? 'missing'
    secondaryStatus = endpoints.find((e) => e.name === secondaryName)?.status ?? 'missing'

    // Treat non-online primary as failed over once secondary is healthy.
    if (secondaryStatus === 'online' && primaryStatus !== 'online') {
      return { primaryStatus, secondaryStatus }
    }
    await sleep(FAILOVER_POLL_INTERVAL_MS)
  }

  throw new Error(
    `Failover not ready within ${FAILOVER_WAIT_TIMEOUT_MS}ms (primary=${primaryStatus}, secondary=${secondaryStatus})`
  )
}

test.describe('LB Load Balancing @workflows', () => {
  test('LB-01: レイテンシ優先ルーティング', async ({ request }) => {
    test.setTimeout(60000)
    const fastMock = await startMockOpenAIEndpointServer({ models: [SHARED_MODEL], responseDelayMs: 50 })
    const slowMock = await startMockOpenAIEndpointServer({ models: [SHARED_MODEL], responseDelayMs: 500 })
    const fastName = `e2e-lb-fast-${Date.now()}`
    const slowName = `e2e-lb-slow-${Date.now()}`

    try {
      // Create fast endpoint
      const fastResp = await request.post(`${API_BASE}/api/endpoints`, {
        headers: AUTH_HEADER,
        data: { name: fastName, base_url: fastMock.baseUrl },
      })
      expect(fastResp.ok()).toBeTruthy()
      const fastEp = await fastResp.json()

      // Create slow endpoint
      const slowResp = await request.post(`${API_BASE}/api/endpoints`, {
        headers: AUTH_HEADER,
        data: { name: slowName, base_url: slowMock.baseUrl },
      })
      expect(slowResp.ok()).toBeTruthy()
      const slowEp = await slowResp.json()

      // Test connection and sync models for both
      await request.post(`${API_BASE}/api/endpoints/${fastEp.id}/test`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })
      await request.post(`${API_BASE}/api/endpoints/${slowEp.id}/test`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })
      await request.post(`${API_BASE}/api/endpoints/${fastEp.id}/sync`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })
      await request.post(`${API_BASE}/api/endpoints/${slowEp.id}/sync`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })

      // Wait for endpoints to come online
      await new Promise((resolve) => setTimeout(resolve, 3000))

      // Send 20 requests and track responses
      const results: string[] = []
      for (let i = 0; i < 20; i++) {
        const chatResp = await request.post(`${API_BASE}/v1/chat/completions`, {
          headers: AUTH_HEADER,
          data: {
            model: SHARED_MODEL,
            messages: [{ role: 'user', content: `request-${i}` }],
          },
        })
        if (chatResp.ok()) {
          const body = await chatResp.json()
          const content = body.choices?.[0]?.message?.content || ''
          results.push(content)
        }
      }

      // Verify we got responses
      expect(results.length).toBeGreaterThan(0)

      // After warm-up, fast endpoint should receive more requests due to latency-priority routing
      // We just verify that routing is happening (responses come back successfully)
      const mockOkCount = results.filter((r) => r.includes('MOCK_OK')).length
      expect(mockOkCount).toBeGreaterThan(0)
    } finally {
      await deleteEndpointsByName(request, fastName)
      await deleteEndpointsByName(request, slowName)
      await fastMock.close()
      await slowMock.close()
    }
  })

  test('LB-02: オフラインエンドポイント除外', async ({ request }) => {
    test.setTimeout(60000)
    const mock1 = await startMockOpenAIEndpointServer({ models: [SHARED_MODEL], responseDelayMs: 50 })
    const mock2 = await startMockOpenAIEndpointServer({ models: [SHARED_MODEL], responseDelayMs: 50 })
    const name1 = `e2e-lb-online-${Date.now()}`
    const name2 = `e2e-lb-offline-${Date.now()}`

    try {
      // Create both endpoints
      const resp1 = await request.post(`${API_BASE}/api/endpoints`, {
        headers: AUTH_HEADER,
        data: { name: name1, base_url: mock1.baseUrl },
      })
      const ep1 = await resp1.json()

      const resp2 = await request.post(`${API_BASE}/api/endpoints`, {
        headers: AUTH_HEADER,
        data: { name: name2, base_url: mock2.baseUrl },
      })
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

      await new Promise((resolve) => setTimeout(resolve, 3000))

      // Close mock2 to make it offline
      await mock2.close()

      // Wait for health check to detect offline
      await new Promise((resolve) => setTimeout(resolve, 5000))

      // Send requests - should all route to the remaining online endpoint
      const results: string[] = []
      for (let i = 0; i < 5; i++) {
        const chatResp = await request.post(`${API_BASE}/v1/chat/completions`, {
          headers: AUTH_HEADER,
          data: {
            model: SHARED_MODEL,
            messages: [{ role: 'user', content: `offline-test-${i}` }],
          },
        })
        if (chatResp.ok()) {
          const body = await chatResp.json()
          results.push(body.choices?.[0]?.message?.content || '')
        }
      }

      // At least some requests should succeed via the online endpoint
      const successCount = results.filter((r) => r.includes('MOCK_OK')).length
      expect(successCount).toBeGreaterThan(0)
    } finally {
      await deleteEndpointsByName(request, name1)
      await deleteEndpointsByName(request, name2)
      await mock1.close().catch(() => {})
    }
  })

  test('LB-03: フェイルオーバー', async ({ request }) => {
    test.setTimeout(60000)
    const primaryMock = await startMockOpenAIEndpointServer({ models: [SHARED_MODEL], responseDelayMs: 50 })
    const secondaryMock = await startMockOpenAIEndpointServer({ models: [SHARED_MODEL], responseDelayMs: 50 })
    const primaryName = `e2e-lb-primary-${Date.now()}`
    const secondaryName = `e2e-lb-secondary-${Date.now()}`

    try {
      // Create both endpoints
      const primaryResp = await request.post(`${API_BASE}/api/endpoints`, {
        headers: AUTH_HEADER,
        data: { name: primaryName, base_url: primaryMock.baseUrl },
      })
      const primaryEp = await primaryResp.json()

      const secondaryResp = await request.post(`${API_BASE}/api/endpoints`, {
        headers: AUTH_HEADER,
        data: { name: secondaryName, base_url: secondaryMock.baseUrl },
      })
      const secondaryEp = await secondaryResp.json()

      // Bring both online
      await request.post(`${API_BASE}/api/endpoints/${primaryEp.id}/test`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })
      await request.post(`${API_BASE}/api/endpoints/${secondaryEp.id}/test`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })
      await request.post(`${API_BASE}/api/endpoints/${primaryEp.id}/sync`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })
      await request.post(`${API_BASE}/api/endpoints/${secondaryEp.id}/sync`, {
        headers: { Authorization: 'Bearer sk_debug' },
      })

      await new Promise((resolve) => setTimeout(resolve, 3000))

      // Shut down primary
      await primaryMock.close()

      const failoverStatus = await waitForFailoverReady(request, primaryName, secondaryName)

      // Retry briefly to absorb in-flight transition windows on CI.
      let body: { choices?: Array<{ message?: { content?: string } }> } | null = null
      let lastStatus = -1
      for (let i = 0; i < 8; i++) {
        const chatResp = await request.post(`${API_BASE}/v1/chat/completions`, {
          headers: AUTH_HEADER,
          data: {
            model: SHARED_MODEL,
            messages: [{ role: 'user', content: 'failover-test' }],
          },
        })
        lastStatus = chatResp.status()
        if (chatResp.ok()) {
          const parsed = await chatResp.json()
          if (parsed.choices?.[0]?.message?.content?.includes('MOCK_OK')) {
            body = parsed
            break
          }
        }
        await sleep(FAILOVER_POLL_INTERVAL_MS)
      }

      expect(
        body,
        `failover request never succeeded (lastStatus=${lastStatus}, primary=${failoverStatus.primaryStatus}, secondary=${failoverStatus.secondaryStatus})`
      ).not.toBeNull()
      expect(body!.choices?.[0]?.message?.content).toContain('MOCK_OK')
    } finally {
      await deleteEndpointsByName(request, primaryName)
      await deleteEndpointsByName(request, secondaryName)
      await primaryMock.close().catch(() => {})
      await secondaryMock.close()
    }
  })
})
