import { test, expect, type Page } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'
import type { ChatCompletionRequest } from '../../../../src/web/dashboard/src/lib/api/chat'

const FIRST_MODEL = 'alpha-persistence'
const SAVED_MODEL = 'bravo-persistence'
const DB_NAME = 'llmlb-lb-playground'
const STORE_NAME = 'conversations'

interface SavedConversation {
  version: 1
  messages: Array<{
    role: 'user' | 'assistant'
    content: string
    reasoning?: string
    attachments?: Array<{ type: 'image'; name: string; data: string; mimeType: string }>
  }>
  settings: {
    selectedModel: string
    systemPrompt: string
    streamEnabled: boolean
    temperature: number
    maxTokens: number
    useMaxContext: boolean
  }
}

function savedConversation(model = SAVED_MODEL, text = 'Saved user message'): SavedConversation {
  return {
    version: 1,
    messages: [
      { role: 'user', content: text },
      { role: 'assistant', content: 'Saved answer', reasoning: 'Saved reasoning' },
    ],
    settings: {
      selectedModel: model, systemPrompt: '', streamEnabled: true,
      temperature: 0.7, maxTokens: 16384, useMaxContext: false,
    },
  }
}

async function openPlayground(page: Page, model?: string) {
  await page.evaluate((hash) => { window.location.hash = hash },
    model ? `lb-playground?model=${encodeURIComponent(model)}` : 'lb-playground')
  await expect(page.locator('#lb-playground-sidebar')).toBeVisible()
  await expect(page.locator('#lb-model-select')).not.toHaveText('Select a model')
  await expect(page.locator('#lb-chat-input')).toBeEnabled()
}

async function leavePlayground(page: Page) {
  await page.getByRole('button', { name: 'Back to Dashboard', exact: true }).click()
  await expect(page.locator('#theme-toggle')).toBeVisible()
  await expect(page.locator('#lb-playground-sidebar')).toHaveCount(0)
}

async function readStored(page: Page, userId: string): Promise<SavedConversation | null> {
  return page.evaluate(async ({ dbName, storeName, key }) => {
    return new Promise<SavedConversation | null>((resolve, reject) => {
      const opening = indexedDB.open(dbName, 1)
      opening.onupgradeneeded = () => {
        opening.result.createObjectStore(storeName)
      }
      opening.onerror = () => reject(opening.error)
      opening.onsuccess = () => {
        const db = opening.result
        const transaction = db.transaction(storeName, 'readonly')
        const request = transaction.objectStore(storeName).get(key)
        transaction.oncomplete = () => {
          db.close()
          resolve(typeof request.result === 'string' ? JSON.parse(request.result) : null)
        }
        transaction.onerror = () => { db.close(); reject(transaction.error) }
      }
    })
  }, { dbName: DB_NAME, storeName: STORE_NAME, key: `lb:${userId}` })
}

async function seedStored(page: Page, userId: string, serialized: string) {
  await page.evaluate(async ({ dbName, storeName, key, value }) => {
    await new Promise<void>((resolve, reject) => {
      const opening = indexedDB.open(dbName, 1)
      opening.onupgradeneeded = () => {
        opening.result.createObjectStore(storeName)
      }
      opening.onerror = () => reject(opening.error)
      opening.onsuccess = () => {
        const db = opening.result
        const transaction = db.transaction(storeName, 'readwrite')
        transaction.objectStore(storeName).put(value, key)
        transaction.oncomplete = () => { db.close(); resolve() }
        transaction.onerror = () => { db.close(); reject(transaction.error) }
      }
    })
  }, { dbName: DB_NAME, storeName: STORE_NAME, key: `lb:${userId}`, value: serialized })
}

