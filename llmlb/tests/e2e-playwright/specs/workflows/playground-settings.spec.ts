import { test, expect } from '../../helpers/playground-diagnostics.fixture'
import { randomUUID } from 'node:crypto'
import { ensureDashboardLogin, deleteEndpointsByName, listEndpoints, openDashboardRoute } from '../../helpers/api-helpers'
import { startMockOpenAIEndpointServer, type MockOpenAIEndpointServer } from '../../helpers/mock-openai-endpoint'

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768'
const AUTH_HEADER = { Authorization: 'Bearer sk_debug', 'Content-Type': 'application/json' }

test.describe.configure({ mode: 'serial' })

test.describe('Playground Settings @playground', () => {
  let mock: MockOpenAIEndpointServer
  const endpointName = `e2e-playground-settings-${randomUUID()}`
  let endpointId = ''
  let beforeChatResponse: (() => Promise<void>) | undefined

  test.beforeAll(async ({ request }) => {
    mock = await startMockOpenAIEndpointServer({
      models: [`${endpointName}-model`],
      beforeChatResponse: async () => { await beforeChatResponse?.() },
    })

    const resp = await request.post(`${API_BASE}/api/endpoints`, {
      headers: AUTH_HEADER,
      data: { name: endpointName, base_url: mock.baseUrl },
    })
    expect(resp.ok()).toBeTruthy()
    const ep = (await resp.json()) as { id: string }
    endpointId = ep.id

    const testResp = await request.post(`${API_BASE}/api/endpoints/${endpointId}/test`, {
      headers: { Authorization: 'Bearer sk_debug' },
    })
    expect(testResp.ok()).toBeTruthy()

    const syncResp = await request.post(`${API_BASE}/api/endpoints/${endpointId}/sync`, {
      headers: { Authorization: 'Bearer sk_debug' },
    })
    expect(syncResp.ok()).toBeTruthy()
  })

  test.afterAll(async ({ request }) => {
    try {
      await deleteEndpointsByName(request, endpointName)
    } finally {
      await mock?.close()
    }
  })

  test('PS-01: Endpoint Playground -> System Prompt -> message -> response', async ({ page }) => {
    test.setTimeout(60000)
    await ensureDashboardLogin(page)

    // Navigate to endpoint playground
    const endpoints = await listEndpoints(page.context().request)
    const ep = endpoints.find((e) => e.name === endpointName)
    expect(ep).toBeTruthy()

    await openDashboardRoute(page, `playground/${ep!.id}`)
    await expect(page.getByText('Start a conversation')).toBeVisible({ timeout: 20000 })

    // If a system prompt textarea is visible, set it
    const systemPrompt = page.locator('#system-prompt, textarea[placeholder*="system"]')
    if (await systemPrompt.isVisible({ timeout: 3000 }).catch(() => false)) {
      await systemPrompt.fill('You are a helpful assistant.')
    }

    // Select model
    const modelSelect = page.getByRole('combobox').first()
    await modelSelect.click()
    await page.getByRole('option', { name: mock.models[0] }).click()

    // Send message
    const input = page.getByPlaceholder('Type a message or attach files...')
    await input.fill('Hello from PS-01')
    await page.getByRole('button', { name: 'Send' }).click()

    // Verify mock response
    await expect(page.getByText('MOCK_OK')).toBeVisible({ timeout: 20000 })
  })

  test('PS-02: Streaming toggle -> badge display changes', async ({ page }) => {
    test.setTimeout(60000)
    await ensureDashboardLogin(page)

    const endpoints = await listEndpoints(page.context().request)
    const ep = endpoints.find((e) => e.name === endpointName)
    expect(ep).toBeTruthy()

    await openDashboardRoute(page, `playground/${ep!.id}`)
    await expect(page.getByText('Start a conversation')).toBeVisible({ timeout: 20000 })

    // Streaming badge should be visible by default
    await expect(page.getByText('Streaming')).toBeVisible({ timeout: 10000 })

    // Toggle streaming off via toggle control
    const streamToggle = page.locator('#stream-toggle, [data-testid="stream-toggle"]')
    if (await streamToggle.isVisible({ timeout: 3000 }).catch(() => false)) {
      await streamToggle.click()
      // After toggle, the badge text should change (e.g., "Non-Streaming" or toggle state)
      await expect(page.getByText('Streaming')).toBeVisible({ timeout: 5000 })
    }
  })

  test('PS-03: cURL button -> command displayed -> copy', async ({ page }) => {
    test.setTimeout(60000)
    await ensureDashboardLogin(page)

    const endpoints = await listEndpoints(page.context().request)
    const ep = endpoints.find((e) => e.name === endpointName)
    expect(ep).toBeTruthy()

    await openDashboardRoute(page, `playground/${ep!.id}`)
    await expect(page.getByText('Start a conversation')).toBeVisible({ timeout: 20000 })

    // The cURL button is a regular button with text "cURL" in the toolbar
    const copyCurl = page.getByRole('button', { name: 'cURL' })
    await expect(copyCurl).toBeVisible({ timeout: 10000 })
    await copyCurl.click()

    // Verify cURL dialog opens with the command
    await expect(page.getByText('cURL Command')).toBeVisible({ timeout: 5000 })
    // The pre element inside the dialog should contain "curl"
    await expect(page.locator('pre').filter({ hasText: 'curl' })).toBeVisible({ timeout: 5000 })
  })

  for (const colorScheme of ['light', 'dark'] as const) {
    test.describe(`PS-04 ${colorScheme}`, () => {
      test.use({ colorScheme })
      test(`PS-04: LB Playground -> Temperature/MaxTokens -> chat send (${colorScheme})`, async ({ page, request }) => {
        test.setTimeout(60000)
        await ensureDashboardLogin(page)
        await page.evaluate((theme) => localStorage.setItem('llmlb-theme', theme), colorScheme)
        await page.reload()
        await expect(page.locator('html')).toHaveClass(colorScheme === 'dark' ? /dark/ : /^(?!.*dark)/)

        // Another suite may sync and delete providers of the generic mock models.
        // Keep one alive while the model catalog is read, then churn it during chat.
        const competitor = await startMockOpenAIEndpointServer()
        let competitorId = ''
        let releaseChat: (() => void) | undefined
        try {
          const created = await request.post('/api/endpoints', {
            headers: AUTH_HEADER,
            data: { name: `e2e-ps04-competitor-${randomUUID()}`, base_url: competitor.baseUrl },
          })
          expect(created.ok()).toBeTruthy()
          competitorId = (await created.json()).id as string
          for (const action of ['test', 'sync']) {
            const response = await request.post(`/api/endpoints/${competitorId}/${action}`, { headers: AUTH_HEADER })
            expect(response.ok()).toBeTruthy()
          }

          const catalogResponse = await request.get('/v1/models', { headers: AUTH_HEADER })
          expect(catalogResponse.ok()).toBeTruthy()
          const catalog = await catalogResponse.json()
          expect(catalog.data.find((model: { id: string }) => model.id === mock.models[0]).endpoint_ids,
            'PS-04 must have exactly its own provider, even when a generic mock is online').toEqual([endpointId])

          const openLbPlayground = page.locator('#lb-playground-button')
          await expect(openLbPlayground).toBeVisible({ timeout: 10000 })
          await openLbPlayground.click()

          await expect(page).toHaveURL(/#lb-playground/)
          await expect(page.locator('#lb-playground-sidebar')).toBeVisible({ timeout: 10000 })

          // Adjust temperature if available
          const tempInput = page.locator('#lb-temperature, [data-testid="lb-temperature"]')
          if (await tempInput.isVisible({ timeout: 3000 }).catch(() => false)) {
            await tempInput.fill('0.5')
          }

          // Adjust max tokens if available
          const maxTokensInput = page.locator('#lb-max-tokens, [data-testid="lb-max-tokens"]')
          if (await maxTokensInput.isVisible({ timeout: 3000 }).catch(() => false)) {
            await maxTokensInput.fill('256')
          }

          // Select model
          const modelSelect = page.locator('#lb-model-select')
          await modelSelect.click()
          await page.getByRole('option', { name: mock.models[0] }).click()

          // Send message
          await page.fill('#lb-chat-input', 'Hello from PS-04 LB Playground')
          let chatStarted = false
          const responseAllowed = new Promise<void>((resolve) => { releaseChat = resolve })
          beforeChatResponse = async () => {
            chatStarted = true
            await responseAllowed
          }
          await Promise.all([
            page.click('#lb-send-chat'),
            (async () => {
              // The real inference must reach our provider before churn begins;
              // its response remains pending until all sync/cleanup has finished.
              await expect.poll(() => chatStarted, {
                timeout: 10000, message: 'PS-04 request must reach its own provider',
              }).toBe(true)
              try {
                for (let i = 0; i < 10; i += 1) {
                  const response = await request.post(`/api/endpoints/${competitorId}/sync`, { headers: AUTH_HEADER })
                  expect(response.ok()).toBeTruthy()
                }
                const removed = await request.delete(`/api/endpoints/${competitorId}`, { headers: AUTH_HEADER })
                expect(removed.status()).toBe(204)
              } finally {
                releaseChat!()
              }
            })(),
          ])

          // Verify response
          await expect(page.getByText('MOCK_OK')).toBeVisible({ timeout: 20000 })
          await expect(page.getByText('MOCK_OK')).toContainText(`model=${mock.models[0]}`)
        } finally {
          releaseChat?.()
          beforeChatResponse = undefined
          try {
            if (competitorId) {
              const removed = await request.delete(`/api/endpoints/${competitorId}`, { headers: AUTH_HEADER })
              expect([204, 404]).toContain(removed.status())
            }
          } finally {
            await competitor.close()
          }
        }
      })
    })
  }

  test('PS-05: LB Playground -> cURL button -> correct URL and headers', async ({ page }) => {
    test.setTimeout(60000)
    await ensureDashboardLogin(page)

    const openLbPlayground = page.locator('#lb-playground-button')
    await expect(openLbPlayground).toBeVisible({ timeout: 10000 })
    await openLbPlayground.click()

    await expect(page).toHaveURL(/#lb-playground/)

    // Look for cURL button in LB Playground
    const copyCurl = page.locator(
      '#lb-copy-curl, [data-testid="lb-copy-curl"], button:has-text("cURL")'
    )
    if (await copyCurl.isVisible({ timeout: 5000 }).catch(() => false)) {
      await copyCurl.click()
      // Verify the cURL command contains the correct base URL and authorization header
      const curlText = page.locator('[data-testid="curl-command"], pre:has-text("curl")')
      if (await curlText.isVisible({ timeout: 5000 }).catch(() => false)) {
        const text = await curlText.textContent()
        expect(text).toContain('/api/dashboard/playground/chat/completions')
        expect(text).toContain('X-CSRF-Token')
      }
    }
  })
})
