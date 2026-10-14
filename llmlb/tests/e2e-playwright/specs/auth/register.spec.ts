import { test, expect, REGISTRATION_PASSWORD } from '../../helpers/registration.fixture'
import { RegisterPage } from '../../pages/auth.page'

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Registration request page (${colorScheme}) @auth @dashboard`, () => {
    test.use({ colorScheme })

    test.beforeEach(async ({ page }) => {
      await new RegisterPage(page).goto()
      await expect(page.locator('html')).toHaveClass(colorScheme === 'dark' ? /\bdark\b/ : /^(?!.*\bdark\b)/)
    })

    test('R-01: email registration needs no code and remains pending until approval', async ({ page, registrationApi }, testInfo) => {
      const form = new RegisterPage(page)
      const username = registrationApi.username('pending')
      await expect(page).toHaveTitle(/Register.*LLM Load Balancer/)
      await expect(page.getByLabel('Email', { exact: true })).toBeVisible()
      await expect(page.getByLabel('Password', { exact: true })).toBeVisible()
      await expect(page.getByLabel('Confirm Password', { exact: true })).toBeVisible()
      await expect(page.locator('#invitation-code')).toHaveCount(0)
      await expect(page.getByRole('button', { name: 'Request registration', exact: true })).toBeVisible()
      const responsePromise = page.waitForResponse((response) =>
        new URL(response.url()).pathname === '/api/auth/register' && response.request().method() === 'POST')
      await form.register(username, REGISTRATION_PASSWORD)
      const response = await responsePromise
      expect(response.status()).toBe(202)
      expect(response.request().postDataJSON()).toEqual({ username, password: REGISTRATION_PASSWORD })
      expect(await response.json()).toMatchObject({ status: 'pending', message: expect.any(String) })
      await form.waitForSuccess()
      await expect(page.getByText('Awaiting administrator approval.', { exact: true })).toBeVisible()
      await expect(page).toHaveURL(/register\.html/)
      expect((await registrationApi.pending()).map((item) => item.username)).toContain(username)
      expect((await registrationApi.users()).map((user) => user.username)).not.toContain(username)
      await registrationApi.expectCannotLogin(username)
      const duplicate = await registrationApi.request.post('/api/auth/register', {
        data: { username, password: REGISTRATION_PASSWORD },
      })
      expect(duplicate.status()).toBe(409)
      await testInfo.attach(`registration-pending-${colorScheme}`, {
        body: await page.screenshot({ animations: 'disabled' }), contentType: 'image/png',
      })
      await page.getByRole('button', { name: 'Back to Sign In', exact: true }).click()
      await expect(page).toHaveURL(/login\.html/)
      await expect(page.locator('#username')).toBeVisible()
    })

    test('R-02: password mismatch keeps the request form and does not submit', async ({ page }) => {
      const requests: string[] = []
      page.on('request', (request) => {
        if (new URL(request.url()).pathname === '/api/auth/register') requests.push(request.method())
      })
      const form = new RegisterPage(page)
      await form.usernameInput.fill('mismatch@example.com')
      await form.passwordInput.fill(REGISTRATION_PASSWORD)
      await form.confirmPasswordInput.fill('DifferentPass123')
      await form.submitButton.click()
      await expect(page.getByText('Passwords do not match', { exact: true })).toBeVisible()
      await expect(form.submitButton).toBeVisible()
      expect(requests).toEqual([])
    })

    test('R-03: sign-in link returns to login', async ({ page }) => {
      await page.getByRole('link', { name: 'Sign in', exact: true }).click()
      await expect(page).toHaveURL(/login\.html/)
    })
  })
}
