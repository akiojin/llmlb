import { test, expect, errors, type WebSocketRoute } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'

const ENDPOINT_A = '11111111-2222-3333-4444-555555555555'
const ENDPOINT_B = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee'
const MODELS = ['alpha-chat', 'bravo-chat']

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Models Table ViewModel (${colorScheme}) @dashboard`, () => {
    test.use({ colorScheme })

    test('preserves controls, scoped TPS updates and polling until details close (#856)', async ({ page }, testInfo) => {
      const pageErrors: string[] = []
      page.on('pageerror', (error) => pageErrors.push(error.message))
      page.on('console', (message) => {
        if (message.type() === 'error') pageErrors.push(message.text())
      })

      // Keep real authentication. Controlled model data must not hide a 401
      // or turn a failed session probe into an ignored console error.
      const login = await page.request.post('/api/auth/login', {
        data: { username: 'admin', password: 'test' },
      })
      expect(login.ok()).toBe(true)

      let dashboardSocket: WebSocketRoute | undefined
      await page.routeWebSocket('**/ws/dashboard', (socket) => {
        dashboardSocket = socket
      })

      const endpoints = [ENDPOINT_A, ENDPOINT_B].map((id, index) => ({
        id,
        name: `viewmodel-endpoint-${index + 1}`,
        base_url: `http://192.0.2.${index + 1}:8080`,
        status: 'online',
        endpoint_type: 'xllm',
        health_check_interval_secs: 30,
        inference_timeout_secs: 120,
        error_count: 0,
        registered_at: '2026-10-01T00:00:00Z',
        model_count: 1,
        total_requests: 0,
        successful_requests: 0,
        failed_requests: 0,
      }))
      await page.route('**/api/dashboard/endpoints', (route) => route.fulfill({ json: endpoints }))
      await page.route('**/api/dashboard/models*', (route) => route.fulfill({ json: {
        object: 'list',
        data: MODELS.map((id, index) => ({
          id, object: 'model', created: 0, owned_by: 'load balancer',
          lifecycle_status: 'registered', ready: true, supported_apis: ['chat_completions'],
          tags: [], aliases: [], endpoint_ids: [endpoints[index].id],
        })),
      } }))

      let allStatsRequests = 0
      const allStatsTotals: Record<string, number> = { [MODELS[0]]: 8, [MODELS[1]]: 21 }
      await page.route('**/api/dashboard/model-stats', (route) => {
        allStatsRequests++
        return route.fulfill({ json: MODELS.map((model_id) => ({
          model_id, total_requests: allStatsTotals[model_id],
          successful_requests: allStatsTotals[model_id] - 1, failed_requests: 1,
        })) })
      })

      const statsRequests: Record<string, number> = { [ENDPOINT_A]: 0, [ENDPOINT_B]: 0 }
      const statsTotals: Record<string, number> = { [ENDPOINT_A]: 12, [ENDPOINT_B]: 12 }
      const tpsRequests: Record<string, number> = { [ENDPOINT_A]: 0, [ENDPOINT_B]: 0 }
      const tpsValues: Record<string, number> = { [ENDPOINT_A]: 42.5, [ENDPOINT_B]: 7 }
      await page.route('**/api/endpoints/*/model-stats', (route) => {
        const id = new URL(route.request().url()).pathname.split('/')[3]
        const index = endpoints.findIndex((endpoint) => endpoint.id === id)
        expect(index).toBeGreaterThanOrEqual(0)
        statsRequests[id]++
        return route.fulfill({ json: [{
          model_id: MODELS[index], total_requests: statsTotals[id],
          successful_requests: statsTotals[id] - 2, failed_requests: 2,
        }] })
      })
      await page.route('**/api/endpoints/*/model-tps', (route) => {
        const id = new URL(route.request().url()).pathname.split('/')[3]
        const index = endpoints.findIndex((endpoint) => endpoint.id === id)
        expect(index).toBeGreaterThanOrEqual(0)
        tpsRequests[id]++
        return route.fulfill({ json: [{
          model_id: MODELS[index], api_kind: 'chat_completions', source: 'production',
          tps: tpsValues[id], request_count: 3, total_output_tokens: 300, average_duration_ms: 100,
        }] })
      })

      await page.clock.install()
      await ensureDashboardLogin(page)
      await expect.poll(() => Boolean(dashboardSocket)).toBe(true)
      await page.getByRole('tab', { name: 'Models', exact: true }).click()
      const table = page.getByRole('tabpanel', { name: 'Models', exact: true }).getByRole('table')
      const identities = table.locator('[data-model-id]')
      await expect(identities).toHaveCount(2)
      const rowFor = (id: string) => table.getByRole('row').filter({ has: page.locator(`[data-model-id="${id}"]`) })
      const alphaRow = rowFor(MODELS[0])
      const bravoRow = rowFor(MODELS[1])
      await expect(alphaRow.getByRole('cell', { name: '8', exact: true })).toBeVisible()
      await expect(bravoRow.getByRole('cell', { name: '21', exact: true })).toBeVisible()
      expect(allStatsRequests).toBe(1)
      expect(statsRequests).toEqual({ [ENDPOINT_A]: 0, [ENDPOINT_B]: 0 })
      expect(tpsRequests).toEqual({ [ENDPOINT_A]: 0, [ENDPOINT_B]: 0 })
      if (colorScheme === 'dark') {
        await expect(page.locator('html')).toHaveClass(/\bdark\b/)
      } else {
        await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)
      }

      // Exclude the existing 5s provider polling from controls and WS updates.
      await page.clock.pauseAt(Date.now() + 1000)
      const pausedAt = await page.evaluate(() => Date.now())
      const search = page.getByPlaceholder('Search by model ID...')
      await search.fill('BRAVO')
      await expect(identities).toHaveCount(1)
      await expect(identities).toHaveAttribute('data-model-id', MODELS[1])
      await search.fill('no-such-model')
      await expect(table.getByText('No models match the filter criteria')).toBeVisible()
      await search.clear()

      await expect(identities).toHaveCount(2)
      await expect(identities).toHaveText(MODELS)
      await table.getByRole('columnheader', { name: 'Model ID', exact: true }).click()
      await expect(identities).toHaveText([...MODELS].reverse())
      await table.getByRole('columnheader', { name: 'Routed Requests', exact: true }).click()
      await expect(identities).toHaveText(MODELS)

      await alphaRow.getByRole('button', { name: 'Expand row', exact: true }).click()
      await bravoRow.getByRole('button', { name: 'Expand row', exact: true }).click()
      // Deliver only React Query's queued renders, staying below polling.
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return {
          alphaTps: await table.getByText('TPS: chat 42.5 tok/s', { exact: true }).count(),
          bravoTps: await table.getByText('TPS: chat 7.0 tok/s', { exact: true }).count(),
          stats: await table.getByText('Total: 12', { exact: true }).count(),
        }
      }).toEqual({ alphaTps: 1, bravoTps: 1, stats: 2 })
      await expect(table.getByText('TPS: chat 7.0 tok/s', { exact: true })).toBeVisible()
      await expect(table.getByText('Total: 12', { exact: true })).toHaveCount(2)
      await expect(table.getByText('OK: 10', { exact: true })).toHaveCount(2)
      await expect(table.getByText('Fail: 2', { exact: true })).toHaveCount(2)
      await expect(table.locator(`a[href="#playground/${ENDPOINT_A}"]`)).toBeVisible()
      expect(statsRequests).toEqual({ [ENDPOINT_A]: 1, [ENDPOINT_B]: 1 })
      expect(tpsRequests).toEqual({ [ENDPOINT_A]: 1, [ENDPOINT_B]: 1 })

      // Change both server responses, then notify A. Mounted B must retain its
      // cached value and make no request: a broad invalidation would fail this.
      tpsValues[ENDPOINT_A] = 12
      tpsValues[ENDPOINT_B] = 99
      const unrelatedRefresh = page.waitForRequest(
        (request) => new URL(request.url()).pathname === `/api/endpoints/${ENDPOINT_B}/model-tps`,
        { timeout: 500 },
      ).then(
        () => true,
        (error: unknown) => {
          if (error instanceof errors.TimeoutError) return false
          throw error
        },
      )
      dashboardSocket!.send(JSON.stringify({ changed: 'tps', id: ENDPOINT_A }))
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return table.getByText('TPS: chat 12.0 tok/s', { exact: true }).count()
      }).toBe(1)
      expect(await unrelatedRefresh).toBe(false)
      await expect(table.getByText('TPS: chat 7.0 tok/s', { exact: true })).toBeVisible()
      await expect(table.getByText('TPS: chat 99.0 tok/s', { exact: true })).toHaveCount(0)
      expect(tpsRequests).toEqual({ [ENDPOINT_A]: 2, [ENDPOINT_B]: 1 })
      // A TPS notification must not add new stats invalidation or polling.
      expect(allStatsRequests).toBe(1)
      expect(statsRequests).toEqual({ [ENDPOINT_A]: 1, [ENDPOINT_B]: 1 })
      const advancedMs = await page.evaluate(() => Date.now()) - pausedAt
      expect(advancedMs).toBeGreaterThan(0)
      expect(advancedMs).toBeLessThan(1000)

      // No stats notification is sent. The unchanged 5s provider interval
      // must still refresh both aggregate and expanded-endpoint statistics.
      allStatsTotals[MODELS[0]] = 31
      allStatsTotals[MODELS[1]] = 44
      statsTotals[ENDPOINT_A] = 19
      statsTotals[ENDPOINT_B] = 23
      await page.clock.runFor(5000)
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return {
          allStatsRequests,
          statsRequests: { ...statsRequests },
          alphaTotal: await alphaRow.getByRole('cell', { name: '31', exact: true }).count(),
          bravoTotal: await bravoRow.getByRole('cell', { name: '44', exact: true }).count(),
          alphaDetails: await table.getByText('Total: 19', { exact: true }).count(),
          bravoDetails: await table.getByText('Total: 23', { exact: true }).count(),
        }
      }).toEqual({
        allStatsRequests: 2,
        statsRequests: { [ENDPOINT_A]: 2, [ENDPOINT_B]: 2 },
        alphaTotal: 1, bravoTotal: 1, alphaDetails: 1, bravoDetails: 1,
      })
      await expect(table.getByText('TPS: chat 99.0 tok/s', { exact: true })).toBeVisible()
      expect(tpsRequests).toEqual({ [ENDPOINT_A]: 3, [ENDPOINT_B]: 2 })

      // Closing A removes its query observers. Another 5s interval must
      // refresh the still-expanded B, without keeping A's polling alive.
      await alphaRow.getByRole('button', { name: 'Collapse row', exact: true }).click()
      await expect(table.getByText('viewmodel-endpoint-1', { exact: true })).toHaveCount(0)
      await expect(table.getByText('viewmodel-endpoint-2', { exact: true })).toBeVisible()
      allStatsTotals[MODELS[0]] = 51
      allStatsTotals[MODELS[1]] = 62
      statsTotals[ENDPOINT_A] = 70
      statsTotals[ENDPOINT_B] = 41
      tpsValues[ENDPOINT_A] = 100
      tpsValues[ENDPOINT_B] = 19
      await page.clock.runFor(5000)
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return {
          allStatsRequests,
          statsRequests: { ...statsRequests },
          tpsRequests: { ...tpsRequests },
          alphaTotal: await alphaRow.getByRole('cell', { name: '51', exact: true }).count(),
          bravoTotal: await bravoRow.getByRole('cell', { name: '62', exact: true }).count(),
          bravoDetails: await table.getByText('Total: 41', { exact: true }).count(),
          bravoTps: await table.getByText('TPS: chat 19.0 tok/s', { exact: true }).count(),
        }
      }).toEqual({
        allStatsRequests: 3,
        statsRequests: { [ENDPOINT_A]: 2, [ENDPOINT_B]: 3 },
        tpsRequests: { [ENDPOINT_A]: 3, [ENDPOINT_B]: 3 },
        alphaTotal: 1, bravoTotal: 1, bravoDetails: 1, bravoTps: 1,
      })
      await expect(alphaRow.getByRole('button', { name: 'Expand row', exact: true })).toBeVisible()
      await expect(bravoRow.getByRole('button', { name: 'Collapse row', exact: true })).toBeVisible()
      expect(pageErrors).toEqual([])
      await testInfo.attach(`models-table-${colorScheme}`, {
        body: await page.screenshot(),
        contentType: 'image/png',
      })
    })
  })
}
