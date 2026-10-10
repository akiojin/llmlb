import { test, expect, type Locator, type Route } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'

const ID = '11111111-2222-3333-4444-555555555555'
for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Model download progress (${colorScheme}) @dashboard`, () => {
    test.use({ colorScheme })
    test('preserves 2 second progress, completion refresh, close lifetime and failure (#872)', async ({ page }, testInfo) => {
      test.setTimeout(60000)
      const errors: string[] = []
      page.on('pageerror', (error) => errors.push(error.message))
      page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text()) })
      const login = await page.request.post('/api/auth/login', { data: { username: 'admin', password: 'test' } })
      expect(login.ok()).toBe(true)
      const csrfCookie = (await page.context().cookies()).find((cookie) => cookie.name === 'llmlb_csrf')
      expect(csrfCookie).toBeDefined()
      const csrfToken = decodeURIComponent(csrfCookie!.value)
      await page.routeWebSocket('**/ws/dashboard', () => {})
      let endpointReads = 0
      let modelReads = 0
      let progressReads = 0
      let phase: 'downloading' | 'completed' | 'failed' = 'downloading'
      let downloadRoute: Route | undefined
      await page.route('**/api/dashboard/endpoints', (route) => {
        endpointReads++
        return route.fulfill({ json: [{
          id: ID, name: 'download-progress-endpoint', base_url: 'http://192.0.2.1:8080', status: 'online',
          endpoint_type: 'xllm', health_check_interval_secs: 30, inference_timeout_secs: 600,
          error_count: 0, registered_at: '2026-10-01T00:00:00Z', model_count: phase === 'completed' ? 1 : 0,
          total_requests: 0, successful_requests: 0, failed_requests: 0,
        }] })
      })
      await page.route(`**/api/endpoints/${ID}/models`, (route) => {
        modelReads++
        return route.fulfill({ json: { endpoint_id: ID, models: phase === 'completed'
          ? [{ id: 'tiny', object: 'model', created: 0, owned_by: 'xllm' }] : [] } })
      })
      await page.route(`**/api/endpoints/${ID}/model-stats`, (route) => route.fulfill({ json: [] }))
      await page.route(`**/api/endpoints/${ID}/model-tps`, (route) => route.fulfill({ json: [] }))
      await page.route(`**/api/endpoints/${ID}/today-stats`, (route) => route.fulfill({ json: {
        date: '2026-10-10', total_requests: 0, successful_requests: 0, failed_requests: 0,
      } }))
      await page.route(`**/api/endpoints/${ID}/download`, (route) => {
        expect(route.request().method()).toBe('POST')
        expect(route.request().headers()['x-csrf-token']).toBe(csrfToken)
        expect(route.request().postDataJSON()).toEqual({ model: 'tiny' })
        downloadRoute = route
      })
      await page.route(`**/api/endpoints/${ID}/download/progress`, (route) => {
        progressReads++
        return route.fulfill({ json: { tasks: [{ task_id: 'download-task', model: 'tiny', status: phase,
          progress: phase === 'completed' ? 100 : 35, speed_mbps: 1.25, eta_seconds: 65,
          ...(phase === 'failed' ? { error: 'disk full' } : {}),
        }] } })
      })
      await page.clock.install()
      await page.clock.pauseAt(Date.now() + 1000)
      await ensureDashboardLogin(page)
      const visible = async (locator: Locator) => expect.poll(async () => {
        await page.clock.runFor(1)
        return locator.isVisible()
      }).toBe(true)
      await expect.poll(() => endpointReads).toBe(1)
      if (colorScheme === 'dark') await expect(page.locator('html')).toHaveClass(/\bdark\b/)
      else await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)
      const row = page.getByRole('row').filter({ hasText: 'download-progress-endpoint' })
      await visible(row)
      await row.locator('button[title="Details"]').click()
      const details = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'download-progress-endpoint', exact: true }) })
      await visible(details)
      await details.getByRole('button', { name: 'Download Model', exact: true }).click()
      const dialog = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Download Model', exact: true }) })
      await visible(dialog)
      await dialog.getByLabel('Model Name', { exact: true }).fill(' tiny ')
      await dialog.getByRole('button', { name: 'Download', exact: true }).click()
      await expect.poll(() => !!downloadRoute).toBe(true)
      await visible(dialog.getByRole('button', { name: 'Download', exact: true }).filter({ has: page.locator('.animate-spin') }))
      expect(progressReads).toBe(0)
      await downloadRoute!.fulfill({ json: { task_id: 'download-task' } })
      await visible(dialog.getByText('1.3 Mbps / ETA 1m 5s', { exact: true }))
      expect(progressReads).toBe(1)
      await page.keyboard.press('Escape')
      await visible(dialog.getByText(/Downloading\s+tiny/))
      await page.clock.runFor(1900)
      expect(progressReads).toBe(1)
      await page.clock.runFor(100)
      await expect.poll(() => progressReads).toBe(2)
      await testInfo.attach(`progress-${colorScheme}`, { body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png' })
      const before = { endpoints: endpointReads, models: modelReads }
      phase = 'completed'
      await page.clock.runFor(2000)
      await visible(dialog.getByText('Download Completed', { exact: true }))
      await expect.poll(() => ({ endpoints: endpointReads, models: modelReads })).toEqual({ endpoints: before.endpoints + 1, models: before.models + 1 })
      await testInfo.attach(`completed-${colorScheme}`, { body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png' })
      const completedReads = progressReads
      await page.clock.runFor(6000)
      expect(progressReads).toBe(completedReads)
      await dialog.getByRole('button', { name: 'Close', exact: true }).last().click()
      await page.clock.runFor(200)
      await expect(dialog).not.toBeVisible()
      phase = 'failed'
      await details.getByRole('button', { name: 'Download Model', exact: true }).click()
      await visible(dialog.getByLabel('Model Name', { exact: true }))
      await expect(dialog.getByLabel('Model Name', { exact: true })).toHaveValue('')
      await dialog.getByLabel('Model Name', { exact: true }).fill('tiny')
      downloadRoute = undefined
      await dialog.getByRole('button', { name: 'Download', exact: true }).click()
      await expect.poll(() => !!downloadRoute).toBe(true)
      await downloadRoute!.fulfill({ json: { task_id: 'download-task' } })
      await visible(dialog.getByText('Download Failed', { exact: true }))
      await expect(dialog.getByText('disk full', { exact: true })).toBeVisible()
      await testInfo.attach(`failed-${colorScheme}`, { body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png' })
      const failedReads = progressReads
      await page.clock.runFor(6000)
      expect(progressReads).toBe(failedReads)
      await dialog.getByRole('button', { name: 'Cancel', exact: true }).click()
      await page.clock.runFor(200)
      await expect(dialog).not.toBeVisible()
      expect(errors).toEqual([])
    })
  })
}
