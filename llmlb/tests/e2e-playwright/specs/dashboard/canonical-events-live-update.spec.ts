import { test, expect } from '@playwright/test'
import { ensureDashboardLogin, deleteEndpoint, deleteEndpointsByName } from '../../helpers/api-helpers'
import { startMockOpenAIEndpointServer } from '../../helpers/mock-openai-endpoint'

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768'
const AUTH_HEADER = { Authorization: 'Bearer sk_debug', 'Content-Type': 'application/json' }

// SPEC #582 US-007 "Client invalidation rules": the request behind each invalidated query key.
const OVERVIEW = '/api/dashboard/overview' // ['dashboard-overview']
const REQUEST_RESPONSES = '/api/dashboard/request-responses' // ['request-responses']

type LogEntry =
  | { kind: 'frame'; changed: string; id?: string }
  | { kind: 'request'; path: string }

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Canonical events live update (${colorScheme}) @dashboard`, () => {
    test.use({ colorScheme })

    // The dashboard also polls, so a lost event shows no symptom on screen (#781).
    // This test stops the page clock: with polling frozen, only a WebSocket event
    // can make the dashboard refetch.
    test('NodeRegistered, MetricsUpdated and NodeRemoved refetch their queries without polling (#785)', async ({
      page,
      request,
    }) => {
      test.setTimeout(120_000)

      const suffix = `${Date.now()}-${Math.random().toString(16).slice(2)}`
      const endpointName = `e2e-canonical-events-${suffix}`
      // A model name no other endpoint serves, so the proxied request reaches this endpoint.
      const mock = await startMockOpenAIEndpointServer({ models: [`e2e-canonical-events-model-${suffix}`] })

      // WebSocket frames and dashboard API requests, in the order the browser saw them.
      const log: LogEntry[] = []
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
            const keys = Object.keys(event).sort().join(',')
            if (!['changed', 'changed,id'].includes(keys) || typeof event.changed !== 'string') {
              pageErrors.push(`unexpected wire: ${payload}`)
            }
            log.push({ kind: 'frame', changed: event.changed, id: event.id })
          } catch {
            // ignore non-JSON frames
          }
        })
      })
      page.on('request', (req) => {
        log.push({ kind: 'request', path: new URL(req.url()).pathname })
      })

      let endpointId = ''
      let checkpoint = 0
      const refetchedAfter = (resource: string, path: string) => {
        const frame = log.findIndex(
          (entry, index) => index >= checkpoint && entry.kind === 'frame' && entry.changed === resource && entry.id === endpointId,
        )
        if (frame < 0) return `no ${resource} frame`
        const refetched = log.slice(frame + 1).some((entry) => entry.kind === 'request' && entry.path === path)
        return refetched ? 'refetched' : `no ${path} request after ${resource}`
      }
      const expectRefetchAfter = (resource: string, path: string) =>
        expect.poll(() => refetchedAfter(resource, path), { timeout: 20_000 }).toBe('refetched')

      try {
        await page.addInitScript(() => {
          const NativeWebSocket = window.WebSocket
          Object.assign(window, { dashboardSocketOpen: false })
          window.WebSocket = class extends NativeWebSocket {
            constructor(url: string | URL, protocols?: string | string[]) {
              super(url, protocols)
              if (new URL(url, location.href).pathname === '/ws/dashboard') {
                this.addEventListener('open', () => Object.assign(window, { dashboardSocketOpen: true }))
                this.addEventListener('close', () => Object.assign(window, { dashboardSocketOpen: false }))
              }
            }
          }
        })
        await page.clock.install()
        await ensureDashboardLogin(page)
        loggedIn = true
        await expect.poll(() => page.evaluate(() => Reflect.get(window, 'dashboardSocketOpen'))).toBe(true)

        await page.clock.pauseAt(Date.now() + 1000)
        const pausedAt = await page.evaluate(() => Date.now())

        // NodeRegistered: registered through the API, as another admin would
        checkpoint = log.length
        const created = await request.post(`${API_BASE}/api/endpoints`, {
          headers: AUTH_HEADER,
          data: { name: endpointName, base_url: mock.baseUrl },
        })
        expect(created.ok()).toBe(true)
        endpointId = (await created.json()).id
        await expectRefetchAfter('endpoints', OVERVIEW)
        await expectRefetchAfter('endpoints', REQUEST_RESPONSES)

        // MetricsUpdated: published when a proxied request completes
        await request.post(`${API_BASE}/api/endpoints/${endpointId}/test`, { headers: AUTH_HEADER })
        await request.post(`${API_BASE}/api/endpoints/${endpointId}/sync`, { headers: AUTH_HEADER })
        checkpoint = log.length
        const completion = await request.post(`${API_BASE}/v1/chat/completions`, {
          headers: AUTH_HEADER,
          data: { model: mock.models[0], messages: [{ role: 'user', content: 'canonical events' }] },
        })
        expect(completion.ok()).toBe(true)
        await expectRefetchAfter('metrics', OVERVIEW)

        // NodeRemoved
        checkpoint = log.length
        expect(await deleteEndpoint(request, endpointId)).toBe(true)
        await expectRefetchAfter('endpoints', OVERVIEW)
        await expectRefetchAfter('endpoints', REQUEST_RESPONSES)

        // The page clock never advanced, so no polling interval fired in between.
        expect(await page.evaluate(() => Date.now())).toBe(pausedAt)
        expect(pageErrors).toEqual([])
      } finally {
        await deleteEndpointsByName(request, endpointName)
        await mock.close()
      }
    })
  })
}
