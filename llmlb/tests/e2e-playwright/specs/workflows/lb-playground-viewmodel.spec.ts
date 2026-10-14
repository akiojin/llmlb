import { test, expect } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'
import type { ChatCompletionRequest } from '../../../../src/web/dashboard/src/lib/api/chat'
import type { RequestResponseRecord } from '../../../../src/web/dashboard/src/lib/api/dashboard'

const FIRST_MODEL = 'alpha-chat'
const INITIAL_MODEL = 'bravo-chat'

function historyRecord(
  request: ChatCompletionRequest,
  endpointName: string,
  durationMs: number,
): RequestResponseRecord {
  return {
    id: request.user!, timestamp: '2026-10-09T00:00:00Z',
    request_type: 'chat', model: request.model, endpoint_name: endpointName,
    request_body: request, duration_ms: durationMs, status: { type: 'success' },
  }
}

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Load Balancer Playground ViewModel (${colorScheme}) @playground`, () => {
    test.use({ colorScheme })

    test('preserves model selection, provider polling and completion-driven distribution (#860)', async ({ page }, testInfo) => {
      const pageErrors: string[] = []
      page.on('pageerror', (error) => pageErrors.push(error.message))
      page.on('console', (message) => {
        if (message.type() === 'error') pageErrors.push(message.text())
      })

      // Exercise the real login and session probe. Only Playground API data is
      // controlled; neither a rejected login nor a 401 may be hidden by a route.
      const login = await page.request.post('/api/auth/login', {
        data: { username: 'admin', password: 'test' },
      })
      expect(login.ok()).toBe(true)

      let modelIds = [FIRST_MODEL, INITIAL_MODEL]
      let modelRequests = 0
      await page.route('**/api/dashboard/playground/models', (route) => {
        expect(route.request().method()).toBe('GET')
        modelRequests++
        return route.fulfill({ json: {
          object: 'list',
          data: modelIds.map((id) => ({
            id, object: 'model', created: 0, owned_by: 'load balancer',
            lifecycle_status: 'registered', ready: true,
          })),
        } })
      })

      let historyRecords: RequestResponseRecord[] = []
      let distributionRequests = 0
      await page.route('**/api/dashboard/request-responses*', (route) => {
        const params = new URL(route.request().url()).searchParams
        // Dashboard also fetches history while it initializes the theme. Its
        // request has no offset and must not count as a Playground lookup.
        if (!params.has('offset')) return route.continue()
        expect(route.request().method()).toBe('GET')
        expect(params.get('limit')).toBe('100')
        expect(params.get('offset')).toBe('0')
        distributionRequests++
        return route.fulfill({ json: {
          records: historyRecords, total_count: historyRecords.length,
          page: 1, per_page: 100,
        } })
      })

      let releaseChat!: () => void
      const chatReady = new Promise<void>((resolve) => { releaseChat = resolve })
      const chatRequests: ChatCompletionRequest[] = []
      await page.route('**/api/dashboard/playground/chat/completions', async (route) => {
        expect(route.request().method()).toBe('POST')
        expect(route.request().headers()['x-csrf-token']).toBeTruthy()
        const request = route.request().postDataJSON() as ChatCompletionRequest
        chatRequests.push(request)
        await chatReady
        historyRecords = [
          historyRecord(request, 'chat-node', 120),
          historyRecord({ ...request, user: 'unrelated-run' }, 'unrelated-node', 999),
        ]
        await route.fulfill({
          contentType: 'text/event-stream',
          body: `data: ${JSON.stringify({ choices: [{ delta: { content: 'ViewModel chat answer' } }] })}\n\ndata: [DONE]\n\n`,
        })
      })

      let releaseLoadTest!: () => void
      const loadTestReady = new Promise<void>((resolve) => { releaseLoadTest = resolve })
      const loadTestRequests: ChatCompletionRequest[] = []
      await page.route('**/api/dashboard/playground/load-test/chat/completions', async (route) => {
        expect(route.request().method()).toBe('POST')
        expect(route.request().headers()['x-csrf-token']).toBeTruthy()
        const request = route.request().postDataJSON() as ChatCompletionRequest
        loadTestRequests.push(request)
        const endpointName = loadTestRequests.length % 2 === 1 ? 'load-node-a' : 'load-node-b'
        await loadTestReady
        historyRecords.push(historyRecord(request, endpointName, 80))
        await route.fulfill({ json: { choices: [{ message: { content: 'Load response' } }] } })
      })

      // Stop time before mounting either page, so no real elapsed time can
      // masquerade as the provider's inherited five-second polling interval.
      await page.clock.install()
      await page.clock.pauseAt(Date.now() + 1000)
      await ensureDashboardLogin(page)
      if (colorScheme === 'dark') {
        await expect(page.locator('html')).toHaveClass(/\bdark\b/)
      } else {
        await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)
      }
      // Keep the Dashboard-applied theme across the same-document hash route.
      // The server accepts both /dashboard and /dashboard/.
      await page.evaluate((model) => {
        window.location.hash = `lb-playground?model=${encodeURIComponent(model)}`
      }, INITIAL_MODEL)
      await expect(page).toHaveURL(/\/dashboard\/?#lb-playground\?model=bravo-chat$/)
      if (colorScheme === 'dark') {
        await expect(page.locator('html')).toHaveClass(/\bdark\b/)
      } else {
        await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)
      }
      const sidebar = page.locator('#lb-playground-sidebar')
      const modelSelect = page.locator('#lb-model-select')
      const modelSummary = sidebar.getByText('Models:', { exact: true }).locator('..')
      const distributionPanel = page.locator('#lb-distribution-panel')
      const distributionRows = page.getByTestId('lb-distribution-row')
      const input = page.locator('#lb-chat-input')

      await expect.poll(async () => {
        await page.clock.runFor(1)
        return modelSelect.textContent()
      }).toBe(INITIAL_MODEL)
      await expect(modelSummary).toHaveText('Models: 2')
      expect(modelRequests).toBe(1)
      expect(distributionRequests).toBe(0)
      await expect(distributionPanel).toHaveCount(0)

      await test.step('manual refresh preserves the model chosen after the deep link', async () => {
        await modelSelect.click()
        await page.getByRole('option', { name: FIRST_MODEL, exact: true }).click()
        await expect(modelSelect).toHaveText(FIRST_MODEL)
        modelIds = [...modelIds, 'charlie-chat']
        await sidebar.getByRole('button', { name: 'Refresh Models', exact: true }).click()
        await expect.poll(async () => {
          await page.clock.runFor(1)
          return modelSummary.textContent()
        }).toBe('Models: 3')
        await expect(modelSelect).toHaveText(FIRST_MODEL)
        expect(modelRequests).toBe(2)
      })

      await test.step('the provider still polls models after five seconds', async () => {
        modelIds = [...modelIds, 'delta-chat']
        const baseline = modelRequests
        await page.clock.runFor(4000)
        expect(modelRequests).toBe(baseline)
        await expect(modelSummary).toHaveText('Models: 3')
        await page.clock.runFor(1000)
        await expect.poll(async () => {
          await page.clock.runFor(1)
          return modelSummary.textContent()
        }).toBe('Models: 4')
        expect(modelRequests).toBe(baseline + 1)
        await expect(modelSelect).toHaveText(FIRST_MODEL)
        expect(distributionRequests).toBe(0)
      })

      await test.step('chat completion and explicit Refresh retrieve the current distribution', async () => {
        await input.fill('ViewModel chat prompt')
        await page.locator('#lb-send-chat').click()
        await expect.poll(() => chatRequests.length).toBe(1)
        expect(chatRequests[0]).toMatchObject({
          model: FIRST_MODEL, stream: true,
          messages: [{ role: 'user', content: 'ViewModel chat prompt' }],
          user: expect.stringMatching(/^lbpg:.+:1$/),
        })
        await expect(page.locator('#lb-stop-chat')).toBeVisible()
        expect(distributionRequests).toBe(0)
        await expect(distributionPanel).toHaveCount(0)

        releaseChat()
        await expect.poll(async () => {
          await page.clock.runFor(1)
          return distributionRows.first().locator('span').first().textContent()
        }).toBe('chat-node')
        await expect(distributionRows).toHaveCount(1)
        await expect(distributionRows.first().locator('span')).toHaveText(['chat-node', '1', '1', '0', '120'])
        await expect(page.getByText('ViewModel chat answer', { exact: true })).toBeVisible()
        await expect(page.locator('#lb-send-chat')).toBeVisible()
        await expect(input).toBeEnabled()
        await expect(page.locator('#lb-distribution-summary')).toContainText('Matched: 1/1')
        await expect(distributionPanel.getByText('unrelated-node')).toHaveCount(0)
        expect(distributionRequests).toBe(1)

        historyRecords = historyRecords.map((record) => record.id === chatRequests[0].user
          ? { ...record, endpoint_name: 'chat-node-refreshed' } : record)
        await distributionPanel.getByRole('button', { name: 'Refresh', exact: true }).click()
        await expect.poll(async () => {
          await page.clock.runFor(1)
          return distributionRows.first().locator('span').first().textContent()
        }).toBe('chat-node-refreshed')
        expect(distributionRequests).toBe(2)
      })

      await test.step('admin load test completes three requests before fetching its distribution', async () => {
        await page.locator('#lb-mode-load-test').click()
        await page.locator('#lb-total-requests').fill('3')
        await page.locator('#lb-concurrency').fill('2')
        await page.locator('#lb-interval-ms').fill('0')
        await input.fill('ViewModel load prompt')
        await page.locator('#lb-start-load-test').click()
        await expect.poll(() => loadTestRequests.length).toBe(2)
        await expect(page.locator('#lb-stop-load-test')).toBeVisible()
        await expect(input).toBeDisabled()
        expect(distributionRequests).toBe(2)

        releaseLoadTest()
        await expect.poll(async () => {
          await page.clock.runFor(1)
          return page.locator('#lb-load-test-progress').textContent()
        }).toContain('3/3 completed')
        await expect(page.getByText('Load test finished. requests=3, success=3, error=0')).toBeVisible()
        await expect(page.locator('#lb-start-load-test')).toBeEnabled()
        await expect(input).toBeEnabled()
        expect(loadTestRequests).toHaveLength(3)
        expect(new Set(loadTestRequests.map((request) => request.user)).size).toBe(3)
        for (const request of loadTestRequests) {
          expect(request).toMatchObject({
            model: FIRST_MODEL, stream: false,
            messages: [{ role: 'user', content: 'ViewModel load prompt' }],
            user: expect.stringMatching(/^lbpg:.+:[123]$/),
          })
        }
        await expect(page.locator('#lb-distribution-summary')).toContainText('Matched: 3/3')
        await expect(distributionRows).toHaveCount(2)
        await expect(distributionRows.first().locator('span')).toHaveText(['load-node-a', '2', '2', '0', '80'])
        await expect(distributionRows.last().locator('span')).toHaveText(['load-node-b', '1', '1', '0', '80'])
        expect(chatRequests).toHaveLength(1)
        expect(distributionRequests).toBe(3)
      })

      expect(pageErrors).toEqual([])
      await testInfo.attach(`lb-playground-${colorScheme}`, {
        body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png',
      })
    })
  })
}
