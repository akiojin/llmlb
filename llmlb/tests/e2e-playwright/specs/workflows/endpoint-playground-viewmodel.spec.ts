import { test, expect } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'
import type { DashboardEndpoint, endpointsApi } from '../../../../src/web/dashboard/src/lib/api/endpoints'

const ENDPOINT_ID = '11111111-2222-3333-4444-555555555555'
const FIRST_MODEL = 'alpha-chat'
const SELECTED_MODEL = 'bravo-chat'
type EndpointChatRequest = Parameters<typeof endpointsApi.chatCompletions>[1]

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Endpoint Playground ViewModel (${colorScheme}) @playground`, () => {
    test.use({ colorScheme })

    test('preserves endpoint/model polling, chat settings, streaming and cURL (#862)', async ({ page }, testInfo) => {
      const pageErrors: string[] = []
      page.on('pageerror', (error) => pageErrors.push(error.message))
      page.on('console', (message) => {
        if (message.type() === 'error') pageErrors.push(message.text())
      })

      // Login and the session probe use the real server. Only endpoint data
      // and inference responses are controlled below, never authentication.
      const login = await page.request.post('/api/auth/login', {
        data: { username: 'admin', password: 'test' },
      })
      expect(login.ok()).toBe(true)

      let socketConnections = 0
      let socketCloses = 0
      page.on('websocket', (socket) => {
        if (!socket.url().includes('/ws/dashboard')) return
        socketConnections++
        socket.on('close', () => { socketCloses++ })
      })

      let endpoint: DashboardEndpoint = {
        id: ENDPOINT_ID, name: 'playground-node', base_url: 'http://192.0.2.1:8080/',
        status: 'online', endpoint_type: 'xllm', health_check_interval_secs: 30,
        inference_timeout_secs: 600, error_count: 0,
        registered_at: '2026-10-01T00:00:00Z', model_count: 2,
        total_requests: 0, successful_requests: 0, failed_requests: 0,
      }
      let endpointRequests = 0
      await page.route(`**/api/endpoints/${ENDPOINT_ID}`, (route) => {
        expect(route.request().method()).toBe('GET')
        endpointRequests++
        return route.fulfill({ json: endpoint })
      })

      let modelIds = [FIRST_MODEL, SELECTED_MODEL]
      let modelRequests = 0
      await page.route(`**/api/endpoints/${ENDPOINT_ID}/models`, (route) => {
        expect(route.request().method()).toBe('GET')
        modelRequests++
        return route.fulfill({ json: {
          endpoint_id: ENDPOINT_ID,
          models: modelIds.map((model_id) => ({ model_id, max_tokens: 8192 })),
        } })
      })

      let releaseChat!: () => void
      const chatReady = new Promise<void>((resolve) => { releaseChat = resolve })
      const chatRequests: EndpointChatRequest[] = []
      await page.route(`**/api/endpoints/${ENDPOINT_ID}/chat/completions`, async (route) => {
        expect(route.request().method()).toBe('POST')
        expect(route.request().headers()['x-csrf-token']).toBeTruthy()
        chatRequests.push(route.request().postDataJSON() as EndpointChatRequest)
        await chatReady
        await route.fulfill({
          contentType: 'text/event-stream',
          body: [
            `data: ${JSON.stringify({ choices: [{ delta: { reasoning_content: 'Endpoint reasoning' } }] })}`,
            `data: ${JSON.stringify({ choices: [{ delta: { content: 'Endpoint ViewModel answer' } }] })}`,
            'data: [DONE]', '',
          ].join('\n\n'),
        })
      })

      // Freeze time before mounting the Playground. Small advances below
      // deliver queued React Query renders without triggering its 5s polling.
      await page.clock.install()
      await page.clock.pauseAt(Date.now() + 1000)
      await ensureDashboardLogin(page)
      await expect(page.locator('#connection-status')).toContainText('Online')
      await expect.poll(() => socketConnections).toBeGreaterThan(0)
      const dashboardConnections = socketConnections
      await page.evaluate((id) => { window.location.hash = `playground/${id}` }, ENDPOINT_ID)
      await expect(page).toHaveURL(new RegExp(`/dashboard/?#playground/${ENDPOINT_ID}$`))
      await expect.poll(() => socketCloses).toBe(dashboardConnections)

      const modelSelect = page.getByRole('combobox')
      const modelSummary = page.getByText('Models:', { exact: true }).locator('..')
      const statusSummary = page.getByText('Status:', { exact: true }).locator('..')
      const input = page.getByPlaceholder('Type a message or attach files...')
      await expect.poll(async () => {
        await page.clock.runFor(1)
        return modelSelect.textContent()
      }).toBe(FIRST_MODEL)
      await expect(page.getByRole('heading', { name: 'playground-node', exact: true })).toBeVisible()
      await expect(modelSummary).toHaveText('Models: 2')
      await expect(statusSummary).toHaveText('Status: Online')
      await expect(page.getByText('Streaming', { exact: true })).toBeVisible()
      expect(endpointRequests).toBe(1)
      expect(modelRequests).toBe(1)
      // Dashboard owns and closes its socket on navigation. Endpoint
      // Playground must not introduce another connection to change US-004.
      expect(socketConnections).toBe(dashboardConnections)
      if (colorScheme === 'dark') {
        await expect(page.locator('html')).toHaveClass(/\bdark\b/)
      } else {
        await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)
      }

      await modelSelect.click()
      await page.getByRole('option', { name: SELECTED_MODEL, exact: true }).click()
      await expect(modelSelect).toHaveText(SELECTED_MODEL)

      await test.step('both queries retain the provider five-second polling interval', async () => {
        endpoint = { ...endpoint, name: 'playground-node-refreshed', status: 'pending' }
        modelIds = [...modelIds, 'charlie-chat']
        const endpointBaseline = endpointRequests
        const modelBaseline = modelRequests
        await page.clock.runFor(4990)
        expect(endpointRequests).toBe(endpointBaseline)
        expect(modelRequests).toBe(modelBaseline)
        await expect(modelSummary).toHaveText('Models: 2')
        await expect(statusSummary).toHaveText('Status: Online')
        await page.clock.runFor(10)
        await expect.poll(async () => {
          await page.clock.runFor(1)
          return {
            endpointRequests, modelRequests,
            models: await modelSummary.textContent(),
            status: await statusSummary.textContent(),
          }
        }).toEqual({
          endpointRequests: endpointBaseline + 1, modelRequests: modelBaseline + 1,
          models: 'Models: 3', status: 'Status: Pending',
        })
        await expect(page.getByRole('heading', { name: 'playground-node-refreshed', exact: true })).toBeVisible()
        await expect(modelSelect).toHaveText(SELECTED_MODEL)
        await modelSelect.click()
        await expect(page.getByRole('option', { name: 'charlie-chat', exact: true })).toBeVisible()
        await page.keyboard.press('Escape')
      })

      await test.step('settings reach the endpoint proxy and streamed output finishes', async () => {
        await page.getByRole('button', { name: 'Settings', exact: true }).click()
        const settings = page.getByRole('dialog', { name: 'Settings', exact: true })
        await settings.getByPlaceholder('You are a helpful assistant...').fill('Be concise.')
        await settings.getByRole('checkbox', { name: /Use model max context/ }).click()
        await settings.getByRole('button', { name: 'Done', exact: true }).click()
        // Flush Radix's deferred close/focus restoration before the next
        // interaction; otherwise the frozen clock restores focus after chat.
        await page.clock.runFor(1)
        await expect(settings).toHaveCount(0)
        await input.fill('Endpoint ViewModel prompt')
        await page.getByRole('button', { name: 'Send', exact: true }).click()
        await expect.poll(() => chatRequests.length).toBe(1)
        expect(chatRequests[0]).toEqual({
          model: SELECTED_MODEL,
          messages: [
            { role: 'system', content: 'Be concise.' },
            { role: 'user', content: 'Endpoint ViewModel prompt' },
          ],
          stream: true, temperature: 0.7, max_tokens: 8192,
        })
        await expect(page.getByRole('button', { name: 'Stop', exact: true })).toBeVisible()
        await expect(input).toHaveValue('')
        releaseChat()
        await expect.poll(async () => {
          await page.clock.runFor(1)
          return page.getByText('Endpoint ViewModel answer', { exact: true }).count()
        }).toBe(1)
        await expect(page.getByText('Show reasoning', { exact: true })).toBeVisible()
        await expect(page.getByRole('button', { name: 'Stop', exact: true })).toHaveCount(0)
        await expect(page.getByRole('button', { name: 'Send', exact: true })).toBeVisible()
        await expect(input).toBeFocused()
        await page.getByText('Show reasoning', { exact: true }).click()
        await expect(page.getByText('Endpoint reasoning', { exact: true })).toBeVisible()
      })

      await test.step('cURL retains the direct endpoint URL and current settings', async () => {
        await page.getByRole('button', { name: 'cURL', exact: true }).click()
        const dialog = page.getByRole('dialog', { name: 'cURL Command', exact: true })
        const command = dialog.locator('pre')
        await expect(command).toContainText("curl -X POST 'http://192.0.2.1:8080/v1/chat/completions'")
        await expect(command).toContainText(`"model": "${SELECTED_MODEL}"`)
        await expect(command).toContainText('"content": "Be concise."')
        await expect(command).toContainText('"content": "Endpoint ViewModel answer"')
        await expect(command).toContainText('"max_tokens": 8192')
        await expect(command).not.toContainText('Authorization:')
        await page.keyboard.press('Escape')
        await page.clock.runFor(1)
        await expect(dialog).toHaveCount(0)
      })

      expect(socketConnections).toBe(dashboardConnections)
      expect(pageErrors).toEqual([])
      await testInfo.attach(`endpoint-playground-${colorScheme}`, {
        body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png',
      })
      await page.getByRole('button', { name: 'Back to Dashboard', exact: true }).click()
      await expect(page.locator('#theme-toggle')).toBeVisible()
      expect(pageErrors).toEqual([])
    })
  })
}
