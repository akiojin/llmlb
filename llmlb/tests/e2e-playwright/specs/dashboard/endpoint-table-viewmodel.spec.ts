import { test, expect, type WebSocketRoute } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'
import type { DashboardEndpoint } from '../../../../src/web/dashboard/src/lib/api/endpoints'

const ENDPOINT_A = '11111111-2222-3333-4444-555555555555'
const ENDPOINT_B = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee'
const ENDPOINT_C = '99999999-8888-7777-6666-555555555555'

function endpoint(id: string, name: string, host: number): DashboardEndpoint {
  return {
    id, name, base_url: `http://192.0.2.${host}:8080`, status: 'online',
    endpoint_type: 'xllm', health_check_interval_secs: 30,
    inference_timeout_secs: 600, error_count: 0,
    registered_at: '2026-10-01T00:00:00Z', model_count: 1,
    total_requests: 0, successful_requests: 0, failed_requests: 0,
    latency_ms: 42,
  }
}

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Endpoint Table ViewModel (${colorScheme}) @dashboard`, () => {
    test.use({ colorScheme })

    test('preserves controls, mutation refresh, endpoint notifications and provider polling (#858)', async ({ page }, testInfo) => {
      const pageErrors: string[] = []
      page.on('pageerror', (error) => pageErrors.push(error.message))
      page.on('console', (message) => {
        if (message.type() === 'error') pageErrors.push(message.text())
      })

      // Authenticate against the real server; only endpoint communication is
      // controlled below. A failed session must not disappear behind a mock.
      const login = await page.request.post('/api/auth/login', {
        data: { username: 'admin', password: 'test' },
      })
      expect(login.ok()).toBe(true)

      let dashboardSocket: WebSocketRoute | undefined
      let acceptConnections = true
      await page.routeWebSocket('**/ws/dashboard', async (socket) => {
        if (acceptConnections) dashboardSocket = socket
        else await socket.close({ code: 1000, reason: 'Test disconnected polling' })
      })

      let endpoints = [endpoint(ENDPOINT_A, 'alpha-endpoint', 1), endpoint(ENDPOINT_B, 'beta-endpoint', 2)]
      let listRequests = 0
      await page.route('**/api/dashboard/endpoints', (route) => {
        expect(route.request().method()).toBe('GET')
        listRequests++
        return route.fulfill({ json: endpoints })
      })

      let testRequests = 0
      await page.route(`**/api/endpoints/${ENDPOINT_A}/test`, (route) => {
        expect(route.request().method()).toBe('POST')
        testRequests++
        // Online stays Online, so there is no status-transition WS event.
        endpoints = endpoints.map((item) => item.id === ENDPOINT_A ? { ...item, latency_ms: 75 } : item)
        return route.fulfill({ json: { success: true, latency_ms: 75 } })
      })

      let syncRequests = 0
      await page.route(`**/api/endpoints/${ENDPOINT_A}/sync`, (route) => {
        expect(route.request().method()).toBe('POST')
        syncRequests++
        endpoints = endpoints.map((item) => item.id === ENDPOINT_A ? { ...item, model_count: 4 } : item)
        return route.fulfill({ json: {
          synced_models: ['one', 'two', 'three', 'four'].map((model_id) => ({ model_id })),
          added: 3, removed: 0, updated: 0,
        } })
      })

      let createRequests = 0
      await page.route('**/api/endpoints', (route) => {
        expect(route.request().method()).toBe('POST')
        expect(route.request().postDataJSON()).toEqual({
          name: 'gamma-endpoint', base_url: 'http://192.0.2.3:8080',
          api_key: 'sk-endpoint', notes: 'viewmodel test',
        })
        createRequests++
        const created = { ...endpoint(ENDPOINT_C, 'gamma-endpoint', 3), notes: 'viewmodel test' }
        endpoints = [...endpoints, created]
        return route.fulfill({ status: 201, json: created })
      })

      let deleteRequests = 0
      await page.route(`**/api/endpoints/${ENDPOINT_C}`, (route) => {
        expect(route.request().method()).toBe('DELETE')
        deleteRequests++
        endpoints = endpoints.filter((item) => item.id !== ENDPOINT_C)
        return route.fulfill({ status: 204 })
      })

      await page.clock.install()
      await ensureDashboardLogin(page)
      await expect.poll(() => Boolean(dashboardSocket)).toBe(true)
      const panel = page.getByRole('tabpanel', { name: 'Endpoints', exact: true })
      const table = panel.getByRole('table')
      const rowFor = (name: string) => table.getByRole('row').filter({
        has: page.getByRole('cell', { name, exact: true }),
      })
      const names = table.locator('tbody tr td:first-child')
      await expect(names).toHaveText(['alpha-endpoint', 'beta-endpoint'])
      await expect(rowFor('alpha-endpoint').locator('td').nth(5)).toHaveText('42ms')
      expect(listRequests).toBe(1)
      if (colorScheme === 'dark') {
        await expect(page.locator('html')).toHaveClass(/\bdark\b/)
      } else {
        await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)
      }

      // Freeze provider polling. Small advances below only deliver React
      // Query's queued renders; none can explain a 5s/10s polling refresh.
      await page.clock.pauseAt(Date.now() + 1000)
      const pausedAt = await page.evaluate(() => Date.now())
      const search = page.getByPlaceholder('Search by name or URL...')
      await search.fill('BETA')
      await expect(names).toHaveText(['beta-endpoint'])
      await search.fill('192.0.2.1')
      await expect(names).toHaveText(['alpha-endpoint'])
      await search.fill('no-such-endpoint')
      await expect(table.getByText('No endpoints match the filter criteria')).toBeVisible()
      await search.clear()
      await table.getByRole('columnheader', { name: 'Name', exact: true }).click()
      await expect(names).toHaveText(['beta-endpoint', 'alpha-endpoint'])
      await table.getByRole('columnheader', { name: 'Name', exact: true }).click()
      await expect(names).toHaveText(['alpha-endpoint', 'beta-endpoint'])
      expect(listRequests).toBe(1)

      const testButton = rowFor('alpha-endpoint').getByRole('button', { name: 'Test Connection', exact: true })
      await testButton.click()
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return rowFor('alpha-endpoint').locator('td').nth(5).textContent()
      }).toBe('75ms')
      await expect(rowFor('alpha-endpoint').locator('td').nth(3)).toHaveText('Online')
      await expect(testButton).toBeEnabled()
      expect(testRequests).toBe(1)
      expect(listRequests).toBe(2)

      const syncButton = rowFor('alpha-endpoint').getByRole('button', { name: 'Sync Models', exact: true })
      await syncButton.click()
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return rowFor('alpha-endpoint').locator('td').nth(6).textContent()
      }).toBe('4')
      await expect(syncButton).toBeEnabled()
      expect(syncRequests).toBe(1)
      expect(listRequests).toBe(3)

      // Aggregate list subscriptions must accept a change scoped to one id.
      endpoints = endpoints.map((item) => item.id === ENDPOINT_A ? { ...item, name: 'alpha-updated' } : item)
      dashboardSocket!.send(JSON.stringify({ changed: 'endpoints', id: ENDPOINT_A }))
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return rowFor('alpha-updated').count()
      }).toBe(1)
      await expect(rowFor('alpha-endpoint')).toHaveCount(0)
      expect(listRequests).toBe(4)

      await panel.getByRole('button', { name: 'Add Endpoint', exact: true }).click()
      const createDialog = page.getByRole('dialog', { name: 'Add New Endpoint', exact: true })
      const createButton = createDialog.getByRole('button', { name: 'Create Endpoint', exact: true })
      await expect(createButton).toBeDisabled()
      await createDialog.getByLabel('Name *', { exact: true }).fill('gamma-endpoint')
      await expect(createButton).toBeDisabled()
      await createDialog.getByLabel('Base URL *', { exact: true }).fill('http://192.0.2.3:8080')
      await createDialog.getByLabel('API Key (optional)', { exact: true }).fill('sk-endpoint')
      await createDialog.getByLabel('Notes (optional)', { exact: true }).fill('viewmodel test')
      await createButton.click()
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return rowFor('gamma-endpoint').count()
      }).toBe(1)
      await expect(createDialog).toBeHidden()
      expect(createRequests).toBe(1)
      expect(listRequests).toBe(5)

      await rowFor('gamma-endpoint').getByRole('button', { name: 'Delete', exact: true }).click()
      const deleteDialog = page.getByRole('alertdialog', { name: 'Delete Endpoint?', exact: true })
      await expect(deleteDialog).toContainText('This will delete "gamma-endpoint".')
      expect(deleteRequests).toBe(0)
      await deleteDialog.getByRole('button', { name: 'Delete', exact: true }).click()
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return rowFor('gamma-endpoint').count()
      }).toBe(0)
      await expect(deleteDialog).toBeHidden()
      expect(deleteRequests).toBe(1)
      expect(listRequests).toBe(6)
      const advancedMs = await page.evaluate(() => Date.now()) - pausedAt
      expect(advancedMs).toBeGreaterThan(0)
      expect(advancedMs).toBeLessThan(1000)

      // Connected provider polling remains at 10s: no notification is sent.
      endpoints = endpoints.map((item) => item.id === ENDPOINT_B ? { ...item, latency_ms: 110 } : item)
      const connectedBaseline = listRequests
      await page.clock.runFor(5000)
      expect(listRequests).toBe(connectedBaseline)
      await expect(rowFor('beta-endpoint').locator('td').nth(5)).toHaveText('42ms')
      await page.clock.runFor(5000)
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return rowFor('beta-endpoint').locator('td').nth(5).textContent()
      }).toBe('110ms')
      expect(listRequests).toBe(connectedBaseline + 1)

      // Reject reconnect attempts before they open, leaving the real hook in
      // its disconnected state so fallback polling must refresh after 5s.
      acceptConnections = false
      await dashboardSocket!.close({ code: 1000, reason: 'Test disconnected polling' })
      await page.clock.runFor(1)
      endpoints = endpoints.map((item) => item.id === ENDPOINT_B ? { ...item, model_count: 7 } : item)
      const disconnectedBaseline = listRequests
      await page.clock.runFor(4000)
      expect(listRequests).toBe(disconnectedBaseline)
      await expect(rowFor('beta-endpoint').locator('td').nth(6)).toHaveText('1')
      await page.clock.runFor(1000)
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return rowFor('beta-endpoint').locator('td').nth(6).textContent()
      }).toBe('7')
      expect(listRequests).toBe(disconnectedBaseline + 1)

      expect(pageErrors).toEqual([])
      await testInfo.attach(`endpoint-table-${colorScheme}`, {
        body: await page.screenshot(),
        contentType: 'image/png',
      })
    })
  })
}
