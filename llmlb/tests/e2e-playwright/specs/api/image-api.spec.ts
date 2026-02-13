import { test, expect } from '@playwright/test'
import { deleteEndpointsByName } from '../../helpers/api-helpers'
import { startMockOpenAIEndpointServer, type MockOpenAIEndpointServer } from '../../helpers/mock-openai-endpoint'

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768'
const AUTH_HEADER = { Authorization: 'Bearer sk_debug', 'Content-Type': 'application/json' }

test.describe.configure({ mode: 'serial' })

test.describe('Image API @api', () => {
  let mock: MockOpenAIEndpointServer
  const endpointName = `e2e-image-${Date.now()}`

  test.beforeAll(async ({ request }) => {
    mock = await startMockOpenAIEndpointServer({ supportImages: true })

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
  })

  test.afterAll(async ({ request }) => {
    await deleteEndpointsByName(request, endpointName)
    await mock.close()
  })

  // Image proxy requires endpoints registered with ImageGeneration capability.
  // Mock endpoints only register with default ChatCompletion capability.
  // llmlb uses capability-based routing; image capabilities are not auto-detected.
  test.skip('IA-01: /v1/images/generations -> ImageResponse', async ({ request }) => {
    test.setTimeout(30000)

    const resp = await request.post(`${API_BASE}/v1/images/generations`, {
      headers: AUTH_HEADER,
      data: { model: mock.models[0], prompt: 'a cat', n: 1, size: '256x256' },
    })
    expect(resp.ok()).toBeTruthy()
    const json = await resp.json()
    expect(json.data).toHaveLength(1)
    expect(json.data[0].url).toBeTruthy()
  })

  // Image proxy requires endpoints registered with ImageGeneration capability.
  test.skip('IA-02: /v1/images/edits -> ImageResponse (multipart)', async ({ request }) => {
    test.setTimeout(30000)

    const resp = await request.post(`${API_BASE}/v1/images/edits`, {
      headers: { Authorization: 'Bearer sk_debug' },
      multipart: {
        image: {
          name: 'test.png',
          mimeType: 'image/png',
          buffer: Buffer.from('fake image data'),
        },
        prompt: 'add a hat',
        model: mock.models[0],
      },
    })
    expect(resp.ok()).toBeTruthy()
    const json = await resp.json()
    expect(json.data).toHaveLength(1)
    expect(json.data[0].url).toBeTruthy()
  })

  // Image proxy requires endpoints registered with ImageGeneration capability.
  test.skip('IA-03: /v1/images/variations -> ImageResponse (multipart)', async ({ request }) => {
    test.setTimeout(30000)

    const resp = await request.post(`${API_BASE}/v1/images/variations`, {
      headers: { Authorization: 'Bearer sk_debug' },
      multipart: {
        image: {
          name: 'test.png',
          mimeType: 'image/png',
          buffer: Buffer.from('fake image data'),
        },
        model: mock.models[0],
      },
    })
    expect(resp.ok()).toBeTruthy()
    const json = await resp.json()
    expect(json.data).toHaveLength(1)
    expect(json.data[0].url).toBeTruthy()
  })

  test('IA-04: /v1/images/generations without auth -> 401', async ({ request }) => {
    const resp = await request.post(`${API_BASE}/v1/images/generations`, {
      data: { prompt: 'test' },
    })
    expect(resp.status()).toBe(401)
  })
})
