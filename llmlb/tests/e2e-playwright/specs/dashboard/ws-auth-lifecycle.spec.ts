import { type Page } from '@playwright/test'
import { test, expect } from '../../helpers/isolated-registration.fixture'
import { REGISTRATION_PASSWORD } from '../../helpers/registration.fixture'

type SocketPhase = 'viewer' | 'admin' | 'logged-out'
interface SocketEvidence {
  phase: SocketPhase
  cookiePresent?: boolean
  authorizationPresent?: boolean
  status?: number
}

async function cookiePresence(page: Page) {
  const cookies = await page.context().cookies()
  return {
    jwt: cookies.some((cookie) => cookie.name === 'llmlb_jwt'),
    csrf: cookies.some((cookie) => cookie.name === 'llmlb_csrf'),
  }
}

async function signOut(page: Page) {
  await page.getByRole('button', { name: 'User menu', exact: true }).click()
  const responsePromise = page.waitForResponse((response) =>
    new URL(response.url()).pathname === '/api/auth/logout' && response.request().method() === 'POST')
  await page.getByRole('menuitem', { name: /sign out/i }).click()
  const response = await responsePromise
  expect(response.status()).toBe(204)
  // The application can issue two competing redirects; assert the settled page.
  await expect(page).toHaveURL(/\/dashboard\/login\.html$/)
  await expect(page.locator('#username')).toBeVisible()
  expect(await cookiePresence(page)).toEqual({ jwt: false, csrf: false })
}

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Dashboard socket authentication lifecycle (${colorScheme}) @auth @dashboard`, () => {
    test.use({ colorScheme })

    test('viewer logout opens no socket, while a new admin session connects and reconnects', async ({
      page, registrationApi, browserErrors,
    }, testInfo) => {
      test.setTimeout(60_000)
      const username = registrationApi.username('ws-viewer')
      const pending = await registrationApi.submit(username)
      const approved = await registrationApi.request.post(`/api/registration-requests/${pending.id}/approve`, {
        headers: registrationApi.headers,
        data: { role: 'viewer' },
      })
      expect(approved.status()).toBe(201)

      // Log in through the real server before loading the app. This avoids an
      // expected unauthenticated startup probe without filtering browser errors.
      const viewerLogin = await page.request.post('/api/auth/login', {
        data: { username, password: REGISTRATION_PASSWORD },
      })
      expect(viewerLogin.status()).toBe(200)
      expect(await cookiePresence(page)).toEqual({ jwt: true, csrf: true })

      let phase: SocketPhase = 'viewer'
      let socketStarts = 0
      const sockets = new Map<string, SocketEvidence>()
      const cdp = await page.context().newCDPSession(page)
      await cdp.send('Network.enable')
      cdp.on('Network.webSocketCreated', (event) => {
        if (new URL(event.url).pathname === '/ws/dashboard') {
          sockets.set(event.requestId, { phase })
        }
      })
      cdp.on('Network.webSocketWillSendHandshakeRequest', (event) => {
        const evidence = sockets.get(event.requestId)
        if (!evidence) return
        // Persist presence only: neither credentials nor raw headers enter the
        // test's evidence attachment.
        const names = Object.keys(event.request.headers).map((name) => name.toLowerCase())
        evidence.cookiePresent = names.includes('cookie')
        evidence.authorizationPresent = names.includes('authorization')
      })
      cdp.on('Network.webSocketHandshakeResponseReceived', (event) => {
        const evidence = sockets.get(event.requestId)
        if (evidence) evidence.status = event.response.status
      })
      page.on('websocket', (socket) => {
        if (new URL(socket.url()).pathname === '/ws/dashboard') socketStarts += 1
      })
      await page.addInitScript(() => {
        const NativeWebSocket = window.WebSocket
        window.WebSocket = class extends NativeWebSocket {
          constructor(url: string | URL, protocols?: string | string[]) {
            super(url, protocols)
            if (new URL(url, location.href).pathname === '/ws/dashboard') {
              Reflect.set(window, 'dashboardLifecycleSocket', this)
            }
          }
        }
      })

      await page.goto('/dashboard/')
      await expect(page.locator('#theme-toggle')).toBeVisible()
      await expect(page.locator('html')).toHaveClass(colorScheme === 'dark' ? /\bdark\b/ : /^(?!.*\bdark\b)/)
      const viewerMe = await page.request.get('/api/auth/me')
      expect(viewerMe.status()).toBe(200)
      expect(await viewerMe.json()).toMatchObject({ username, role: 'viewer', must_change_password: false })
      expect(socketStarts).toBe(0)
      expect(sockets.size).toBe(0)

      await signOut(page)
      phase = 'logged-out'
      expect(socketStarts, 'viewer logout and the settled unauthenticated page must start no socket').toBe(0)
      expect(sockets.size).toBe(0)
      await testInfo.attach(`viewer-logout-${colorScheme}`, {
        body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png',
      })

      phase = 'admin'
      await page.locator('#username').fill('admin')
      await page.locator('#password').fill('test')
      await page.locator('button[type="submit"]').click()
      await expect(page).not.toHaveURL(/\/login/)
      await expect(page.locator('#theme-toggle')).toBeVisible()
      const adminUpgrades = () => [...sockets.values()].filter((socket) => socket.phase === 'admin' && socket.status === 101)
      await expect.poll(() => adminUpgrades().length, { message: 'the new admin session must complete a real WS101 upgrade' })
        .toBeGreaterThanOrEqual(1)
      expect(adminUpgrades()).toEqual(expect.arrayContaining([
        expect.objectContaining({ cookiePresent: true, authorizationPresent: false, status: 101 }),
      ]))

      const upgradesBeforeClose = adminUpgrades().length
      const closed = await page.evaluate(() => {
        const socket = Reflect.get(window, 'dashboardLifecycleSocket') as WebSocket | undefined
        if (socket?.readyState !== WebSocket.OPEN) return false
        // Exercise the hook's ordinary backoff using a real, clean close.
        socket.close(1000, 'lifecycle regression')
        return true
      })
      expect(closed).toBe(true)
      await expect.poll(() => adminUpgrades().length, { timeout: 10_000, message: 'an authenticated admin must reconnect after a normal close' })
        .toBeGreaterThan(upgradesBeforeClose)
      await testInfo.attach(`admin-reconnected-${colorScheme}`, {
        body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png',
      })

      const startsBeforeAdminLogout = socketStarts
      await signOut(page)
      phase = 'logged-out'
      expect(socketStarts, 'admin logout must not replace the socket after credentials are cleared').toBe(startsBeforeAdminLogout)
      expect([...sockets.values()].filter((socket) => socket.phase !== 'admin')).toEqual([])
      expect(browserErrors, 'the entire lifecycle must have zero console/page errors').toEqual([])
      await testInfo.attach(`socket-lifecycle-evidence-${colorScheme}`, {
        body: JSON.stringify({
          viewerLogoutStatus: 204,
          viewerCookiesAfterLogout: { jwt: false, csrf: false },
          viewerSocketStarts: 0,
          sockets: [...sockets.values()],
          adminLogoutStatus: 204,
          adminCookiesAfterLogout: await cookiePresence(page),
          consoleAndPageErrors: browserErrors,
        }),
        contentType: 'application/json',
      })
      await cdp.detach()
    })
  })
}
