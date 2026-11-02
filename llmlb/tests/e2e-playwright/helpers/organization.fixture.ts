import { randomUUID } from 'node:crypto'
import { expect, type APIRequestContext, type Page } from '@playwright/test'
export { test, expect } from './isolated-registration.fixture'

export interface OrganizationActor {
  id: string
  username: string
  password: string
  token: string
}

/** This fixture uses only the isolated worker server and real public APIs. */
export class OrganizationApi {
  readonly prefix = `org-isolation-${randomUUID()}`
  readonly actors: OrganizationActor[] = []
  readonly organizationIds = new Set<string>()
  readonly keyIds = new Set<string>()
  readonly endpointIds = new Set<string>()
  token = ''
  adminId = ''

  constructor(readonly request: APIRequestContext) {}
  get headers() { return { Authorization: `Bearer ${this.token}` } }
  actorHeaders(actor: OrganizationActor) { return { Authorization: `Bearer ${actor.token}` } }

  async initialize(): Promise<void> {
    const login = await this.request.post('/api/auth/login', { data: { username: 'admin', password: 'test' } })
    expect(login.status()).toBe(200)
    this.token = (await login.json()).token
    const identity = await this.request.get('/api/auth/me', { headers: this.headers })
    expect(identity.status(), await identity.text()).toBe(200)
    this.adminId = (await identity.json()).user_id
  }

  async createOrganization(label: string): Promise<{ id: string; name: string }> {
    const response = await this.request.post('/api/organizations', { headers: this.headers, data: { name: `${this.prefix}-${label}` } })
    expect(response.status()).toBe(201)
    const organization = await response.json()
    this.organizationIds.add(organization.id)
    return organization
  }

  async actor(label: string): Promise<OrganizationActor> {
    const username = `${this.prefix}-${label}@example.com`
    const created = await this.request.post('/api/users', { headers: this.headers, data: { username, role: 'viewer' } })
    expect(created.status()).toBe(201)
    const body = await created.json()
    const actor = { id: body.user.id as string, username, password: '', token: '' }
    this.actors.push(actor)
    const login = await this.request.post('/api/auth/login', { data: { username, password: body.generated_password } })
    expect(login.status()).toBe(200)
    const password = `OrganizationPass-${randomUUID()}!`
    const changed = await this.request.put('/api/auth/change-password', {
      headers: { Authorization: `Bearer ${(await login.json()).token}` },
      data: { current_password: body.generated_password, new_password: password },
    })
    expect(changed.status()).toBe(200)
    const signedIn = await this.request.post('/api/auth/login', { data: { username, password } })
    expect(signedIn.status()).toBe(200)
    actor.password = password
    actor.token = (await signedIn.json()).token
    return actor
  }

  async assign(actor: OrganizationActor, organizationId: string, role: 'admin' | 'member' = 'member'): Promise<void> {
    const response = await this.request.put(`/api/users/${actor.id}/membership`, {
      headers: this.headers, data: { organization_id: organizationId, organization_role: role },
    })
    expect(response.status()).toBe(200)
  }

  async key(actor: OrganizationActor, organizationId: string, label: string): Promise<{ id: string; key: string }> {
    const response = await this.request.post('/api/me/api-keys', {
      headers: this.headers, data: { name: `${this.prefix}-${label}`, usage_account_id: actor.id, organization_id: organizationId,
        permissions: ['openai.inference', 'openai.models.read'] },
    })
    expect(response.status()).toBe(201)
    const key = await response.json()
    this.keyIds.add(key.id)
    return key
  }

  async endpoint(baseUrl: string, model: string): Promise<void> {
    const created = await this.request.post('/api/endpoints', { headers: this.headers, data: { name: model, base_url: baseUrl } })
    expect(created.status()).toBe(201)
    const id = (await created.json()).id
    this.endpointIds.add(id)
    for (const action of ['test', 'sync']) {
      const response = await this.request.post(`/api/endpoints/${id}/${action}`, { headers: this.headers })
      expect(response.ok()).toBe(true)
    }
    await expect.poll(async () => {
      const response = await this.request.get('/v1/models', { headers: { Authorization: 'Bearer sk_debug' } })
      expect(response.ok()).toBe(true)
      return (await response.json()).data.map((item: { id: string }) => item.id)
    }).toContain(model)
  }

  async usage(token: string, organizationId?: string, accountId?: string) {
    const query = new URLSearchParams({ group_by: 'account', per_page: '1000' })
    if (organizationId) query.set('organization_id', organizationId)
    if (accountId) query.set('account_id', accountId)
    const response = await this.request.get(`/api/dashboard/usage?${query}`, { headers: { Authorization: `Bearer ${token}` } })
    expect(response.status()).toBe(200)
    expect(response.headers()['cache-control']).toContain('no-store')
    expect(response.headers()['x-llmlb-scope']).toBeTruthy()
    return response.json()
  }

  async records(token: string, model: string) {
    const response = await this.request.get(`/api/dashboard/request-responses?per_page=100&model=${encodeURIComponent(model)}`, {
      headers: { Authorization: `Bearer ${token}` },
    })
    expect(response.status()).toBe(200)
    expect(response.headers()['cache-control']).toContain('no-store')
    return response.json() as Promise<{ records: Array<{ id: string; organization_id?: string; account_id?: string }>; total_count: number }>
  }

  async browserLogin(page: Page, actor?: OrganizationActor): Promise<void> {
    const response = await page.context().request.post('/api/auth/login', { data: actor
      ? { username: actor.username, password: actor.password } : { username: 'admin', password: 'test' } })
    expect(response.status()).toBe(200)
  }

  async openOrganizations(page: Page): Promise<void> {
    await page.goto('/dashboard/')
    await expect(page.locator('#theme-toggle')).toBeVisible()
    await page.mouse.move(0, 0)
    await page.getByRole('button', { name: 'User menu', exact: true }).click()
    await page.getByRole('menuitem', { name: 'Organizations & usage', exact: true }).click()
    await expect(page).toHaveURL(/\/dashboard\/#organizations$/)
    await expect(page.getByRole('heading', { name: 'Organizations & usage', exact: true })).toBeVisible()
  }

  async cleanup(): Promise<void> {
    for (const id of this.keyIds) expect([200, 204, 404]).toContain((await this.request.delete(`/api/me/api-keys/${id}`, { headers: this.headers })).status())
    for (const actor of this.actors) expect([204, 404]).toContain((await this.request.delete(`/api/users/${actor.id}`, { headers: this.headers })).status())
    for (const id of this.endpointIds) expect([200, 204, 404]).toContain((await this.request.delete(`/api/endpoints/${id}`, { headers: this.headers })).status())
    for (const id of this.organizationIds) expect([200, 404]).toContain((await this.request.delete(`/api/organizations/${id}`, { headers: this.headers })).status())
  }
}