async function prepare(page: Page, colorScheme: 'light' | 'dark') {
  const errors: string[] = []
  page.on('pageerror', (error) => errors.push(`pageerror: ${error.message}`))
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push(`console: ${message.text()}`)
  })
  // Keep actual login/session probing. The controlled data below belongs only
  // to inference/model/history test fixtures, never to authentication.
  const login = await page.context().request.post('/api/auth/login', {
    data: { username: 'admin', password: 'test' },
  })
  expect(login.status()).toBe(200)
  const auth = await page.context().request.get('/api/auth/me')
  expect(auth.status()).toBe(200)
  const { user_id: userId } = await auth.json() as { user_id: string }
  expect(userId).toBeTruthy()

  await page.route('**/api/dashboard/playground/models', (route) => route.fulfill({ json: {
    object: 'list',
    data: [FIRST_MODEL, SAVED_MODEL].map((id) => ({
      id, object: 'model', created: 0, owned_by: 'load balancer',
      lifecycle_status: 'registered', ready: true, max_tokens: 8192,
    })),
  } }))
  await page.route('**/api/dashboard/request-responses*', (route) => {
    if (!new URL(route.request().url()).searchParams.has('offset')) return route.continue()
    return route.fulfill({ json: { records: [], total_count: 0, page: 1, per_page: 100 } })
  })
  const requests: ChatCompletionRequest[] = []
  await page.route('**/api/dashboard/playground/chat/completions', async (route) => {
    expect(route.request().method()).toBe('POST')
    expect(route.request().headers()['x-csrf-token']).toBeTruthy()
    const request = route.request().postDataJSON() as ChatCompletionRequest
    requests.push(request)
    const answer = requests.length === 1 ? 'First persisted answer' : 'Continued answer'
    if (request.stream) {
      await route.fulfill({
        contentType: 'text/event-stream',
        body: `data: ${JSON.stringify({ choices: [{ delta: { reasoning_content: 'Persisted reasoning' } }] })}\n\ndata: ${JSON.stringify({ choices: [{ delta: { content: answer } }] })}\n\ndata: [DONE]\n\n`,
      })
    } else {
      await route.fulfill({ json: { choices: [{ message: { content: answer } }] } })
    }
  })
  await ensureDashboardLogin(page)
  if (colorScheme === 'dark') {
    await expect(page.locator('html')).toHaveClass(/\bdark\b/)
  } else {
    await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)
  }
  return { errors, requests, userId }
}

