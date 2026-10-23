import { randomUUID } from 'node:crypto'
import { writeFile } from 'node:fs/promises'
import { test, expect, type Locator, type Page } from '@playwright/test'
import { createUser, deleteEndpoint, deleteUser, ensureDashboardLogin } from '../../helpers/api-helpers'
import { startMockOpenAIEndpointServer } from '../../helpers/mock-openai-endpoint'

const headers = { Authorization: 'Bearer sk_debug', 'anthropic-version': '2023-06-01' }

function watchErrors(page: Page): string[] {
  const errors: string[] = []
  page.on('pageerror', (error) => errors.push(error.message))
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push(message.text())
  })
  return errors
}

async function observeActivityChange(indicator: Locator, active: number): Promise<() => Promise<number>> {
  await indicator.evaluate((element, expected) => {
    let observer: MutationObserver
    const changed = new Promise<number>((resolve, reject) => {
      observer = new MutationObserver(() => {
        if (element.getAttribute('data-active-requests') !== String(expected)) return
        observer.disconnect()
        resolve(Date.now())
      })
      observer.observe(element, { attributes: true, attributeFilter: ['data-active-requests'] })
      if (!element.isConnected) reject(new Error('Activity indicator is detached'))
    })
    Object.assign(element, { activityObservation: { changed, observer: observer! } })
  }, active)
  return () => indicator.evaluate(async (element) => {
    const { changed, observer } = (element as HTMLElement & {
      activityObservation: { changed: Promise<number>; observer: MutationObserver }
    }).activityObservation
    const deadline = AbortSignal.timeout(2000)
    let onExpired: () => void
    const expired = new Promise<number>((_, reject) => {
      onExpired = () => reject(new Error('Activity did not change within two seconds'))
      deadline.addEventListener('abort', onExpired, { once: true })
    })
    try { return await Promise.race([changed, expired]) }
    finally {
      observer.disconnect()
      deadline.removeEventListener('abort', onExpired!)
    }
  })
}

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Request activity (${colorScheme}) @dashboard`, () => {
    test.use({ colorScheme, viewport: { width: 1600, height: 1100 } })

    test('shows real streaming starts and finishes within two seconds with 60 second series', async ({ page, request }, testInfo) => {
      test.setTimeout(240_000)
      const name = `e2e-activity-${randomUUID()}`
      let releaseResponse!: () => void
      let requestStarted!: () => void
      let held: Promise<void>
      let started: Promise<void>
      const mock = await startMockOpenAIEndpointServer({
        models: [name],
        afterFirstChatChunk: async () => {
          requestStarted()
          await held
        },
      })
      let endpointId: string | undefined
      try {
        const created = await request.post('/api/endpoints', {
          headers, data: { name, base_url: mock.baseUrl },
        })
        expect(created.ok()).toBe(true)
        endpointId = (await created.json()).id
        for (const action of ['test', 'sync']) {
          expect((await request.post(`/api/endpoints/${endpointId}/${action}`, { headers })).ok()).toBe(true)
        }
        expect((await request.post('/api/auth/login', {
          data: { username: 'admin', password: 'test' },
        })).ok()).toBe(true)
        expect((await page.context().request.post('/api/auth/login', {
          data: { username: 'admin', password: 'test' },
        })).ok()).toBe(true)
        const errors = watchErrors(page)
        const metricsFrames: Array<{ receivedAt: number; keys: string[] }> = []
        page.on('websocket', (socket) => {
          if (!socket.url().includes('/ws/dashboard')) return
          socket.on('framereceived', ({ payload }) => {
            const frame = JSON.parse(payload.toString())
            if (frame.changed === 'metrics' && frame.id === endpointId) {
              metricsFrames.push({ receivedAt: Date.now(), keys: Object.keys(frame).sort() })
            }
          })
        })
        await ensureDashboardLogin(page)
        if (colorScheme === 'dark') await expect(page.locator('html')).toHaveClass(/\bdark\b/)
        else await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)
        await page.getByPlaceholder('Search by name or URL...').fill(name)
        const row = page.getByRole('row').filter({ hasText: name })
        const endpointActivity = row.getByTestId('request-activity')
        const totalActivity = page.getByRole('region', { name: 'Request activity', exact: true })
        await expect(endpointActivity).toHaveAttribute('data-state', 'idle')
        await expect(page.getByTestId('activity-connection-status')).toContainText('Live')
        await expect(page.getByRole('columnheader', { name: 'Activity', exact: true })).toBeVisible()
        await expect(page.getByRole('columnheader', { name: 'Req/s · 60s', exact: true })).toBeVisible()
        await expect(page.getByLabel('Activity legend')).toBeVisible()
        await expect(page.getByTestId('operations-tiles').locator(':scope > *')).toHaveCount(6)

        const latencies: Array<{ api: string; startLatency: number; finishLatency: number; upstreamDrainMs: number; releaseToVisibleMs: number; visibility: string; focused: boolean }> = []
        for (let sample = 0; sample < 20; sample++) {
          held = new Promise<void>((resolve) => { releaseResponse = resolve })
          started = new Promise<void>((resolve) => { requestStarted = resolve })
          const startVisibleAt = await observeActivityChange(endpointActivity, 1)
          const submittedAt = Date.now()
          const isAnthropic = sample % 2 === 1
          const completion = request.post(isAnthropic ? '/v1/messages' : '/v1/chat/completions', {
            headers,
            timeout: 90_000,
            data: { model: name, stream: true, max_tokens: 16, messages: [{ role: 'user', content: 'held activity stream' }] },
          })
          try {
            await Promise.race([
              started,
              completion.then((response) => { throw new Error(`Stream ended before holding: ${response.status()}`) }),
            ])
            const startLatency = (await startVisibleAt()) - submittedAt
            await expect(endpointActivity).toHaveAttribute('data-state', 'busy')
            await expect(totalActivity.getByTestId('request-activity')).toHaveAttribute('data-state', 'busy')
            await expect.poll(() => metricsFrames.filter((frame) => frame.receivedAt >= submittedAt).length, {
              timeout: Math.max(1, 2000 - (Date.now() - submittedAt)),
            }).toBeGreaterThan(0)
            expect(metricsFrames.every((frame) => frame.keys.join(',') === 'changed,id')).toBe(true)

            const activityResponse = await request.get('/api/dashboard/activity')
            expect(activityResponse.ok()).toBe(true)
            const activity = await activityResponse.json()
            expect(activity.bucket_seconds).toBe(1)
            expect(activity.window_seconds).toBe(60)
            expect(activity.total.requests_per_second).toHaveLength(60)
            const endpoint = activity.endpoints.find((item: { endpoint_id: string }) => item.endpoint_id === endpointId)
            expect(endpoint.active_requests).toBe(1)
            expect(endpoint.queued_requests).toBe(0)
            expect(endpoint.requests_per_second).toHaveLength(60)
            expect(endpoint.requests_per_second.reduce((sum: number, value: number) => sum + value, 0)).toBeGreaterThan(0)
            await expect(row.getByRole('img')).toHaveAttribute('data-sample-count', '60')
            await expect(totalActivity.getByRole('img')).toHaveAttribute('data-sample-count', '60')
            if (sample < 2) {
              // The upstream already sent an SSE chunk. Its body must keep the
              // lease alive throughout a long stream, beyond header receipt.
              const streamingAt = Date.now()
              await expect.poll(() => Date.now() - streamingAt, { timeout: 35_000, intervals: [500] }).toBeGreaterThanOrEqual(30_000)
              await expect(endpointActivity).toHaveAttribute('data-active-requests', '1')
              await expect(totalActivity.getByTestId('activity-area')).toBeVisible()
              await expect(totalActivity.getByTestId('activity-latest-point')).toBeVisible()
              const graphInspector = totalActivity.getByRole('slider', { name: 'Inspect request rate' })
              await graphInspector.focus()
              await graphInspector.press('ArrowLeft')
              await expect(totalActivity.getByRole('tooltip')).toBeVisible()
              await expect(row.getByTestId('endpoint-activity-dot')).toHaveAttribute('data-state', 'busy')
              const screenshotName = `busy-${isAnthropic ? 'anthropic' : 'openai'}-${colorScheme}`
              const screenshotPath = testInfo.outputPath(`${screenshotName}.png`)
              await page.evaluate(() => window.scrollTo(0, 0))
              await page.screenshot({ path: screenshotPath, fullPage: true })
              await testInfo.attach(screenshotName, { path: screenshotPath, contentType: 'image/png' })
            }

            await page.emulateMedia({ reducedMotion: 'reduce' })
            await expect.poll(() => endpointActivity.evaluate((element) =>
              element.getAnimations({ subtree: true }).filter((animation) => animation.playState === 'running').length,
            )).toBe(0)
            await expect(endpointActivity).toHaveAttribute('data-state', 'busy')
            await expect.poll(() => page.getByTestId('operations-tiles').evaluate((element) =>
              element.getAnimations({ subtree: true }).filter((animation) => animation.playState === 'running').length,
            )).toBe(0)
            const finishVisibleAt = await observeActivityChange(endpointActivity, 0)
            const releasedAt = Date.now()
            releaseResponse()
            const response = await completion
            const completedAt = Date.now()
            const idleVisibleAt = await finishVisibleAt()
            // Releasing the upstream starts its remaining body; the client's
            // completed HTTP response is the request-completion boundary.
            const finishLatency = Math.max(0, idleVisibleAt - completedAt)
            expect(response.ok()).toBe(true)
            expect(await response.text()).toContain(isAnthropic ? 'message_stop' : '[DONE]')
            await expect(endpointActivity).toHaveAttribute('data-active-requests', '0')
            await expect(endpointActivity).toHaveAttribute('data-state', 'idle')
            const visibility = await page.evaluate(() => ({ visibility: document.visibilityState, focused: document.hasFocus() }))
            latencies.push({ api: isAnthropic ? 'anthropic' : 'openai', startLatency, finishLatency,
              upstreamDrainMs: completedAt - releasedAt, releaseToVisibleMs: idleVisibleAt - releasedAt, ...visibility })
            expect(errors).toEqual([])
          } finally {
            releaseResponse()
            await completion
          }
        }
        const p95 = (values: number[]) => [...values].sort((a, b) => a - b)[Math.ceil(values.length * 0.95) - 1]
        const startP95 = p95(latencies.map((sample) => sample.startLatency))
        const finishP95 = p95(latencies.map((sample) => sample.finishLatency))
        const latencyPath = testInfo.outputPath('activity-latency.json')
        await writeFile(latencyPath, JSON.stringify({ colorScheme, latencies, startP95, finishP95 }, null, 2))
        await testInfo.attach('activity-latency', { path: latencyPath, contentType: 'application/json' })
        expect(startP95).toBeLessThan(2000)
        expect(finishP95).toBeLessThan(2000)
      } finally {
        await page.close()
        if (endpointId) expect(await deleteEndpoint(request, endpointId)).toBe(true)
        await mock.close()
      }
    })

    for (const mode of ['disconnected', 'viewer'] as const) {
      test(`refreshes once per second through ${mode} polling`, async ({ page, request }) => {
        test.setTimeout(60_000)
        expect((await request.post('/api/auth/login', {
          data: { username: 'admin', password: 'test' },
        })).ok()).toBe(true)
        let viewerId: string | undefined
        let socketConnections = 0
        try {
          let pageCredentials = { username: 'admin', password: 'test' }
          if (mode === 'viewer') {
            const viewer = await createUser(request, `e2e-activity-viewer-${randomUUID()}@example.com`, '', 'viewer')
            viewerId = viewer.id
            expect(viewerId).toBeTruthy()
            const viewerLogin = await page.context().request.post('/api/auth/login', {
              data: { username: viewer.username, password: viewer.generated_password },
            })
            expect(viewerLogin.ok()).toBe(true)
            const password = 'ActivityViewerPass123!'
            expect((await page.context().request.put('/api/auth/change-password', {
              headers: { Authorization: `Bearer ${(await viewerLogin.json()).token}` },
              data: { current_password: viewer.generated_password, new_password: password },
            })).ok()).toBe(true)
            pageCredentials = { username: viewer.username, password }
          }
          expect((await page.context().request.post('/api/auth/login', {
            data: pageCredentials,
          })).ok()).toBe(true)
          const identity = await page.context().request.get('/api/auth/me')
          expect(identity.ok()).toBe(true)
          expect((await identity.json()).role).toBe(mode === 'viewer' ? 'viewer' : 'admin')
          const errors = watchErrors(page)
          await page.routeWebSocket('**/ws/dashboard', async (socket) => {
            socketConnections++
            await socket.close({ code: 1000, reason: 'Verify one second fallback polling' })
          })
          let active = 0
          let activityReads = 0
          await page.route('**/api/dashboard/activity', (route) => {
            activityReads++
            return route.fulfill({ json: {
              timestamp: new Date().toISOString(), bucket_seconds: 1, window_seconds: 60,
              total: { active_requests: active, queued_requests: active ? 2 : 0, requests_per_second: Array.from({ length: 60 }, (_, index) => index === 59 ? active : 0) },
              endpoints: [],
            } })
          })
          await ensureDashboardLogin(page)
          const section = page.getByRole('region', { name: 'Request activity', exact: true })
          const indicator = section.getByTestId('request-activity')
          await expect(indicator).toHaveAttribute('data-active-requests', '0')
          await expect(page.getByTestId('activity-connection-status')).toContainText(/Polling.*1s.*WebSocket/i)
          const readsBefore = activityReads
          const changedAt = Date.now()
          active = 3
          await expect(indicator).toHaveAttribute('data-active-requests', '3', { timeout: 2000 })
          expect(Date.now() - changedAt).toBeLessThan(2000)
          await expect(indicator).toHaveAttribute('data-queued-requests', '2')
          expect(activityReads).toBeGreaterThan(readsBefore)
          if (mode === 'viewer') expect(socketConnections).toBe(0)
          expect(errors).toEqual([])
        } finally {
          await page.close()
          if (viewerId) expect(await deleteUser(request, viewerId)).toBe(true)
        }
      })
    }
  })
}
