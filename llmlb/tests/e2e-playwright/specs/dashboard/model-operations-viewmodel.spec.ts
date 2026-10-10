import { test, expect, type Locator, type Route } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'
import type { DashboardEndpoint } from '../../../../src/web/dashboard/src/lib/api/endpoints'

const ENDPOINT_A = '11111111-2222-3333-4444-555555555555'
const ENDPOINT_B = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee'
const REPO_ID = 'model-operations/tiny-llm-GGUF'
const MODEL_A = 'alpha-chat'
const MODEL_B = 'beta-chat'

function endpoint(id: string, name: string, host: number): DashboardEndpoint {
  return {
    id, name, base_url: `http://192.0.2.${host}:8080`, status: 'online',
    endpoint_type: 'xllm', health_check_interval_secs: 30,
    inference_timeout_secs: 600, error_count: 0,
    registered_at: '2026-10-01T00:00:00Z', model_count: 1,
    total_requests: 0, successful_requests: 0, failed_requests: 0,
  }
}

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Model operation ViewModels (${colorScheme}) @dashboard`, () => {
    test.use({ colorScheme })

    test('preserves download and deletion pending states and immediate scoped refresh (#868)', async ({ page }, testInfo) => {
      test.setTimeout(60000)
      const pageErrors: string[] = []
      page.on('pageerror', (error) => pageErrors.push(error.message))
      page.on('console', (message) => {
        if (message.type() === 'error') pageErrors.push(message.text())
      })

      // Authentication and the session probe stay real. Only catalog/model
      // data and endpoint operations are controlled; no download or deletion
      // reaches an endpoint.
      const login = await page.request.post('/api/auth/login', {
        data: { username: 'admin', password: 'test' },
      })
      expect(login.ok()).toBe(true)
      const csrfCookie = (await page.context().cookies()).find((cookie) => cookie.name === 'llmlb_csrf')
      expect(csrfCookie).toBeDefined()
      const csrfToken = decodeURIComponent(csrfCookie!.value)
      expect(csrfToken).not.toBe('')

      // No WS notification can explain an operation's immediate list refresh.
      let socketConnected = false
      await page.routeWebSocket('**/ws/dashboard', () => { socketConnected = true })
      let endpoints = [
        endpoint(ENDPOINT_A, 'model-op-alpha', 1),
        endpoint(ENDPOINT_B, 'model-op-beta', 2),
      ]
      let listRequests = 0
      await page.route('**/api/dashboard/endpoints', (route) => {
        expect(route.request().method()).toBe('GET')
        listRequests++
        return route.fulfill({ json: endpoints })
      })
      await page.route('**/api/dashboard/models*', (route) => route.fulfill({ json: {
        object: 'list',
        data: [MODEL_A, MODEL_B].map((id, index) => ({
          id, object: 'model', created: 0, owned_by: 'load balancer',
          lifecycle_status: 'registered', ready: true, supported_apis: ['chat_completions'],
          tags: [], aliases: [], endpoint_ids: [endpoints[index].id],
        })),
      } }))
      await page.route('**/api/dashboard/model-stats', (route) => route.fulfill({ json: [] }))

      const reads = Object.fromEntries([ENDPOINT_A, ENDPOINT_B].map((id) => [id, {
        models: 0, stats: 0, tps: 0,
      }]))
      for (const id of [ENDPOINT_A, ENDPOINT_B]) {
        await page.route(`**/api/endpoints/${id}/models`, (route) => {
          expect(route.request().method()).toBe('GET')
          reads[id].models++
          return route.fulfill({ json: { endpoint_id: id, models: [] } })
        })
        await page.route(`**/api/endpoints/${id}/model-stats`, (route) => {
          expect(route.request().method()).toBe('GET')
          reads[id].stats++
          return route.fulfill({ json: [] })
        })
        await page.route(`**/api/endpoints/${id}/model-tps`, (route) => {
          expect(route.request().method()).toBe('GET')
          reads[id].tps++
          return route.fulfill({ json: [] })
        })
      }

      const catalogReads = { search: 0, detail: 0, recommendations: 0 }
      await page.route('**/api/catalog/search*', (route) => {
        const url = new URL(route.request().url())
        expect(url.searchParams.get('q')).toBe('tiny')
        expect(url.searchParams.get('limit')).toBe('20')
        catalogReads.search++
        return route.fulfill({ json: { models: [{
          repo_id: REPO_ID, description: 'Controlled tiny model', downloads: 12,
          tags: ['gguf'], engine_names: { xllm: 'tiny-llm', ollama: null },
          supports_download: ['xllm'],
        }] } })
      })
      await page.route(`**/api/catalog/${REPO_ID}`, (route) => {
        catalogReads.detail++
        return route.fulfill({ json: {
          repo_id: REPO_ID, description: 'Controlled tiny model',
          engine_names: { xllm: 'tiny-llm', ollama: null },
          supports_download: ['xllm'], siblings: [{ rfilename: 'tiny.Q4_K_M.gguf' }],
        } })
      })
      await page.route(`**/api/catalog/recommend-endpoints/${REPO_ID}`, (route) => {
        catalogReads.recommendations++
        return route.fulfill({ json: { endpoints: endpoints.map((item) => ({
          id: item.id, name: item.name, endpoint_type: item.endpoint_type,
          can_download: item.id === ENDPOINT_A, has_model: false,
        })) } })
      })

      let downloadRoute: Route | undefined
      await page.route('**/api/endpoints/*/download', (route) => {
        expect(new URL(route.request().url()).pathname).toBe(`/api/endpoints/${ENDPOINT_A}/download`)
        expect(route.request().method()).toBe('POST')
        expect(route.request().headers()['x-csrf-token']).toBe(csrfToken)
        expect(route.request().postDataJSON()).toEqual({ model: REPO_ID, hf_repo: REPO_ID })
        expect(downloadRoute).toBeUndefined()
        downloadRoute = route // The test releases the success response after inspecting pending UI.
      })
      let deleteRoute: Route | undefined
      await page.route('**/api/endpoints/*/models/delete', (route) => {
        expect(new URL(route.request().url()).pathname).toBe(`/api/endpoints/${ENDPOINT_A}/models/delete`)
        // The existing endpoint model deletion API uses POST, not HTTP DELETE.
        expect(route.request().method()).toBe('POST')
        expect(route.request().headers()['x-csrf-token']).toBe(csrfToken)
        expect(route.request().postDataJSON()).toEqual({ model: MODEL_A })
        expect(deleteRoute).toBeUndefined()
        deleteRoute = route
      })

      await page.clock.install()
      await page.clock.pauseAt(Date.now() + 1000)
      await ensureDashboardLogin(page)
      await expect.poll(() => listRequests).toBe(1)
      await expect.poll(() => socketConnected).toBe(true)
      if (colorScheme === 'dark') await expect(page.locator('html')).toHaveClass(/\bdark\b/)
      else await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)
      const pausedAt = await page.evaluate(() => Date.now())
      const visible = async (locator: Locator) => expect.poll(async () => {
        await page.clock.runFor(1)
        return locator.isVisible()
      }).toBe(true)
      const screenshot = async (name: string) => testInfo.attach(`${name}-${colorScheme}`, {
        body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png',
      })

      await page.getByRole('tab', { name: 'Models', exact: true }).click()
      const table = page.getByRole('tabpanel', { name: 'Models', exact: true }).getByRole('table')
      const modelRow = (id: string) => table.getByRole('row').filter({
        has: page.locator(`[data-model-id="${id}"]`),
      })
      await visible(modelRow(MODEL_A))
      await modelRow(MODEL_A).getByRole('button', { name: 'Expand row', exact: true }).click()
      await modelRow(MODEL_B).getByRole('button', { name: 'Expand row', exact: true }).click()
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return reads
      }).toEqual({
        [ENDPOINT_A]: { models: 0, stats: 1, tps: 1 },
        [ENDPOINT_B]: { models: 0, stats: 1, tps: 1 },
      })

      await page.getByRole('button', { name: 'Add Model', exact: true }).click()
      const wizard = page.getByRole('dialog')
      const search = wizard.getByPlaceholder('Search models (e.g., llama, mistral, phi)...')
      await search.fill('t')
      await page.clock.runFor(300)
      expect(catalogReads).toEqual({ search: 0, detail: 0, recommendations: 0 })
      await search.fill('tiny')
      await page.clock.runFor(300)
      const catalogRow = wizard.getByRole('row').filter({ hasText: REPO_ID })
      await visible(catalogRow)
      await catalogRow.click()
      await visible(wizard.getByText('tiny.Q4_K_M.gguf', { exact: true }))
      await wizard.getByRole('button', { name: 'Select Endpoints', exact: true }).click()
      await visible(wizard.getByText('model-op-alpha', { exact: true }))
      await expect(wizard.getByText('model-op-beta', { exact: true })).toHaveCount(0)
      const startDownload = wizard.getByRole('button', { name: 'Download to 0 Endpoints', exact: true })
      await expect(startDownload).toBeDisabled()
      await wizard.getByText('model-op-alpha', { exact: true }).click()
      await wizard.getByRole('button', { name: 'Download to 1 Endpoint', exact: true }).click()
      await expect.poll(() => Boolean(downloadRoute)).toBe(true)
      await visible(wizard.getByText('Sending download requests...', { exact: true }))
      await page.clock.runFor(100)
      expect(listRequests).toBe(1)
      await expect(wizard.getByRole('button', { name: 'Done', exact: true })).toHaveCount(0)
      await screenshot('model-download-pending')

      // A changed name makes consumption of the fresh list observable in the
      // already-expanded model row, without mounting another query observer.
      endpoints = endpoints.map((item) => item.id === ENDPOINT_A
        ? { ...item, name: 'model-op-alpha-downloaded', model_count: 2 } : item)
      await downloadRoute!.fulfill({ json: { task_id: 'controlled-download' } })
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return { lists: listRequests, done: await wizard.getByRole('button', { name: 'Done', exact: true }).isVisible() }
      }).toEqual({ lists: 2, done: true })
      await expect(page.getByText('Download requests sent', { exact: true })).toBeVisible()
      await expect(wizard.getByText('All download requests have been sent. Check endpoint details for progress.')).toBeVisible()
      await screenshot('model-download-complete')
      await wizard.getByRole('button', { name: 'Done', exact: true }).click()
      await visible(table.getByText('model-op-alpha-downloaded', { exact: true }))
      await expect(table.getByText('model-op-beta', { exact: true })).toBeVisible()

      const endpointRow = table.getByText('model-op-alpha-downloaded', { exact: true }).locator('..').locator('..')
      await endpointRow.getByRole('button', { name: 'Delete model from endpoint', exact: true }).click()
      const confirmation = page.getByRole('dialog', { name: 'Delete Model', exact: true })
      await expect(confirmation).toContainText(`This action will permanently remove ${MODEL_A} from model-op-alpha-downloaded.`)
      expect(deleteRoute).toBeUndefined()
      const confirmDelete = confirmation.getByRole('button', { name: 'Delete', exact: true })
      await confirmDelete.click()
      await expect.poll(() => Boolean(deleteRoute)).toBe(true)
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return confirmDelete.isDisabled()
      }).toBe(true)
      await page.clock.runFor(100)
      expect(listRequests).toBe(2)
      await expect(confirmation).toBeVisible()
      await expect(page.getByText('Model deleted', { exact: true })).toHaveCount(0)
      await screenshot('model-delete-pending')

      endpoints = endpoints.map((item) => item.id === ENDPOINT_A
        ? { ...item, name: 'model-op-alpha-deleted', model_count: 1 } : item)
      await deleteRoute!.fulfill({ status: 204 })
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return {
          lists: listRequests, closed: !(await confirmation.isVisible()),
          freshName: await table.getByText('model-op-alpha-deleted', { exact: true }).isVisible(),
        }
      }).toEqual({ lists: 3, closed: true, freshName: true })
      await expect(page.getByText('Model deleted', { exact: true })).toBeVisible()
      await expect(table.getByText('model-op-beta', { exact: true })).toBeVisible()

      // Endpoint-detail model observers are not mounted in the Models tab.
      // Exact endpointModels invalidation scope is tested at the VM boundary;
      // this browser check catches extra reads or unrelated stats/TPS refreshes.
      expect(reads).toEqual({
        [ENDPOINT_A]: { models: 0, stats: 1, tps: 1 },
        [ENDPOINT_B]: { models: 0, stats: 1, tps: 1 },
      })
      expect(catalogReads).toEqual({ search: 1, detail: 1, recommendations: 1 })
      const advancedMs = await page.evaluate(() => Date.now()) - pausedAt
      expect(advancedMs).toBeGreaterThanOrEqual(800)
      expect(advancedMs).toBeLessThan(5000)
      await screenshot('model-operations-complete')
      expect(pageErrors).toEqual([])
    })
  })
}
