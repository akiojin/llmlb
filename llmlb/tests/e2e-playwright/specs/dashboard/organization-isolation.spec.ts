import { type BrowserContext, type Page } from '@playwright/test'
import { test, expect, OrganizationApi, type OrganizationActor } from '../../helpers/organization.fixture'
import { startMockOpenAIEndpointServer } from '../../helpers/mock-openai-endpoint'

test.use({ trace: 'on' })

function watchErrors(page: Page, errors: string[]): void {
  page.on('pageerror', (error) => errors.push(`pageerror: ${error.message}`))
  page.on('console', (message) => { if (message.type() === 'error') errors.push(`console: ${message.text()}`) })
}

async function createOrganization(page: Page, api: OrganizationApi, name: string): Promise<{ id: string; name: string }> {
  const created = page.waitForResponse((response) => new URL(response.url()).pathname === '/api/organizations' && response.request().method() === 'POST')
  await page.getByLabel('New organization name', { exact: true }).fill(name)
  await page.getByRole('button', { name: 'Create organization', exact: true }).click()
  const response = await created
  expect(response.status()).toBe(201)
  const organization = await response.json()
  api.organizationIds.add(organization.id)
  await expect(page.getByRole('button', { name, exact: true })).toBeVisible()
  return organization
}

async function assignThroughUI(page: Page, actor: OrganizationActor, organizationId: string, role: 'admin' | 'member'): Promise<void> {
  await page.getByRole('button', { name: `Edit membership for ${actor.username}`, exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Edit membership', exact: true })
  await dialog.getByLabel('Organization', { exact: true }).selectOption(organizationId)
  await dialog.getByLabel('Organization role', { exact: true }).selectOption(role)
  const saved = page.waitForResponse((response) => new URL(response.url()).pathname === `/api/users/${actor.id}/membership` && response.request().method() === 'PUT')
  await dialog.getByRole('button', { name: 'Save membership', exact: true }).click()
  expect((await saved).status()).toBe(200)
  await expect(dialog).not.toBeVisible()
}

async function infer(api: OrganizationApi, token: string, model: string, marker: string, options: { organizationId?: string; jwt?: boolean } = {}): Promise<void> {
  const response = await api.request.post(options.jwt ? '/api/dashboard/playground/chat/completions' : '/v1/chat/completions', {
    headers: { Authorization: `Bearer ${token}` },
    data: { model, messages: [{ role: 'user', content: marker }], organization_id: options.organizationId,
      request_id: 'a-client-id-is-not-the-logical-request-uuid' },
  })
  expect(response.status()).toBe(200)
  expect((await response.json()).choices[0].message.content).toContain(marker)
}

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Organization isolation (${colorScheme}) @dashboard @organizations`, () => {
    test.use({ colorScheme, viewport: { width: 1440, height: 1050 } })

    test('real organization management, scoped usage, account/key details and export in both themes', async ({ page, request, browser, baseURL, browserErrors }, testInfo) => {
      test.setTimeout(180_000)
      const api = new OrganizationApi(request)
      const contexts: BrowserContext[] = []
      const roleErrors: string[] = []
      const model = `${api.prefix}-model`
      const upstream = await startMockOpenAIEndpointServer({ models: [model] })
      try {
        await api.initialize()
        await api.browserLogin(page)
        await api.openOrganizations(page)
        await page.mouse.move(0, 0)
        await expect(page.getByRole('heading', { name: 'Organizations & usage', exact: true })).toBeVisible()
        if (colorScheme === 'dark') await expect(page.locator('html')).toHaveClass(/\bdark\b/)
        else await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)
        const alpha = await createOrganization(page, api, `${api.prefix}-Alpha`)
        const beta = await createOrganization(page, api, `${api.prefix}-Beta`)
        const aAdmin = await api.actor('a-admin')
        const alice = await api.actor('alice')
        const peer = await api.actor('a-peer')
        const bAdmin = await api.actor('b-admin')
        const bob = await api.actor('bob')
        await page.getByRole('button', { name: 'Refresh', exact: true }).click()
        await assignThroughUI(page, aAdmin, alpha.id, 'admin')
        await assignThroughUI(page, alice, alpha.id, 'member')
        await assignThroughUI(page, peer, alpha.id, 'member')
        await assignThroughUI(page, bAdmin, beta.id, 'admin')
        await assignThroughUI(page, bob, beta.id, 'member')
        await page.getByRole('button', { name: alpha.name, exact: true }).click()
        const renamed = `${alpha.name}-renamed`
        await page.getByLabel('Organization name', { exact: true }).fill(renamed)
        const rename = page.waitForResponse((response) => new URL(response.url()).pathname === `/api/organizations/${alpha.id}` && response.request().method() === 'PUT')
        await page.getByRole('button', { name: 'Save name', exact: true }).click()
        expect((await rename).status()).toBe(200)
        await expect(page.getByRole('button', { name: renamed, exact: true })).toBeVisible()
        const refusedArchive = await request.delete(`/api/organizations/${alpha.id}`, { headers: api.headers })
        expect(refusedArchive.status()).toBe(409)
        await expect(page.getByRole('button', { name: 'Archive organization', exact: true })).toBeDisabled()

        const uiKeyName = `${api.prefix}-ui-key`
        await page.getByRole('button', { name: `Issue API key for ${alice.username}`, exact: true }).click()
        const dialog = page.getByRole('dialog', { name: 'Issue API key', exact: true })
        await dialog.getByLabel('Key name', { exact: true }).fill(uiKeyName)
        const issued = page.waitForResponse((response) => new URL(response.url()).pathname === '/api/me/api-keys' && response.request().method() === 'POST')
        await dialog.getByRole('button', { name: 'Issue key', exact: true }).click()
        const keyResponse = await issued
        expect(keyResponse.status()).toBe(201)
        const keyId = (await keyResponse.json()).id
        api.keyIds.add(keyId)
        const uiKey = await dialog.getByLabel('API key (shown once)', { exact: true }).inputValue()
        expect(uiKey.length).toBeGreaterThan(16)
        await dialog.getByRole('button', { name: 'Done', exact: true }).click()
        await expect(dialog).not.toBeVisible()
        await expect(page.locator('body')).not.toContainText(uiKey)
        const secondKey = await api.key(alice, alpha.id, 'second-key')
        const publicKeys = await request.get('/api/me/api-keys', { headers: api.headers })
        expect(publicKeys.status()).toBe(200)
        const metadata = (await publicKeys.json()).api_keys.find((key: { id: string }) => key.id === keyId)
        expect(metadata).toMatchObject({ created_by: api.adminId, usage_account_id: alice.id, organization_id: alpha.id })
        expect(metadata.created_by).not.toBe(metadata.usage_account_id)
        await api.endpoint(upstream.baseUrl, model)
        await infer(api, uiKey, model, 'alice-key-1', { organizationId: beta.id })
        await infer(api, secondKey.key, model, 'alice-key-2', { organizationId: beta.id })
        await infer(api, alice.token, model, 'alice-jwt', { organizationId: beta.id, jwt: true })
        await infer(api, peer.token, model, 'peer-jwt', { jwt: true })
        await infer(api, bob.token, model, 'bob-jwt', { organizationId: alpha.id, jwt: true })
        await expect.poll(async () => (await api.usage(api.token, alpha.id)).totals.request_count).toBe(4)
        expect((await api.usage(api.token, beta.id)).totals.request_count).toBe(1)
        expect((await api.usage(api.token, alpha.id, alice.id)).totals.request_count).toBe(3)
        const aHistory = await api.records(aAdmin.token, model)
        const ownHistory = await api.records(alice.token, model)
        const bHistory = await api.records(bAdmin.token, model)
        expect(aHistory.total_count).toBe(4)
        expect(ownHistory.total_count).toBe(3)
        expect(bHistory.total_count).toBe(1)
        expect(new Set(aHistory.records.map((record) => record.id)).size).toBe(4)
        const ownIds = new Set(ownHistory.records.map((record) => record.id))
        const peerRecord = aHistory.records.find((record) => !ownIds.has(record.id))!
        expect((await request.get(`/api/dashboard/request-responses/${bHistory.records[0].id}`, { headers: api.actorHeaders(aAdmin) })).status()).toBe(404)
        expect((await request.get(`/api/dashboard/request-responses/${peerRecord.id}`, { headers: api.actorHeaders(alice) })).status()).toBe(404)
        expect((await request.get(`/api/dashboard/request-responses/${ownHistory.records[0].id}`, { headers: api.actorHeaders(alice) })).status()).toBe(200)
        const foreignFilter = await request.get(`/api/dashboard/usage?organization_id=${beta.id}`, { headers: api.actorHeaders(aAdmin) })
        expect([403, 404]).toContain(foreignFilter.status())
        const apiKeyRead = await request.get('/api/dashboard/usage', { headers: { Authorization: `Bearer ${uiKey}` } })
        expect([401, 403]).toContain(apiKeyRead.status())
        expect((await request.post('/api/organizations', { headers: api.actorHeaders(aAdmin), data: { name: 'must-not-create' } })).status()).toBe(403)
        const csv = await request.get(`/api/dashboard/request-responses/export?format=csv&model=${encodeURIComponent(model)}`, { headers: api.actorHeaders(aAdmin) })
        expect(csv.status()).toBe(200)
        expect(csv.headers()['cache-control']).toContain('no-store')
        const csvText = await csv.text()
        for (const record of aHistory.records) expect(csvText).toContain(record.id)
        expect(csvText).not.toContain(bHistory.records[0].id)
        const pageTwo = await request.get('/api/dashboard/usage?group_by=account&per_page=1&page=2', { headers: api.actorHeaders(aAdmin) })
        expect(pageTwo.status()).toBe(200)
        const paged = await pageTwo.json()
        expect(paged.rows).toHaveLength(1)
        expect(paged.totals.request_count).toBe(4)
        expect(paged.total).toBe(2)

        await page.getByRole('button', { name: 'Refresh', exact: true }).click()
        await page.getByRole('button', { name: `View account ${alice.username}`, exact: true }).click()
        await expect(page.getByRole('heading', { name: `Account detail · ${alice.username}`, exact: true })).toBeVisible()
        await expect(page.getByRole('table').first().getByRole('row').filter({ hasText: alice.username }).getByRole('cell').nth(1)).toHaveText('3')
        await expect(page.getByText(/Monthly quota is not available/i)).toBeVisible()
        await testInfo.attach(`organization-account-${colorScheme}`, { body: await page.screenshot({ fullPage: true }), contentType: 'image/png' })
        await page.setViewportSize({ width: 390, height: 844 })
        await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
        await expect(page.getByLabel('From (UTC, inclusive)')).toBeVisible()
        await testInfo.attach(`organization-narrow-${colorScheme}`, { body: await page.screenshot({ fullPage: true }), contentType: 'image/png' })
        await page.setViewportSize({ width: 1440, height: 1050 })
        await page.getByRole('link', { name: 'Requests, details & export', exact: true }).click()
        await expect(page.getByRole('tab', { name: 'Requests', exact: true })).toHaveAttribute('aria-selected', 'true')
        await page.getByRole('cell', { name: model, exact: true }).first().click()
        const requestDetail = page.getByRole('dialog', { name: 'Request Details', exact: true })
        await expect(requestDetail).toBeVisible()
        await requestDetail.getByRole('tab', { name: 'Request', exact: true }).click()
        await expect(requestDetail.locator('pre')).toContainText(model)
        await testInfo.attach(`organization-request-detail-${colorScheme}`, { body: await page.screenshot({ fullPage: true }), contentType: 'image/png' })
        await requestDetail.getByRole('button', { name: 'Close', exact: true }).click()

        const actors: Array<[OrganizationActor, string, 'admin' | 'member', string[]]> = [
          [aAdmin, alpha.id, 'admin', [aAdmin.id, alice.id, peer.id]], [alice, alpha.id, 'member', [alice.id]],
          [bAdmin, beta.id, 'admin', [bAdmin.id, bob.id]], [bob, beta.id, 'member', [bob.id]],
        ]
        for (const [actor, organizationId, role, expectedIds] of actors) {
          const identity = await request.get('/api/auth/me', { headers: api.actorHeaders(actor) })
          expect(await identity.json()).toMatchObject({ organization_id: organizationId, organization_role: role, role: 'viewer' })
          const users = await request.get('/api/users', { headers: api.actorHeaders(actor) })
          expect(users.status()).toBe(200)
          expect((await users.json()).users.map((item: { id: string }) => item.id).sort()).toEqual([...expectedIds].sort())
          const context = await browser.newContext({ baseURL, colorScheme, viewport: { width: 1280, height: 900 } })
          contexts.push(context)
          const rolePage = await context.newPage()
          watchErrors(rolePage, roleErrors)
          await api.browserLogin(rolePage, actor)
          await api.openOrganizations(rolePage)
          await expect(rolePage.getByText(role === 'admin' ? 'Your organization' : 'Your account', { exact: true })).toBeVisible()
          await expect(rolePage.getByLabel('New organization name')).toHaveCount(0)
          await expect(rolePage.getByRole('button', { name: `View account ${actor.username}`, exact: true })).toBeVisible()
          const foreignName = organizationId === alpha.id ? bob.username : alice.username
          await expect(rolePage.getByRole('button', { name: `View account ${foreignName}`, exact: true })).toHaveCount(0)
          await expect(rolePage.getByRole('button', { name: /Edit membership for/ })).toHaveCount(0)
          if (actor.id === alice.id) {
            await rolePage.goto('/dashboard/')
            await rolePage.locator('#api-keys-button').click()
            const keyModal = rolePage.getByRole('dialog', { name: 'API Keys', exact: true })
            await expect(keyModal.getByRole('cell', { name: uiKeyName, exact: true })).toBeVisible()
            await expect(keyModal.getByRole('button', { name: `Delete ${uiKeyName}`, exact: true })).toBeDisabled()
          }
          await context.close()
          contexts.pop()
        }
        expect(roleErrors).toEqual([])
        expect(browserErrors).toEqual([])
      } finally {
        for (const context of contexts) await context.close()
        await page.close()
        try { await api.cleanup() } finally { await upstream.close() }
      }
    })

    test('settles an accepted stream in its original organization after transfer and preserves deleted-account usage', async ({ page, request, browserErrors }) => {
      test.setTimeout(180_000)
      const api = new OrganizationApi(request)
      let releaseStream!: () => void
      let streamStarted!: () => void
      const held = new Promise<void>((resolve) => { releaseStream = resolve })
      const started = new Promise<void>((resolve) => { streamStarted = resolve })
      const model = `${api.prefix}-transfer`
      const upstream = await startMockOpenAIEndpointServer({ models: [model], afterFirstChatChunk: async () => { streamStarted(); await held } })
      try {
        await api.initialize()
        const alpha = await api.createOrganization('stream-A')
        const beta = await api.createOrganization('stream-B')
        const mover = await api.actor('mover')
        await api.assign(mover, alpha.id)
        const key = await api.key(mover, alpha.id, 'old-org-key')
        await api.endpoint(upstream.baseUrl, model)
        const stream = request.post('/v1/chat/completions', {
          headers: { Authorization: `Bearer ${key.key}` }, data: { model, stream: true, messages: [{ role: 'user', content: 'accepted-in-A' }] },
        })
        try {
          await Promise.race([started, stream.then((response) => { throw new Error(`stream ended before transfer: ${response.status()}`) })])
          await api.assign(mover, beta.id)
          const oldKey = await request.get('/v1/models', { headers: { Authorization: `Bearer ${key.key}` } })
          expect([401, 403]).toContain(oldKey.status())
          const identity = await request.get('/api/auth/me', { headers: api.actorHeaders(mover) })
          expect(identity.status()).toBe(200)
          expect(await identity.json()).toMatchObject({ organization_id: beta.id, organization_role: 'member' })
          await infer(api, mover.token, model, 'after-transfer-B', { jwt: true })
        } finally { releaseStream() }
        const completed = await stream
        expect(completed.status()).toBe(200)
        expect(await completed.text()).toContain('[DONE]')
        await expect.poll(async () => (await api.usage(api.token, alpha.id, mover.id)).totals.request_count).toBe(1)
        expect((await api.usage(api.token, beta.id, mover.id)).totals.request_count).toBe(1)
        const selfHistory = await api.records(mover.token, model)
        expect(selfHistory.total_count).toBe(1)
        expect((await api.usage(mover.token)).totals.request_count).toBe(1)
        expect((await request.delete(`/api/users/${mover.id}`, { headers: api.headers })).status()).toBe(204)
        api.actors.splice(api.actors.indexOf(mover), 1)
        expect((await api.usage(api.token, alpha.id, mover.id)).totals.request_count).toBe(1)
        expect((await api.usage(api.token, beta.id, mover.id)).totals.request_count).toBe(1)
        expect((await request.delete(`/api/organizations/${alpha.id}`, { headers: api.headers })).status()).toBe(200)
        api.organizationIds.delete(alpha.id)
        const deleted = await api.usage(api.token, alpha.id, mover.id)
        expect(deleted.rows[0]).toMatchObject({ account_deleted: true, organization_archived: true, request_count: 1 })
        await api.browserLogin(page)
        await api.openOrganizations(page)
        await page.getByLabel('Group by').selectOption('account')
        await expect(page.getByRole('table').first().getByText('Deleted account', { exact: true }).first()).toBeVisible()
        expect(browserErrors).toEqual([])
      } finally {
        releaseStream()
        await page.close()
        try { await api.cleanup() } finally { await upstream.close() }
      }
    })
  })
}