// route.fulfill buffers complete bodies. An in-browser response stream lets the
// real UI process a partial SSE while the body remains open. Deliberately ignore
// abort in this test transport: late data must also be rejected by request scope.
async function installPartialReply(page: Page) {
  await page.evaluate(() => {
    const originalFetch = window.fetch.bind(window)
    let controller: ReadableStreamDefaultController<Uint8Array>
    const encoder = new TextEncoder()
    let intercepted = false
    const emit = (content: string, reasoning = '') => controller.enqueue(encoder.encode(
      `data: ${JSON.stringify({ choices: [{ delta: { content, reasoning_content: reasoning } }] })}\n\n`,
    ))
    Object.assign(window, { __lbEmitLateDelta: () => emit(' Late stale answer') })
    window.fetch = async (input, init) => {
      if (!intercepted && String(input).endsWith('/api/dashboard/playground/chat/completions')) {
        intercepted = true
        return new Response(new ReadableStream<Uint8Array>({
          start(nextController) {
            controller = nextController
            emit('Partial persisted answer', 'Partial persisted reasoning')
          },
        }), { status: 200, headers: { 'Content-Type': 'text/event-stream' } })
      }
      return originalFetch(input, init)
    }
  })
}

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`LB conversation persistence (${colorScheme}) @playground`, () => {
    test.use({ colorScheme })

    test('restores a 4MB attachment, reasoning and settings and continues its context after reload', async ({ page }, testInfo) => {
      const { errors, requests, userId } = await prepare(page, colorScheme)
      await openPlayground(page)
      await page.locator('#lb-model-select').click()
      await page.getByRole('option', { name: SAVED_MODEL, exact: true }).click()
      await page.getByRole('button', { name: 'Settings', exact: true }).click()
      const settings = page.getByRole('dialog', { name: 'Settings', exact: true })
      await settings.getByPlaceholder('You are a helpful assistant...').fill('Keep this conversation concise.')
      await settings.getByRole('button', { name: 'Precise', exact: true }).click()
      await settings.getByRole('button', { name: 'Done', exact: true }).click()

      // A valid 1px PNG padded to the existing 4MB per-file boundary. Its data
      // URL exceeds localStorage's common limit; it must not be silently dropped.
      const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a5V8AAAAASUVORK5CYII=', 'base64')
      const image = Buffer.alloc(4 * 1024 * 1024)
      png.copy(image)
      await page.locator('input[type=file][accept="image/*"]').setInputFiles({
        name: 'boundary-4mb.png', mimeType: 'image/png', buffer: image,
      })
      await expect(page.getByRole('img', { name: 'boundary-4mb.png' })).toBeVisible()
      await page.locator('#lb-chat-input').fill('Remember this image')
      await page.locator('#lb-send-chat').click()
      await expect(page.getByText('First persisted answer', { exact: true })).toBeVisible()
      await expect(page.locator('#lb-chat-input')).toBeEnabled()
      await expect.poll(async () => (await readStored(page, userId))?.messages.at(-1)?.content).toBe('First persisted answer')
      const stored = (await readStored(page, userId))!
      expect(stored.messages[0].attachments?.[0].data).toBe(`data:image/png;base64,${image.toString('base64')}`)
      expect(stored.messages[1].reasoning).toBe('Persisted reasoning')
      expect(stored.settings).toMatchObject({
        selectedModel: SAVED_MODEL, systemPrompt: 'Keep this conversation concise.',
        streamEnabled: true, temperature: 0.2, maxTokens: 2048, useMaxContext: false,
      })
      expect(Object.keys(stored).sort()).toEqual(['messages', 'settings', 'version'])

      await page.reload()
      await expect(page.locator('#lb-model-select')).toHaveText(SAVED_MODEL)
      await expect(page.getByText('First persisted answer', { exact: true })).toBeVisible()
      const restoredImage = page.getByRole('img', { name: 'boundary-4mb.png' })
      await expect(restoredImage).toBeVisible()
      expect(await restoredImage.evaluate((element) => (element as HTMLImageElement).naturalWidth)).toBe(1)
      await page.getByTestId('reasoning-disclosure').locator('summary').click()
      await expect(page.getByText('Persisted reasoning', { exact: true })).toBeVisible()
      await page.getByRole('button', { name: 'Settings', exact: true }).click()
      await expect(settings.getByPlaceholder('You are a helpful assistant...')).toHaveValue('Keep this conversation concise.')
      await expect(settings.getByRole('slider')).toHaveValue('0.2')
      await expect(settings.getByRole('spinbutton')).toHaveValue('2048')
      await settings.getByRole('button', { name: 'Done', exact: true }).click()
      expect(requests).toHaveLength(1)
      await testInfo.attach(`lb-persistence-restored-${colorScheme}`, {
        body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png',
      })

      await leavePlayground(page)
      await openPlayground(page)
      await expect(page.getByText('First persisted answer', { exact: true })).toBeVisible()
      await page.locator('#lb-chat-input').fill('Continue with the same image')
      await page.locator('#lb-send-chat').click()
      await expect(page.getByText('Continued answer', { exact: true })).toBeVisible()
      expect(requests).toHaveLength(2)
      expect(requests[1]).toMatchObject({
        model: SAVED_MODEL, temperature: 0.2, max_tokens: 2048,
        messages: [
          { role: 'system', content: 'Keep this conversation concise.' },
          { role: 'user', content: [
            { type: 'text', text: 'Remember this image' },
            { type: 'image_url', image_url: { url: stored.messages[0].attachments![0].data } },
          ] },
          { role: 'assistant', content: 'First persisted answer' },
          { role: 'user', content: 'Continue with the same image' },
        ],
      })
      await expect(page.locator('#lb-chat-input')).toBeEnabled()
      await page.getByRole('button', { name: 'Clear Chat', exact: true }).click()
      await expect(page.getByText('Start a load balancer conversation', { exact: true })).toBeVisible()
      await expect.poll(async () => (await readStored(page, userId))?.messages.length ?? 0).toBe(0)
      await page.reload()
      await expect(page.getByText('Start a load balancer conversation', { exact: true })).toBeVisible()
      await expect(page.getByText('First persisted answer', { exact: true })).toHaveCount(0)
      expect(requests).toHaveLength(2)
      expect(errors).toEqual([])
      await testInfo.attach(`lb-persistence-${colorScheme}`, {
        body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png',
      })
    })

    test('keeps user scopes separate and applies route, saved and available model priorities', async ({ page }) => {
      const { errors, requests, userId } = await prepare(page, colorScheme)
      await seedStored(page, 'another-user', JSON.stringify(savedConversation(FIRST_MODEL, 'Other user private conversation')))
      await seedStored(page, userId, JSON.stringify(savedConversation()))
      await openPlayground(page)
      await expect(page.locator('#lb-model-select')).toHaveText(SAVED_MODEL)
      await expect(page.getByText('Saved user message', { exact: true })).toBeVisible()
      await expect(page.getByText('Other user private conversation', { exact: true })).toHaveCount(0)
      await leavePlayground(page)

      await openPlayground(page, FIRST_MODEL)
      await expect(page.locator('#lb-model-select')).toHaveText(FIRST_MODEL)
      await page.locator('#lb-model-select').click()
      await page.getByRole('option', { name: SAVED_MODEL, exact: true }).click()
      await page.getByRole('button', { name: 'Refresh Models', exact: true }).click()
      await expect(page.locator('#lb-model-select')).toHaveText(SAVED_MODEL)
      await leavePlayground(page)

      await seedStored(page, userId, JSON.stringify(savedConversation('retired-model')))
      await openPlayground(page)
      await expect(page.locator('#lb-model-select')).toHaveText(FIRST_MODEL)
      await expect(page.getByText('Saved user message', { exact: true })).toBeVisible()
      expect(requests).toEqual([])
      expect(errors).toEqual([])
    })

    test('restores a partial stream as stopped without automatically sending it again', async ({ page }) => {
      const { errors, requests, userId } = await prepare(page, colorScheme)
      await openPlayground(page)
      await installPartialReply(page)
      await page.locator('#lb-chat-input').fill('Begin a partial answer')
      await page.locator('#lb-send-chat').click()
      await expect(page.getByText('Partial persisted answer', { exact: true })).toBeVisible()
      await expect(page.locator('#lb-stop-chat')).toBeVisible()
      await expect.poll(async () => (await readStored(page, userId))?.messages.at(-1)?.content).toBe('Partial persisted answer')
      await leavePlayground(page)
      await openPlayground(page)
      await expect(page.getByText('Partial persisted answer', { exact: true })).toBeVisible()
      await expect(page.locator('#lb-stop-chat')).toHaveCount(0)
      await expect(page.locator('#lb-chat-input')).toBeEnabled()
      expect(requests).toEqual([])
      await page.getByTestId('reasoning-disclosure').locator('summary').click()
      await expect(page.getByText('Partial persisted reasoning', { exact: true })).toBeVisible()
      await page.locator('#lb-chat-input').fill('Continue after the interruption')
      await page.locator('#lb-send-chat').click()
      await expect(page.getByText('First persisted answer', { exact: true })).toBeVisible()
      expect(requests[0].messages).toEqual([
        { role: 'user', content: 'Begin a partial answer' },
        { role: 'assistant', content: 'Partial persisted answer' },
        { role: 'user', content: 'Continue after the interruption' },
      ])
      expect(errors).toEqual([])
    })

    test('does not resurrect a cleared stream when the old transport delivers another delta', async ({ page }) => {
      const { errors, userId } = await prepare(page, colorScheme)
      await openPlayground(page)
      await installPartialReply(page)
      await page.locator('#lb-chat-input').fill('Clear this running request')
      await page.locator('#lb-send-chat').click()
      await expect(page.getByText('Partial persisted answer', { exact: true })).toBeVisible()
      await page.getByRole('button', { name: 'Clear Chat', exact: true }).click()
      await page.evaluate(() => {
        (window as unknown as { __lbEmitLateDelta: () => void }).__lbEmitLateDelta()
      })
      await expect(page.getByText('Start a load balancer conversation', { exact: true })).toBeVisible()
      await expect(page.getByText(/Late stale answer/)).toHaveCount(0)
      await expect.poll(async () => (await readStored(page, userId))?.messages.length ?? 0).toBe(0)
      await page.reload()
      await expect(page.getByText('Start a load balancer conversation', { exact: true })).toBeVisible()
      await expect(page.getByText(/Partial persisted answer|Late stale answer/)).toHaveCount(0)
      expect(errors).toEqual([])
    })

    test('notifies a storage quota failure while keeping the current conversation usable', async ({ page }) => {
      const { errors, requests } = await prepare(page, colorScheme)
      await openPlayground(page)
      await page.evaluate(() => {
        IDBObjectStore.prototype.put = function () {
          throw new DOMException('Test quota exhausted', 'QuotaExceededError')
        }
      })
      await page.locator('#lb-chat-input').fill('Keep working without persistence')
      await page.locator('#lb-send-chat').click()
      await expect(page.getByText('First persisted answer', { exact: true })).toBeVisible()
      await expect(page.getByText('Conversation not saved', { exact: true })).toBeVisible()
      await expect(page.locator('#lb-chat-input')).toBeEnabled()
      await page.locator('#lb-chat-input').fill('Continue in memory')
      await page.locator('#lb-send-chat').click()
      await expect(page.getByText('Continued answer', { exact: true })).toBeVisible()
      expect(requests[1].messages).toEqual([
        { role: 'user', content: 'Keep working without persistence' },
        { role: 'assistant', content: 'First persisted answer' },
        { role: 'user', content: 'Continue in memory' },
      ])
      expect(errors).toEqual([])
    })

    test('safely rejects malformed JSON and unsupported storage versions', async ({ page }) => {
      const { errors, requests, userId } = await prepare(page, colorScheme)
      for (const serialized of ['{broken-json', JSON.stringify({ ...savedConversation(), version: 999 })]) {
        await seedStored(page, userId, serialized)
        await openPlayground(page)
        await expect(page.getByText('Conversation could not be restored', { exact: true })).toBeVisible()
        await expect(page.getByText('Start a load balancer conversation', { exact: true })).toBeVisible()
        await expect(page.getByText('Saved user message', { exact: true })).toHaveCount(0)
        await expect(page.locator('#lb-chat-input')).toBeEnabled()
        await leavePlayground(page)
      }
      expect(requests).toEqual([])
      expect(errors).toEqual([])
    })
  })
}
