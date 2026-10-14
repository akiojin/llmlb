import { randomUUID } from 'node:crypto'
import { test as base, expect, type APIRequestContext } from '@playwright/test'

export const REGISTRATION_PASSWORD = 'RegistrationPass123'

export interface PendingRegistration {
  id: string
  username: string
  created_at: string
}

export interface RegistrationUser {
  id: string
  username: string
  role: 'admin' | 'viewer'
}

/** All setup, assertions and cleanup reach the real HTTP server. */
export class RegistrationApi {
  readonly prefix = `e2e-registration-${randomUUID()}`

  constructor(readonly request: APIRequestContext, readonly token: string) {}

  username(label: string): string {
    return `${this.prefix}-${label}@example.com`
  }

  get headers() {
    return { Authorization: `Bearer ${this.token}` }
  }

  async pending(): Promise<PendingRegistration[]> {
    const response = await this.request.get('/api/registration-requests', { headers: this.headers })
    expect(response.status()).toBe(200)
    const requests = await response.json()
    expect(Array.isArray(requests)).toBe(true)
    return requests
  }

  async users(): Promise<RegistrationUser[]> {
    const response = await this.request.get('/api/users', { headers: this.headers })
    expect(response.status()).toBe(200)
    return (await response.json()).users
  }

  async submit(username: string): Promise<PendingRegistration> {
    const response = await this.request.post('/api/auth/register', {
      data: { username, password: REGISTRATION_PASSWORD },
    })
    expect(response.status()).toBe(202)
    expect(await response.json()).toMatchObject({ status: 'pending', message: expect.any(String) })
    const pending = (await this.pending()).find((item) => item.username === username)
    expect(pending).toBeDefined()
    return pending!
  }

  async expectCannotLogin(username: string): Promise<void> {
    // APIRequestContext checks rejection without generating browser resource errors.
    const response = await this.request.post('/api/auth/login', {
      data: { username, password: REGISTRATION_PASSWORD },
    })
    expect(response.status()).toBe(401)
    expect(await response.json()).not.toHaveProperty('token')
  }

  async cleanup(): Promise<void> {
    for (const item of (await this.pending()).filter((item) => item.username.startsWith(this.prefix))) {
      const response = await this.request.delete(`/api/registration-requests/${item.id}`, { headers: this.headers })
      expect(response.status()).toBe(204)
    }
    for (const user of (await this.users()).filter((user) => user.username.startsWith(this.prefix))) {
      const response = await this.request.delete(`/api/users/${user.id}`, { headers: this.headers })
      expect(response.status()).toBe(204)
    }
  }
}

export const test = base.extend<{ registrationApi: RegistrationApi; browserErrors: string[] }>({
  registrationApi: async ({ request, page }, use) => {
    const login = await request.post('/api/auth/login', { data: { username: 'admin', password: 'test' } })
    expect(login.status()).toBe(200)
    const api = new RegistrationApi(request, (await login.json()).token)
    await use(api)
    // Stop polling before deleting a signed-in test administrator (#844).
    await page.close()
    await api.cleanup()
  },
  browserErrors: [async ({ page }, use) => {
    const errors: string[] = []
    page.on('pageerror', (error) => errors.push(`pageerror: ${error.message}`))
    page.on('console', (message) => {
      if (message.type() === 'error') errors.push(`console: ${message.text()}`)
    })
    await use(errors)
    expect(errors, 'registration pages must have zero console/page errors').toEqual([])
  }, { auto: true }],
})

export { expect }
