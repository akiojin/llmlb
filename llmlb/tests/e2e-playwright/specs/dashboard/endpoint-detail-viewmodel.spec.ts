import { test, expect, type WebSocketRoute } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'
import type { DashboardEndpoint, ModelTpsEntry } from '../../../../src/web/dashboard/src/lib/api/endpoints'

const ENDPOINT_A = '11111111-2222-3333-4444-555555555555'
const ENDPOINT_B = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee'

function endpoint(id: string, name: string, overrides: Partial<DashboardEndpoint> = {}): DashboardEndpoint {
  return {
    id, name, base_url: `http://192.0.2.${id === ENDPOINT_A ? 1 : 2}:8080`,
    status: 'online', endpoint_type: 'xllm', health_check_interval_secs: 30,
    inference_timeout_secs: 600, error_count: 0,
    registered_at: '2026-10-01T00:00:00Z', model_count: 2,
    total_requests: 100, successful_requests: 95, failed_requests: 5,
    latency_ms: 42, notes: 'alpha notes', ...overrides,
  }
}

function tps(model_id: string, values: Partial<ModelTpsEntry> = {}): ModelTpsEntry {
  return {
    model_id, api_kind: 'chat_completions', source: 'production',
    tps: 12.3, request_count: 2, total_output_tokens: 246,
    average_duration_ms: 250, ...values,
  }
}

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Endpoint Detail ViewModels (${colorScheme}) @dashboard`, () => {
    test.use({ colorScheme })

    test('preserves detail commands, query timing, model consolidation and chart periods (#864)', async ({ page }, testInfo) => {
      test.setTimeout(60000)
      const pageErrors: string[] = []
      page.on('pageerror', (error) => pageErrors.push(error.message))
      page.on('console', (message) => {
        if (message.type() === 'error') pageErrors.push(message.text())
      })

      // Authentication and its session probe use the real server. Fixtures
      // control only endpoint data; the test never replaces the auth boundary.
      const login = await page.request.post('/api/auth/login', {
        data: { username: 'admin', password: 'test' },
      })
      expect(login.ok()).toBe(true)
      let dashboardSocket: WebSocketRoute | undefined
      await page.routeWebSocket('**/ws/dashboard', (socket) => { dashboardSocket = socket })

      let endpoints = [
        endpoint(ENDPOINT_A, 'alpha-endpoint'),
        endpoint(ENDPOINT_B, 'beta-endpoint', {
          status: 'offline', endpoint_type: 'ollama', health_check_interval_secs: 60,
          notes: 'beta notes', model_count: 0, total_requests: 0,
          successful_requests: 0, failed_requests: 0,
        }),
      ]
      let listRequests = 0
      await page.route('**/api/dashboard/endpoints', (route) => {
        expect(route.request().method()).toBe('GET')
        listRequests++
        return route.fulfill({ json: endpoints })
      })

      const requests = Object.fromEntries([ENDPOINT_A, ENDPOINT_B].map((id) => [id, {
        today: 0, models: 0, stats: 0, tps: 0, daily: [] as number[],
      }]))
      let todayTotal = 12
      let modelSuccesses = 92
      let modelTps = [
        tps('alpha-chat'),
        tps('alpha-chat', { api_kind: 'completions', tps: 24.6, request_count: 9, average_duration_ms: 1250 }),
        tps('legacy-model', { tps: null, request_count: 0, average_duration_ms: null }),
      ]

      for (const id of [ENDPOINT_A, ENDPOINT_B]) {
        await page.route(`**/api/endpoints/${id}/today-stats`, (route) => {
          expect(route.request().method()).toBe('GET')
          requests[id].today++
          return route.fulfill({ json: {
            date: '2026-10-01', total_requests: id === ENDPOINT_A ? todayTotal : 0,
            successful_requests: id === ENDPOINT_A ? todayTotal - 1 : 0,
            failed_requests: id === ENDPOINT_A ? 1 : 0,
          } })
        })
        await page.route(`**/api/endpoints/${id}/models`, (route) => {
          expect(route.request().method()).toBe('GET')
          requests[id].models++
          return route.fulfill({ json: {
            endpoint_id: id,
            models: id === ENDPOINT_A ? [
              { model_id: 'alpha-chat', canonical_name: 'alpha-chat', max_tokens: 8192 },
              { model_id: 'idle-model', max_tokens: 0 },
            ] : [],
          } })
        })
        await page.route(`**/api/endpoints/${id}/model-stats`, (route) => {
          expect(route.request().method()).toBe('GET')
          requests[id].stats++
          return route.fulfill({ json: id === ENDPOINT_A ? [
            { model_id: 'alpha-chat', total_requests: 100, successful_requests: modelSuccesses, failed_requests: 100 - modelSuccesses },
            { model_id: 'historical-model', total_requests: 10, successful_requests: 7, failed_requests: 3 },
          ] : [] })
        })
        await page.route(`**/api/endpoints/${id}/model-tps`, (route) => {
          expect(route.request().method()).toBe('GET')
          requests[id].tps++
          return route.fulfill({ json: id === ENDPOINT_A ? modelTps : [] })
        })
        await page.route(`**/api/endpoints/${id}/daily-stats*`, (route) => {
          expect(route.request().method()).toBe('GET')
          const days = Number(new URL(route.request().url()).searchParams.get('days'))
          expect([7, 30, 90]).toContain(days)
          requests[id].daily.push(days)
          return route.fulfill({ json: id === ENDPOINT_A ? [{
            date: days === 7 ? '2026-10-01' : days === 30 ? '2026-09-01' : '2026-07-01',
            total_requests: days + 1, successful_requests: days, failed_requests: 1,
          }] : [] })
        })
      }

      let detailGets = 0
      const savePayloads: unknown[] = []
      await page.route(`**/api/endpoints/${ENDPOINT_A}`, (route) => {
        // The modal consumes its selected endpoint snapshot; fetching the
        // endpoint again would change the acquisition contract of T012a.
        if (route.request().method() === 'GET') {
          detailGets++
          return route.fulfill({ json: endpoints[0] })
        }
        expect(route.request().method()).toBe('PUT')
        expect(route.request().headers()['x-csrf-token']).toBeTruthy()
        const payload = route.request().postDataJSON()
        savePayloads.push(payload)
        endpoints = endpoints.map((item) => item.id === ENDPOINT_A ? { ...item, ...payload } : item)
        return route.fulfill({ json: endpoints.find((item) => item.id === ENDPOINT_A) })
      })

      let releaseTest!: () => void
      const testReady = new Promise<void>((resolve) => { releaseTest = resolve })
      let testRequests = 0
      await page.route(`**/api/endpoints/${ENDPOINT_A}/test`, async (route) => {
        expect(route.request().method()).toBe('POST')
        expect(route.request().headers()['x-csrf-token']).toBeTruthy()
        testRequests++
        await testReady
        return route.fulfill({ json: { success: true, latency_ms: 87 } })
      })
      let syncRequests = 0
      await page.route(`**/api/endpoints/${ENDPOINT_A}/sync`, (route) => {
        expect(route.request().method()).toBe('POST')
        expect(route.request().headers()['x-csrf-token']).toBeTruthy()
        syncRequests++
        return route.fulfill({ json: { synced_models: 4 } })
      })

      // Keep polling deterministic while asserting command-triggered refresh.
      await page.clock.install()
      await page.clock.pauseAt(Date.now() + 1000)
      await ensureDashboardLogin(page)
      await expect.poll(() => Boolean(dashboardSocket)).toBe(true)
      const panel = page.getByRole('tabpanel', { name: 'Endpoints', exact: true })
      const endpointTable = panel.getByRole('table')
      const rowFor = (name: string) => endpointTable.getByRole('row').filter({
        has: page.getByRole('cell', { name, exact: true }),
      })
      await expect(rowFor('alpha-endpoint')).toBeVisible()
      if (colorScheme === 'dark') await expect(page.locator('html')).toHaveClass(/\bdark\b/)
      else await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)

      // No detail query starts while the modal is closed, even as time passes.
      await page.clock.runFor(5000)
      expect(requests[ENDPOINT_A]).toEqual({ today: 0, models: 0, stats: 0, tps: 0, daily: [] })
      expect(requests[ENDPOINT_B]).toEqual({ today: 0, models: 0, stats: 0, tps: 0, daily: [] })
      const openedAt = await page.evaluate(() => Date.now())
      await rowFor('alpha-endpoint').getByRole('button', { name: 'Details', exact: true }).click()
      const dialog = page.getByRole('dialog', { name: 'alpha-endpoint', exact: true })
      await expect(dialog).toBeVisible()
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return {
          requests: requests[ENDPOINT_A],
          today: await dialog.getByText('12', { exact: true }).count(),
          models: await dialog.getByText('24.6 tok/s', { exact: true }).count(),
          chart: await dialog.getByText('10/01', { exact: true }).count(),
        }
      }).toEqual({
        requests: { today: 1, models: 1, stats: 1, tps: 1, daily: [7] },
        today: 1, models: 1, chart: 1,
      })
      await expect(dialog.getByText('Today', { exact: true }).locator('..').locator('..')).toContainText('12')
      await expect(dialog.getByText('Success Rate', { exact: true }).locator('..').locator('..')).toContainText('95.0%')
      await expect(dialog.getByLabel('Display Name', { exact: true })).toHaveValue('alpha-endpoint')
      await expect(dialog.getByLabel('Notes', { exact: true })).toHaveValue('alpha notes')

      const modelsTable = dialog.getByRole('table')
      const modelRow = (id: string) => modelsTable.getByRole('row').filter({
        has: page.locator(`[data-model-id="${id}"]`),
      })
      await expect(dialog.getByText('Models (4)', { exact: true })).toBeVisible()
      await expect(modelsTable.locator('tbody tr')).toHaveCount(4)
      await expect(modelRow('alpha-chat').locator('td').nth(1)).toHaveText('8K')
      await expect(modelRow('alpha-chat').locator('td').nth(2)).toHaveText('24.6 tok/s')
      await expect(modelRow('alpha-chat').locator('td').nth(3)).toHaveText('9')
      await expect(modelRow('alpha-chat').locator('td').nth(4)).toHaveText('92.0%')
      await expect(modelRow('alpha-chat').locator('td').nth(5)).toHaveText('1.3 s')
      await expect(modelRow('alpha-chat').locator('[data-model-canonical-badge]')).toHaveText('canonical')
      await expect(modelRow('idle-model').locator('td').nth(1)).toHaveText('—')
      await expect(modelRow('idle-model').locator('td').nth(2)).toHaveText('—')
      await expect(modelRow('idle-model').locator('td').nth(4)).toHaveText('-')
      await expect(modelRow('legacy-model').locator('td').nth(5)).toHaveText('—')
      await expect(modelRow('historical-model').locator('td').nth(4)).toHaveText('70.0%')

      await expect(dialog.getByText('10/01', { exact: true })).toBeVisible()
      await dialog.getByRole('tab', { name: '30D', exact: true }).click()
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return {
          requests: requests[ENDPOINT_A].daily,
          labels: await dialog.getByText('09/01', { exact: true }).count(),
        }
      }).toEqual({ requests: [7, 30], labels: 1 })
      await expect(dialog.getByText('09/01', { exact: true })).toBeVisible()
      await dialog.getByRole('tab', { name: '90D', exact: true }).click()
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return {
          requests: requests[ENDPOINT_A].daily,
          labels: await dialog.getByText('07/01', { exact: true }).count(),
        }
      }).toEqual({ requests: [7, 30, 90], labels: 1 })
      await expect(dialog.getByText('07/01', { exact: true })).toBeVisible()
      await dialog.getByRole('tab', { name: '7D', exact: true }).click()
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return dialog.getByText('10/01', { exact: true }).count()
      }).toBe(1)
      await expect(dialog.getByText('10/01', { exact: true })).toBeVisible()

      // Today/model/stat/chart queries inherit the five-second provider
      // interval, while the composed TPS query keeps its explicit ten seconds.
      todayTotal = 13
      modelSuccesses = 96
      await page.clock.runFor(openedAt + 4900 - await page.evaluate(() => Date.now()))
      expect(requests[ENDPOINT_A].today).toBe(1)
      expect(requests[ENDPOINT_A].models).toBe(1)
      expect(requests[ENDPOINT_A].stats).toBe(1)
      expect(requests[ENDPOINT_A].tps).toBe(1)
      await page.clock.runFor(200)
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return {
          requests: requests[ENDPOINT_A],
          today: await dialog.getByText('13', { exact: true }).count(),
          successRate: await modelRow('alpha-chat').locator('td').nth(4).textContent(),
        }
      }).toEqual({
        requests: { today: 2, models: 2, stats: 2, tps: 1, daily: [7, 30, 90, 7] },
        today: 1, successRate: '96.0%',
      })
      await expect(dialog.getByText('Today', { exact: true }).locator('..').locator('..')).toContainText('13')
      await expect(modelRow('alpha-chat').locator('td').nth(4)).toHaveText('96.0%')
      modelTps = [tps('alpha-chat', { tps: 31.4 }), modelTps[2]]
      await page.clock.runFor(openedAt + 10100 - await page.evaluate(() => Date.now()))
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return {
          requests: requests[ENDPOINT_A].tps,
          tps: await modelRow('alpha-chat').locator('td').nth(2).textContent(),
        }
      }).toEqual({ requests: 2, tps: '31.4 tok/s' })
      await expect(modelRow('alpha-chat').locator('td').nth(2)).toHaveText('31.4 tok/s')

      // Detail TPS notifications accept the same explicit id only. Unrelated
      // endpoint data remains on its original polling schedule.
      const tpsBaseline = requests[ENDPOINT_A].tps
      dashboardSocket!.send(JSON.stringify({ changed: 'tps', id: ENDPOINT_B }))
      await page.clock.runFor(1)
      expect(requests[ENDPOINT_A].tps).toBe(tpsBaseline)
      modelTps = [tps('alpha-chat', { tps: 40 }), modelTps[1]]
      dashboardSocket!.send(JSON.stringify({ changed: 'tps', id: ENDPOINT_A }))
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return modelRow('alpha-chat').locator('td').nth(2).textContent()
      }).toBe('40.0 tok/s')
      expect(requests[ENDPOINT_A].tps).toBe(tpsBaseline + 1)

      await dialog.getByLabel('Display Name', { exact: true }).fill('alpha-saved')
      await dialog.getByLabel('Notes', { exact: true }).fill('saved notes')
      await dialog.getByLabel('Health Check Interval (sec)', { exact: true }).fill('45')
      await dialog.getByLabel('Inference Timeout (sec)', { exact: true }).fill('120')
      const beforeSave = listRequests
      await dialog.getByRole('button', { name: 'Save', exact: true }).click()
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return {
          refreshed: listRequests > beforeSave,
          ready: await dialog.getByRole('button', { name: 'Save', exact: true }).count(),
          toast: await page.getByText('Endpoint settings updated', { exact: true }).count(),
        }
      }).toEqual({ refreshed: true, ready: 1, toast: 1 })
      expect(savePayloads).toEqual([{
        name: 'alpha-saved', notes: 'saved notes',
        health_check_interval_secs: 45, inference_timeout_secs: 120,
      }])
      await expect(dialog).toBeVisible()
      await expect(dialog.getByLabel('Display Name', { exact: true })).toHaveValue('alpha-saved')

      const testButton = dialog.getByRole('button', { name: 'Test Connection', exact: true })
      const beforeTest = listRequests
      await testButton.click()
      // React Query delivers mutation state through a zero-delay timer. Flush
      // that notification while the intercepted response is still held.
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return { requests: testRequests, disabled: await testButton.isDisabled() }
      }).toEqual({ requests: 1, disabled: true })
      await expect(testButton).toBeDisabled()
      releaseTest()
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return {
          refreshed: listRequests > beforeTest, enabled: await testButton.isEnabled(),
          toast: await page.getByText('Connection Successful', { exact: true }).count(),
        }
      }).toEqual({ refreshed: true, enabled: true, toast: 1 })
      await expect(testButton).toBeEnabled()
      await expect(page.getByText('Connection Successful', { exact: true })).toBeVisible()
      const beforeSync = listRequests
      const syncButton = dialog.getByRole('button', { name: 'Sync Models', exact: true })
      await syncButton.click()
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return {
          refreshed: listRequests > beforeSync, enabled: await syncButton.isEnabled(),
          toast: await page.getByText('Synced 4 models', { exact: true }).count(),
        }
      }).toEqual({ refreshed: true, enabled: true, toast: 1 })
      expect(syncRequests).toBe(1)
      await expect(page.getByText('Synced 4 models', { exact: true })).toBeVisible()
      await testInfo.attach(`endpoint-detail-${colorScheme}`, {
        body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png',
      })

      await dialog.getByLabel('Display Name', { exact: true }).fill('unsaved draft')
      // The footer button precedes Radix's icon-only Close button.
      await dialog.getByRole('button', { name: 'Close', exact: true }).first().click()
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return dialog.count()
      }).toBe(0)
      await expect(dialog).toHaveCount(0)
      const closedRequests = structuredClone(requests[ENDPOINT_A])
      await page.clock.runFor(6000)
      expect(requests[ENDPOINT_A]).toEqual(closedRequests)
      await rowFor('beta-endpoint').getByRole('button', { name: 'Details', exact: true }).click()
      const betaDialog = page.getByRole('dialog', { name: 'beta-endpoint', exact: true })
      await expect(betaDialog.getByLabel('Display Name', { exact: true })).toHaveValue('beta-endpoint')
      await expect(betaDialog.getByLabel('Notes', { exact: true })).toHaveValue('beta notes')
      await expect(betaDialog.getByLabel('Health Check Interval (sec)', { exact: true })).toHaveValue('60')
      await expect(betaDialog.getByLabel('Inference Timeout (sec)', { exact: true })).toHaveValue('600')
      await expect(betaDialog.getByRole('button', { name: 'Sync Models', exact: true })).toBeDisabled()
      await expect(betaDialog.getByRole('button', { name: 'Open Playground', exact: true })).toBeDisabled()
      await expect(betaDialog.getByRole('button', { name: 'Download Model', exact: true })).toHaveCount(0)
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return {
          models: await betaDialog.getByText('No models available', { exact: true }).count(),
          chart: await betaDialog.getByText('No request data available', { exact: true }).count(),
        }
      }).toEqual({ models: 1, chart: 1 })
      await expect(betaDialog.getByText('No request data available', { exact: true })).toBeVisible()
      await betaDialog.getByRole('button', { name: 'Close', exact: true }).first().click()
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return betaDialog.count()
      }).toBe(0)
      await rowFor('alpha-saved').getByRole('button', { name: 'Details', exact: true }).click()
      const reopened = page.getByRole('dialog', { name: 'alpha-saved', exact: true })
      await expect(reopened.getByLabel('Display Name', { exact: true })).toHaveValue('alpha-saved')
      await expect(reopened.getByLabel('Notes', { exact: true })).toHaveValue('saved notes')
      await expect(reopened.getByLabel('Health Check Interval (sec)', { exact: true })).toHaveValue('45')
      await expect(reopened.getByLabel('Inference Timeout (sec)', { exact: true })).toHaveValue('120')
      await expect(reopened.getByRole('tab', { name: '7D', exact: true })).toHaveAttribute('data-state', 'active')
      expect(detailGets).toBe(0)
      expect(pageErrors).toEqual([])
    })
  })
}
