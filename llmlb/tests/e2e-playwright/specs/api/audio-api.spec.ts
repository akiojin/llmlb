import { test, expect } from '@playwright/test'
import { deleteEndpointsByName } from '../../helpers/api-helpers'
import { startMockOpenAIEndpointServer, type MockOpenAIEndpointServer } from '../../helpers/mock-openai-endpoint'

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768'
const AUTH_HEADER = { Authorization: 'Bearer sk_debug', 'Content-Type': 'application/json' }

test.describe.configure({ mode: 'serial' })

test.describe('Audio API @api', () => {
  let mock: MockOpenAIEndpointServer
  const endpointName = `e2e-audio-${Date.now()}`

  test.beforeAll(async ({ request }) => {
    mock = await startMockOpenAIEndpointServer({ supportAudio: true })

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

  // Audio proxy requires endpoints registered with AudioTranscription capability.
  // Mock endpoints only register with default ChatCompletion capability.
  // llmlb uses capability-based routing; audio/image capabilities are not auto-detected.
  test.skip('AA-01: /v1/audio/transcriptions -> mock transcription proxy', async ({ request }) => {
    test.setTimeout(30000)

    const resp = await request.post(`${API_BASE}/v1/audio/transcriptions`, {
      headers: { Authorization: 'Bearer sk_debug' },
      multipart: {
        file: {
          name: 'test.wav',
          mimeType: 'audio/wav',
          buffer: Buffer.from('fake audio data'),
        },
        model: mock.models[0],
      },
    })
    expect(resp.ok()).toBeTruthy()
    const json = await resp.json()
    expect(json.text).toContain('MOCK_TRANSCRIPTION')
  })

  // Audio proxy requires endpoints registered with AudioSpeech capability.
  test.skip('AA-02: /v1/audio/speech -> binary audio response', async ({ request }) => {
    test.setTimeout(30000)

    const resp = await request.post(`${API_BASE}/v1/audio/speech`, {
      headers: { Authorization: 'Bearer sk_debug', 'Content-Type': 'application/json' },
      data: { model: mock.models[0], input: 'Hello', voice: 'alloy' },
    })
    expect(resp.ok()).toBeTruthy()
    const contentType = resp.headers()['content-type']
    expect(contentType).toContain('audio')
  })

  test('AA-03: /v1/audio/transcriptions without auth -> 401', async ({ request }) => {
    const resp = await request.post(`${API_BASE}/v1/audio/transcriptions`, {
      multipart: {
        file: {
          name: 'test.wav',
          mimeType: 'audio/wav',
          buffer: Buffer.from('fake'),
        },
        model: 'any',
      },
    })
    expect(resp.status()).toBe(401)
  })
})
