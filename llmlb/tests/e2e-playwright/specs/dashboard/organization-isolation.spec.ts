import { test, expect } from '../../helpers/isolated-registration.fixture'
import { REGISTRATION_PASSWORD } from '../../helpers/registration.fixture'
import { startMockOpenAIEndpointServer } from '../../helpers/mock-openai-endpoint'
import { LoginPage } from '../../pages/auth.page'

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Organization isolation (${colorScheme}) @dashboard @organizations`, () => {
    test.use({ colorScheme })

    test('manages organizations and proves scoped usage, details, export, keys and session changes on a real server', async ({ page, browser, baseURL, registrationApi }, testInfo) => {
      test.setTimeout(180000)
      const api = registrationApi.request
      const headers = registrationApi.headers
      const suffix = registrationApi.prefix
      const nameA = `Research A ${suffix}`
      const nameB = `Research B ${suffix}`
      const model = `organization-model-${suffix}`
      const upstream = await startMockOpenAIEndpointServer({ endpointType: 'xllm', models: [model],
        tokenUsage: { prompt_tokens: 11, completion_tokens: 7, total_tokens: 18 } })
      const contexts: Awaited<ReturnType<typeof browser.newContext>>[] = []
      try {
        const endpointResponse = await api.post('/api/endpoints', { headers,
          data: { name: `organization-endpoint-${suffix}`, base_url: upstream.baseUrl, health_check_interval_secs: 10 } })
        expect(endpointResponse.status()).toBe(201)
        const endpoint = await endpointResponse.json()
        await expect.poll(async () => {
          const response = await api.get('/api/endpoints', { headers })
          const endpoints = await response.json()
          const rows = Array.isArray(endpoints) ? endpoints : endpoints.endpoints
          return rows.find((item: { id: string }) => item.id === endpoint.id)?.status
        }, { timeout: 45000 }).toBe('online')

        const login = new LoginPage(page)
        await login.goto()
        // Prepare the form away from hover-translated controls in headed windows.
        await page.mouse.move(0, 0)
        await login.login('admin', 'test')
        await login.waitForDashboard()
        if (colorScheme === 'dark') await expect(page.locator('html')).toHaveClass(/\bdark\b/)
        else await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)
        await page.getByRole('button', { name: 'Organizations', exact: true }).click()
        const organizations = page.getByRole('dialog', { name: 'Organizations', exact: true })
        await organizations.getByLabel('Organization name', { exact: true }).fill(nameA)
        await organizations.getByRole('button', { name: 'Create organization', exact: true }).click()
        await expect(organizations.getByRole('heading', { name: nameA, exact: true })).toBeVisible()
        const orgs = await (await api.get('/api/organizations', { headers })).json()
        const orgA = orgs.organizations.find((org: { name: string }) => org.name === nameA)
        const bResponse = await api.post('/api/organizations', { headers, data: { name: nameB } })
        expect(bResponse.status()).toBe(201)
        const orgB = await bResponse.json()

        const account = async (label: string, org: string, role: 'admin' | 'member') => {
          const username = registrationApi.username(label)
          const pending = await registrationApi.submit(username)
          const approved = await api.post(`/api/registration-requests/${pending.id}/approve`, { headers, data: { role: 'viewer' } })
          expect(approved.status()).toBe(201)
          const user = (await registrationApi.users()).find((user) => user.username === username)!
          const assigned = await api.put(`/api/users/${user.id}/membership`, { headers,
            data: { organization_id: org, organization_role: role } })
          expect(assigned.status()).toBe(200)
          const loggedIn = await api.post('/api/auth/login', { data: { username, password: REGISTRATION_PASSWORD } })
          expect(loggedIn.status()).toBe(200)
          return { ...user, token: (await loggedIn.json()).token as string }
        }
        const adminA = await account('org-admin-a', orgA.id, 'admin')
        const memberA = await account('org-member-a', orgA.id, 'member')
        const memberB = await account('org-member-b', orgB.id, 'member')
        const issue = async (id: string, org: string) => {
          const response = await api.post('/api/me/api-keys', { headers, data: {
            name: `inference-${id}`, usage_account_id: id, organization_id: org,
            permissions: ['openai.inference', 'openai.models.read'],
          } })
          expect(response.status()).toBe(201)
          return (await response.json()).key as string
        }
        const keyA = await issue(adminA.id, orgA.id)
        const keyB = await issue(memberB.id, orgB.id)
        const baseline = await (await api.get('/api/dashboard/usage?group_by=organization', { headers })).json()
        for (const key of [keyA, keyB]) {
          const response = await api.post('/v1/chat/completions', { headers: { Authorization: `Bearer ${key}` },
            data: { model, messages: [{ role: 'user', content: `private-${key === keyA ? 'A' : 'B'}` }] } })
          expect(response.status()).toBe(200)
        }
        const memberResponse = await api.post('/api/dashboard/playground/chat/completions', {
          headers: { Authorization: `Bearer ${memberA.token}` },
          data: { model, messages: [{ role: 'user', content: 'private-member-A' }] },
        })
        expect(memberResponse.status()).toBe(200)
        await expect.poll(async () => {
          const response = await api.get('/api/dashboard/usage?group_by=organization', { headers })
          const { totals } = await response.json()
          return {
            requests: totals.request_count, measured: totals.measured_requests,
            tokens: totals.total_tokens, missing: totals.missing_requests,
          }
        }).toEqual({
          requests: baseline.totals.request_count + 3, measured: baseline.totals.measured_requests + 3,
          tokens: baseline.totals.total_tokens + 54, missing: baseline.totals.missing_requests,
        })
        await organizations.getByRole('button', { name: 'Close', exact: true }).click()
        await page.getByRole('tab', { name: 'Usage', exact: true }).click()
        const usage = page.getByRole('region', { name: 'Organization and account usage' })
        await expect(usage.getByRole('button', { name: nameA, exact: true })).toBeVisible()
        await expect(usage.getByRole('button', { name: nameB, exact: true })).toBeVisible()
        await expect(usage.getByText(`${baseline.totals.measured_requests + 3} measured`, { exact: true })).toBeVisible()
        await page.setViewportSize({ width: 390, height: 844 })
        await testInfo.attach(`organizations-usage-narrow-${colorScheme}`, { body: await page.screenshot({ fullPage: true }), contentType: 'image/png' })
        await page.setViewportSize({ width: 1280, height: 900 })

        const contextA = await browser.newContext({ baseURL, colorScheme })
        contexts.push(contextA)
        const pageA = await contextA.newPage()
        const errors: string[] = []
        pageA.on('pageerror', (error) => errors.push(error.message))
        pageA.on('console', (message) => { if (message.type() === 'error') errors.push(message.text()) })
        const loginA = new LoginPage(pageA)
        await loginA.goto()
        await pageA.mouse.move(0, 0)
        await loginA.login(adminA.username, REGISTRATION_PASSWORD)
        await loginA.waitForDashboard()
        await expect(pageA.getByRole('tab', { name: 'Usage', exact: true })).toBeVisible()
        await expect(pageA.getByRole('tab', { name: 'System', exact: true })).toHaveCount(0)
        const scopedUsage = pageA.getByRole('region', { name: 'Organization and account usage' })
        await expect(scopedUsage.getByRole('button', { name: adminA.username, exact: true })).toBeVisible()
        await expect(scopedUsage.getByRole('button', { name: memberA.username, exact: true })).toBeVisible()
        await expect(pageA.getByText(memberB.username, { exact: true })).toHaveCount(0)
        const scoped = { Authorization: `Bearer ${adminA.token}` }
        const scopedHistory = await (await api.get('/api/dashboard/request-responses', { headers: scoped })).json()
        expect(scopedHistory.total_count).toBe(2)
        const globalHistory = await (await api.get('/api/dashboard/request-responses', { headers })).json()
        const otherRecord = globalHistory.records.find((record: { request_body: unknown }) => JSON.stringify(record.request_body).includes('private-B'))
        expect(otherRecord).toBeDefined()
        expect((await api.get(`/api/dashboard/request-responses/${otherRecord.id}`, { headers: scoped })).status()).toBe(404)
        const exported = await api.get('/api/dashboard/request-responses/export?format=csv', { headers: scoped })
        expect(exported.status()).toBe(200)
        expect(await exported.text()).not.toContain('private-B')
        expect((await api.get(`/api/dashboard/usage?organization_id=${orgB.id}`, { headers: scoped })).status()).toBe(403)
        expect((await api.get('/api/dashboard/usage', { headers: { Authorization: `Bearer ${keyA}` } })).status()).toBe(401)
        const self = await (await api.get('/api/dashboard/request-responses', { headers: { Authorization: `Bearer ${memberA.token}` } })).json()
        expect(self.total_count).toBe(1)
        await pageA.getByRole('tab', { name: 'Requests', exact: true }).click()
        await expect(pageA.getByText(model, { exact: true }).first()).toBeVisible()
        await testInfo.attach(`organization-requests-${colorScheme}`, { body: await pageA.screenshot({ fullPage: true }), contentType: 'image/png' })
        const moved = await api.put(`/api/users/${adminA.id}/membership`, { headers,
          data: { organization_id: orgB.id, organization_role: 'admin' } })
        expect(moved.status()).toBe(200)
        expect((await api.post('/v1/chat/completions', { headers: { Authorization: `Bearer ${keyA}` },
          data: { model, messages: [{ role: 'user', content: 'must-not-run' }] } })).status()).toBe(401)
        const movedUsage = await (await api.get('/api/dashboard/usage', { headers: scoped })).json()
        expect(movedUsage.totals.request_count).toBe(1)
        expect(movedUsage.rows.every((row: { organization_id: string }) => row.organization_id === orgB.id)).toBe(true)
        await expect(pageA.getByRole('region', { name: 'Organization and account usage' })).toBeVisible()
        await expect(pageA.getByRole('region', { name: 'Organization and account usage' }).getByRole('button', { name: memberB.username, exact: true })).toBeVisible()
        await expect(pageA.getByText(memberA.username, { exact: true })).toHaveCount(0)
        expect(errors, 'organization page has zero console/page errors').toEqual([])
      } finally {
        await Promise.all(contexts.map((context) => context.close()))
        await upstream.close()
      }
    })
  })
}
