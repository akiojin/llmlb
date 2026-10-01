import { test, expect } from '@playwright/test'
import { ensureDashboardLogin, deleteEndpointsByName } from '../../helpers/api-helpers'
import { startMockOpenAIEndpointServer } from '../../helpers/mock-openai-endpoint'

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768'
const AUTH_HEADER = { Authorization: 'Bearer sk_debug', 'Content-Type': 'application/json' }

// The dashboard polls every 10s while the WebSocket is connected, so a refetch that
// starts well within that window after an event frame was driven by the event.
const LIVE_REFETCH_WINDOW_MS = 3000

const OVERVIEW = '/api/dashboard/overview'
const REQUEST_RESPONSES = '/api/dashboard/request-responses'

interface ReceivedFrame {
  type: string
  runtimeId?: string
  at: number
}

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`WebSocket event invalidation (${colorScheme}) @dashboard`, () => {
    test.use({ colorScheme })

    // SPEC #582 FR-048f / Issue #781 AC-4: the three events that gained a publisher in
    // #781 reach the dashboard and refetch the queries the invalidation table maps them to.
    test('NodeRegistered / MetricsUpdated / NodeRemoved refetch their queries (#781)', async ({
      page,
      request,
    }) => {
      test.setTimeout(120_000)

      const unique = `${Date.now()}-${Math.random().toString(16).slice(2)}`
      const endpointName = `e2e-ws-invalidation-${unique}`
      // A model no parallel spec serves, so the request below is routed to this endpoint.
      const model = `ws-invalidation-${unique}`
      const mock = await startMockOpenAIEndpointServer({ models: [model] })

      const frames: ReceivedFrame[] = []
      const fetches: { path: string; at: number }[] = []
      const pageErrors: string[] = []

      // The pre-login session probe returns 401 by design; only collect errors after login.
      let loggedIn = false
      page.on('pageerror', (err) => {
        if (loggedIn) pageErrors.push(err.message)
      })
      page.on('console', (msg) => {
        if (loggedIn && msg.type() === 'error') pageErrors.push(msg.text())
      })
      page.on('websocket', (ws) => {
        if (!ws.url().includes('/ws/dashboard')) return
        ws.on('framereceived', ({ payload }) => {
          try {
            const event = JSON.parse(payload.toString())
            frames.push({ type: event.type, runtimeId: event.data?.runtime_id, at: Date.now() })
          } catch {
            // ignore non-JSON frames
          }
        })
      })
      page.on('request', (req) => {
        const path = new URL(req.url()).pathname
        if (path === OVERVIEW || path === REQUEST_RESPONSES) fetches.push({ path, at: Date.now() })
      })

      /** Wait for this endpoint's `type` frame, then for a refetch of each path right after it. */
      const expectRefetchAfter = async (type: string, runtimeId: string, paths: string[]) => {
        const frame = () => frames.find((f) => f.type === type && f.runtimeId === runtimeId)
        await expect.poll(frame, { message: `${type} frame`, timeout: 20_000 }).toBeTruthy()
        const { at } = frame()!
        for (const path of paths) {
          await expect
            .poll(
              () =>
                fetches.some(
                  (f) => f.path === path && f.at >= at && f.at - at <= LIVE_REFETCH_WINDOW_MS,
                ),
              { message: `${path} refetched after ${type}`, timeout: 10_000 },
            )
            .toBe(true)
        }
      }

      try {
        await ensureDashboardLogin(page)
        loggedIn = true
        await expect
          .poll(() => frames.some((f) => f.type === 'connected'), { timeout: 15_000 })
          .toBe(true)

        // Registration -> NodeRegistered
        const created = await request.post(`${API_BASE}/api/endpoints`, {
          headers: AUTH_HEADER,
          data: { name: endpointName, base_url: mock.baseUrl },
        })
        expect(created.ok()).toBe(true)
        const endpointId: string = (await created.json()).id
        await expectRefetchAfter('NodeRegistered', endpointId, [OVERVIEW, REQUEST_RESPONSES])

        // A completed inference request -> MetricsUpdated
        await request.post(`${API_BASE}/api/endpoints/${endpointId}/test`, { headers: AUTH_HEADER })
        await request.post(`${API_BASE}/api/endpoints/${endpointId}/sync`, { headers: AUTH_HEADER })
        const completion = await request.post(`${API_BASE}/v1/chat/completions`, {
          headers: AUTH_HEADER,
          data: { model, messages: [{ role: 'user', content: 'ws invalidation' }] },
        })
        expect(completion.ok()).toBe(true)
        await expectRefetchAfter('MetricsUpdated', endpointId, [OVERVIEW])

        // Deletion -> NodeRemoved
        const deleted = await request.delete(`${API_BASE}/api/endpoints/${endpointId}`, {
          headers: AUTH_HEADER,
        })
        expect(deleted.ok()).toBe(true)
        await expectRefetchAfter('NodeRemoved', endpointId, [OVERVIEW, REQUEST_RESPONSES])

        expect(pageErrors).toEqual([])
      } finally {
        await deleteEndpointsByName(request, endpointName)
        await mock.close()
      }
    })
  })
}
