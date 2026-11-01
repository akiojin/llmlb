import { expect, test } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'

test.use({ trace: 'on' })

for (const colorScheme of ['dark', 'light'] as const) {
  test.describe(`Inference defaults (${colorScheme}) @dashboard`, () => {
    test.use({ colorScheme, viewport: { width: 375, height: 740 } })

    test('preserves saved IDs and handles save, clear and failed decoding without overflow (Issue #950)', async ({ page }, testInfo) => {
      const errors: string[] = []
      page.on('console', (message) => {
        if (message.type() === 'error') errors.push(message.text())
      })
      page.on('pageerror', (error) => errors.push(error.message))
      // Authentication is real. Only configuration/catalog data is controlled.
      const login = await page.request.post('/api/auth/login', {
        data: { username: 'admin', password: 'test' },
      })
      expect(login.ok()).toBe(true)
      const removed = `removed/${'model-name-'.repeat(12)}`
      let savedModel: string | null = removed
      const payloads: unknown[] = []
      let failDecode = false
      let releaseSave!: () => void
      let saveGate: Promise<void> | undefined = new Promise((resolve) => { releaseSave = resolve })
      await page.route('**/api/dashboard/models?*', (route) => route.fulfill({ json: {
        object: 'list',
        data: [{
          id: 'available-text-model', object: 'model', owned_by: 'router', created: 0,
          lifecycle_status: 'registered', ready: false,
          capabilities: {
            chat_completion: true, completion: true, embeddings: false, fine_tune: false,
            inference: true, text_to_speech: false, speech_to_text: false, image_generation: false,
          },
        }],
      } }))
      await page.route('**/api/dashboard/inference-defaults', async (route) => {
        if (route.request().method() === 'GET') {
          await route.fulfill({ json: { default_text_model: savedModel } })
          return
        }
        expect(route.request().method()).toBe('PUT')
        expect(route.request().headers()['x-csrf-token']).toBeTruthy()
        const payload = route.request().postDataJSON()
        payloads.push(payload)
        await saveGate
        if (failDecode) {
          // A malformed response must not be treated as a committed save.
          // Returning 200 avoids an intentional HTTP error in the zero-console check.
          await route.fulfill({ contentType: 'application/json', body: 'invalid-json' })
          return
        }
        savedModel = payload.default_text_model
        await route.fulfill({ json: { default_text_model: savedModel } })
      })

      await ensureDashboardLogin(page)
      if (colorScheme === 'dark') await expect(page.locator('html')).toHaveClass(/\bdark\b/)
      else await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)
      await page.getByRole('button', { name: 'Settings', exact: true }).click()
      const dialog = page.getByRole('dialog', { name: 'Settings', exact: true })
      await dialog.getByRole('tab', { name: 'Inference', exact: true }).click()
      const model = dialog.getByLabel('Default text model', { exact: true })
      await expect(model).toHaveValue(removed)
      await expect(dialog.getByText(/Saved model is unavailable/)).toBeVisible()
      await expect(dialog.getByRole('button', { name: 'Save', exact: true })).toBeDisabled()
      await page.setViewportSize({ width: 320, height: 740 })
      expect(await dialog.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true)
      await testInfo.attach(`inference-unavailable-${colorScheme}`, {
        body: await page.screenshot(), contentType: 'image/png',
      })

      await model.selectOption('available-text-model')
      await expect(dialog.getByText('Unsaved changes', { exact: true })).toBeVisible()
      await dialog.getByRole('button', { name: 'Save', exact: true }).click()
      await expect(model).toBeDisabled()
      await expect(dialog.getByRole('button', { name: 'Saving...', exact: true })).toBeDisabled()
      await expect(dialog.getByText(/Saved\. Applies to new requests/)).toHaveCount(0)
      releaseSave()
      saveGate = undefined
      await expect(dialog.getByText('Saved. Applies to new requests that omit model.')).toBeVisible()
      expect(payloads).toEqual([{ default_text_model: 'available-text-model' }])

      await model.selectOption('')
      await dialog.getByRole('button', { name: 'Save', exact: true }).click()
      await expect(dialog.getByText('Saved. Applies to new requests that omit model.')).toBeVisible()
      expect(payloads[1]).toEqual({ default_text_model: null })
      await expect(model).toHaveValue('')

      failDecode = true
      await model.selectOption('available-text-model')
      await dialog.getByRole('button', { name: 'Save', exact: true }).click()
      await expect(dialog.getByRole('alert')).toContainText('Failed to save inference defaults:')
      await expect(dialog.getByText(/Saved\. Applies to new requests/)).toHaveCount(0)
      await expect(model).toHaveValue('available-text-model')
      expect(savedModel).toBe(null)
      await expect(dialog.getByRole('button', { name: 'Save', exact: true })).toBeEnabled()
      expect(await dialog.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true)
      await testInfo.attach(`inference-save-error-${colorScheme}`, {
        body: await page.screenshot(), contentType: 'image/png',
      })
      expect(errors).toEqual([])
    })
  })
}
