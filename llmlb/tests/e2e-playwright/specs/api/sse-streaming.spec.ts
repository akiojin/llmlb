import {
  test,
  expect,
  type APIRequestContext,
  type APIResponse,
} from '@playwright/test'
import { deleteEndpointsByName } from '../../helpers/api-helpers'
import { startMockOpenAIEndpointServer, type MockOpenAIEndpointServer } from '../../helpers/mock-openai-endpoint'

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768'
const AUTH_HEADER = { Authorization: 'Bearer sk_debug', 'Content-Type': 'application/json' }
const REQUEST_RETRIES = 3

async function postStreamingCompletionWithRetry(
  request: APIRequestContext,
  model: string,
  prompt: string
): Promise<APIResponse> {
  let lastResponse: APIResponse | null = null

  for (let attempt = 0; attempt < REQUEST_RETRIES; attempt += 1) {
    const response = await request.post(`${API_BASE}/v1/chat/completions`, {
      headers: AUTH_HEADER,
      data: {
        model,
        messages: [{ role: 'user', content: prompt }],
        stream: true,
      },
    })

    if (response.ok()) {
      return response
    }

    lastResponse = response
    if (attempt < REQUEST_RETRIES - 1) {
      await new Promise((resolve) => setTimeout(resolve, 300 * (attempt + 1)))
    }
  }

  return lastResponse as APIResponse
}

test.describe.configure({ mode: 'serial' })

test.describe('SSE Streaming @api', () => {
  let mock: MockOpenAIEndpointServer
  const endpointName = `e2e-sse-${Date.now()}-${Math.random().toString(16).slice(2)}`

  test.beforeAll(async ({ request }) => {
    mock = await startMockOpenAIEndpointServer()

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

  test('SSE-01: stream=true -> chunked lines -> [DONE] signal', async ({ request }) => {
    test.setTimeout(30000)

    const resp = await postStreamingCompletionWithRetry(request, mock.models[0], 'stream test')
    expect(resp.ok()).toBeTruthy()

    const text = await resp.text()
    const lines = text.split('\n').filter((l) => l.startsWith('data: '))
    expect(lines.length).toBeGreaterThan(1)
    // Last data line should be [DONE]
    expect(lines[lines.length - 1]).toBe('data: [DONE]')
  })

  test('SSE-02: each chunk is JSON with delta.content', async ({ request }) => {
    test.setTimeout(30000)

    const resp = await postStreamingCompletionWithRetry(request, mock.models[0], 'chunk format test')
    expect(resp.ok()).toBeTruthy()

    const text = await resp.text()
    const dataLines = text.split('\n').filter((l) => l.startsWith('data: '))

    // Parse non-[DONE] chunks and verify JSON structure
    const jsonChunks = dataLines
      .filter((l) => l !== 'data: [DONE]')
      .map((l) => {
        const jsonStr = l.replace(/^data: /, '')
        return JSON.parse(jsonStr)
      })

    expect(jsonChunks.length).toBeGreaterThan(0)
    for (const chunk of jsonChunks) {
      expect(chunk.choices).toBeTruthy()
      expect(chunk.choices[0].delta).toBeTruthy()
      expect(typeof chunk.choices[0].delta.content).toBe('string')
    }
  })

  test('SSE-03: completions stream received successfully', async ({ request }) => {
    test.setTimeout(30000)

    const resp = await postStreamingCompletionWithRetry(request, mock.models[0], 'another stream test')
    expect(resp.ok()).toBeTruthy()

    const text = await resp.text()
    // Verify SSE format: lines prefixed with "data: "
    const dataLines = text.split('\n').filter((l) => l.startsWith('data: '))
    expect(dataLines.length).toBeGreaterThan(1)

    // Verify content-type is SSE
    const contentType = resp.headers()['content-type']
    expect(contentType).toContain('text/event-stream')
  })

  test('SSE-04: all chunks concatenated -> complete response', async ({ request }) => {
    test.setTimeout(30000)

    const resp = await postStreamingCompletionWithRetry(request, mock.models[0], 'concat test')
    expect(resp.ok()).toBeTruthy()

    const text = await resp.text()
    const dataLines = text.split('\n').filter((l) => l.startsWith('data: '))

    // Concatenate all delta.content values
    const fullContent = dataLines
      .filter((l) => l !== 'data: [DONE]')
      .map((l) => {
        const jsonStr = l.replace(/^data: /, '')
        const parsed = JSON.parse(jsonStr)
        return parsed.choices?.[0]?.delta?.content ?? ''
      })
      .join('')

    // The concatenated content should contain the MOCK_OK response
    expect(fullContent).toContain('MOCK_OK')
  })
})
