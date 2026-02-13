import { test, expect } from '@playwright/test'
import { ensureDashboardLogin, createUser, deleteUser, listUsers } from '../../helpers/api-helpers'

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768'

test.describe('User Management @dashboard', () => {
  const testUsers: string[] = [] // Track created user IDs for cleanup

  test.afterEach(async ({ request }) => {
    // Clean up test users
    for (const userId of testUsers) {
      await deleteUser(request, userId).catch(() => {})
    }
    testUsers.length = 0
  })

  test('UM-01: admin creates user -> appears in list', async ({ page, request }) => {
    const username = `e2e-user-${Date.now()}`
    await ensureDashboardLogin(page)

    // Create user via API
    const user = await createUser(request, username, 'testpass123', 'viewer')
    testUsers.push(user.id)

    // Verify user appears in API list
    const users = await listUsers(request)
    const found = users.find((u) => u.username === username)
    expect(found).toBeTruthy()
    expect(found?.role).toBe('viewer')
  })

  test('UM-02: role change -> verified via API', async ({ page, request }) => {
    const username = `e2e-role-${Date.now()}`
    const user = await createUser(request, username, 'testpass123', 'viewer')
    testUsers.push(user.id)

    await ensureDashboardLogin(page)

    // Update role via API
    const resp = await request.put(`${API_BASE}/api/users/${encodeURIComponent(user.id)}`, {
      headers: { Authorization: 'Bearer sk_debug', 'Content-Type': 'application/json' },
      data: { role: 'admin' },
    })
    expect(resp.ok()).toBeTruthy()

    // Verify role change
    const users = await listUsers(request)
    const updated = users.find((u) => u.username === username)
    expect(updated?.role).toBe('admin')
  })

  test('UM-03: delete user -> login fails', async ({ page, request }) => {
    const username = `e2e-del-${Date.now()}`
    const user = await createUser(request, username, 'testpass123', 'viewer')

    // Delete user via API
    await deleteUser(request, user.id)

    // Try to login with deleted user credentials
    await page.goto(`${API_BASE}/dashboard`)
    await page.waitForLoadState('networkidle')

    // Fill login form
    await page.fill('#username', username)
    await page.fill('#password', 'testpass123')
    await page.click('button[type="submit"]')

    // Should show error (toast or inline message)
    const errorIndicator = page.locator('[data-sonner-toast], [role="status"], .toast').first()
    await expect(errorIndicator).toBeVisible({ timeout: 5000 })
  })

  test('UM-04: duplicate username -> error', async ({ request }) => {
    const username = `e2e-dup-${Date.now()}`
    const user = await createUser(request, username, 'testpass123', 'viewer')
    testUsers.push(user.id)

    // Attempt to create user with same username
    const resp = await request.post(`${API_BASE}/api/users`, {
      headers: { Authorization: 'Bearer sk_debug', 'Content-Type': 'application/json' },
      data: { username, password: 'testpass456', role: 'user' },
    })
    expect(resp.ok()).toBeFalsy()
  })

  test('UM-05: unauthenticated request -> 401', async ({ request }) => {
    // Access /api/users without authentication -> should return 401
    const resp = await request.get(`${API_BASE}/api/users`)
    expect(resp.status()).toBe(401)
  })
})
