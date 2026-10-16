import { readdir, readFile } from 'node:fs/promises'
import * as path from 'node:path'
import { test as base, expect } from '@playwright/test'

/** Keep first-attempt network and real server evidence alongside the trace. */
export const test = base.extend<{ playgroundDiagnostics: void }>({
  playgroundDiagnostics: [async ({ page, request, baseURL }, use, testInfo) => {
    // Authenticate through the real login endpoint before rendering protected
    // pages, so expected login probes do not become browser console errors.
    const login = await page.context().request.post('/api/auth/login', {
      data: { username: 'admin', password: 'test' },
    })
    expect(login.status()).toBe(200)
    const network: unknown[] = []
    const errors: string[] = []
    const pending: Promise<void>[] = []
    page.on('pageerror', (error) => errors.push(`pageerror: ${error.message}`))
    page.on('console', (message) => {
      if (message.type() === 'error') errors.push(`console: ${message.text()}`)
    })
    page.on('requestfailed', (request) => network.push({
      time: new Date().toISOString(), method: request.method(), url: request.url(), failure: request.failure(),
    }))
    page.on('response', (response) => {
      if (!new URL(response.url()).pathname.startsWith('/api/') &&
          !new URL(response.url()).pathname.startsWith('/v1/')) return
      const entry: Record<string, unknown> = {
        time: new Date().toISOString(), method: response.request().method(), url: response.url(), status: response.status(),
      }
      network.push(entry)
      if (response.url().includes('/playground/chat/completions')) {
        entry.request = response.request().postDataJSON()
        pending.push(response.text().then((body) => { entry.body = body }).catch((error) => {
          entry.bodyError = String(error)
        }))
      }
    })
    try {
      await use()
      expect(errors, 'Playground must have zero console/page errors').toEqual([])
    } finally {
      if (testInfo.status !== testInfo.expectedStatus || errors.length) {
        // Abort unfinished SSE bodies before draining capture promises.
        await page.close()
        await Promise.all(pending)
        const snapshots: Record<string, unknown> = {}
        for (const resource of ['/api/endpoints', '/v1/models']) {
          try {
            const response = await request.get(resource, { headers: { Authorization: 'Bearer sk_debug' } })
            snapshots[resource] = { status: response.status(), body: await response.json() }
          } catch (error) { snapshots[resource] = { error: String(error) } }
        }
        await testInfo.attach('playground-network-and-state', {
          body: JSON.stringify({ retry: testInfo.retry, network, errors, snapshots }, null, 2),
          contentType: 'application/json',
        })
        const port = new URL(baseURL!).port
        const logDir = path.resolve(__dirname, `../.llmlb-${port}/logs`)
        try {
          for (const name of await readdir(logDir)) {
            if (!name.startsWith('llmlb.jsonl')) continue
            const log = await readFile(path.join(logDir, name), 'utf8')
            await testInfo.attach(`server-${name}`, { body: log.slice(-100000), contentType: 'text/plain' })
          }
        } catch (error) {
          await testInfo.attach('server-log-unavailable', { body: String(error), contentType: 'text/plain' })
        }
      }
    }
  }, { auto: true }],
})

export { expect }
