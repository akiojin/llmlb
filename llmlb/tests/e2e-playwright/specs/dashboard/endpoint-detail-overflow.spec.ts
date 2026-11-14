import { test, expect, type Page, type Locator } from '@playwright/test'
import { ensureDashboardLogin } from '../../helpers/api-helpers'

const ID = '94300000-1111-2222-3333-444444444444'
const MODEL = 'organization/Strata-very-long-model-name-with-context-and-quantization-Q4_K_M-131072'
const NAME = 'Strata endpoint with a very long display name for regression coverage'

async function openDetail(page: Page, empty = false, shrink = false) {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  page.on('console', message => { if (message.type() === 'error') errors.push(message.text()) })
  const login = await page.request.post('/api/auth/login', { data: { username: 'admin', password: 'test' } })
  expect(login.ok()).toBe(true)
  await page.routeWebSocket('**/ws/dashboard', () => {})
  const endpoint = {
    id: ID, name: NAME, base_url: 'http://192.0.2.94:8080/long-endpoint-path',
    status: 'online', endpoint_type: 'strata', health_check_interval_secs: 30,
    inference_timeout_secs: 120, error_count: 0, registered_at: '2026-10-01T00:00:00Z',
    model_count: 1, total_requests: 100, successful_requests: 79, failed_requests: 21,
    latency_ms: 42, notes: 'preserve these notes',
  }
  await page.route('**/api/dashboard/endpoints', route => route.fulfill({ json: [endpoint] }))
  await page.route(`**/api/endpoints/${ID}/models`, route => route.fulfill({ json: {
    endpoint_id: ID, models: [{ model_id: MODEL, max_tokens: 131072 }],
  } }))
  await page.route(`**/api/endpoints/${ID}/model-stats`, route => route.fulfill({ json: [
    { model_id: MODEL, total_requests: 100, successful_requests: 79, failed_requests: 21 },
  ] }))
  await page.route(`**/api/endpoints/${ID}/model-tps`, route => route.fulfill({ json: [
    { model_id: MODEL, api_kind: 'chat_completions', source: 'production', tps: 12.3,
      request_count: 5, total_output_tokens: 100, average_duration_ms: 1250 },
  ] }))
  await page.route(`**/api/endpoints/${ID}/today-stats`, route => route.fulfill({ json: {
    date: '2026-10-26', total_requests: 4, successful_requests: 3, failed_requests: 1,
  } }))
  await page.route(`**/api/endpoints/${ID}/daily-stats*`, route => route.fulfill({ json: [] }))
  const saves: unknown[] = []
  await page.route(`**/api/endpoints/${ID}`, route => {
    expect(route.request().method()).toBe('PUT')
    saves.push(route.request().postDataJSON())
    return route.fulfill({ json: endpoint })
  })
  for (const action of ['test', 'sync']) {
    await page.route(`**/api/endpoints/${ID}/${action}`, route => route.fulfill({ json:
      action === 'test' ? { success: true, latency_ms: 42 } : { synced_models: 1 },
    }))
  }
  const failureQueries: URL[] = []
  await page.route('**/api/dashboard/request-responses*', route => {
    const url = new URL(route.request().url())
    if (url.searchParams.has('endpoint_id')) {
      failureQueries.push(url)
      expect(url.searchParams.get('endpoint_id')).toBe(ID)
      expect(url.searchParams.get('status')).toBe('error')
      expect(url.searchParams.has('model')).toBe(false)
      if (empty) return route.fulfill({ json: { records: [], total_count: 0, page: 1, per_page: 25 } })
      if (shrink && url.searchParams.get('page') === '2') {
        return route.fulfill({ json: { records: [], total_count: 1, page: 2, per_page: 25 } })
      }
      return route.fulfill({ json: { records: [{
        id: 'failure-943', timestamp: '2026-10-25T12:34:56Z', model: MODEL,
        endpoint_id: ID, endpoint_name: NAME, duration_ms: 120000,
        request_type: 'chat_completions',
        status: { type: 'error', message: 'Recorded upstream timeout after 120 seconds' },
        request_body: { model: MODEL }, response_body: { error: 'Recorded upstream timeout' },
      }], total_count: shrink ? 26 : 1, page: 1, per_page: 25 } })
    }
    return route.fulfill({ json: { records: [], total_count: 0, page: 1, per_page: 100 } })
  })
  await ensureDashboardLogin(page)
  // Mobile tabs hide their text; the selected panel has no accessible name.
  const panel = page.getByRole('tabpanel')
  const row = panel.getByRole('row').filter({ has: page.getByRole('cell', { name: NAME, exact: true }) })
  const trigger = row.getByRole('button', { name: 'Details', exact: true })
  await trigger.click()
  const dialog = page.getByRole('dialog', { name: NAME, exact: true })
  await expect(dialog.getByRole('cell', { name: '79.0%', exact: true })).toBeVisible()
  return { dialog, trigger, saves, errors, failureQueries }
}

async function insideWidth(locator: Locator, container: Locator) {
  const outer = await container.boundingBox()
  expect(outer).not.toBeNull()
  for (const element of await locator.all()) {
    const box = await element.boundingBox()
    expect(box).not.toBeNull()
    expect(box!.x).toBeGreaterThanOrEqual(outer!.x - 1)
    expect(box!.x + box!.width).toBeLessThanOrEqual(outer!.x + outer!.width + 1)
  }
}

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Endpoint Detail regression ${colorScheme} @dashboard`, () => {
    test.use({ colorScheme })
    for (const [width, height] of [[1920, 1032], [1280, 720], [375, 720]]) {
      test(`all columns and form edges remain reachable at ${width}x${height} (#943)`, async ({ page }, testInfo) => {
        await page.setViewportSize({ width, height })
        const { dialog, trigger, saves, errors } = await openDetail(page)
        await testInfo.attach('initial-layout', { body: await page.screenshot(), contentType: 'image/png' })
        const measurements = await dialog.evaluate(el => ({
          clientWidth: el.clientWidth, scrollWidth: el.scrollWidth,
          descendants: [...el.querySelectorAll('input, textarea, button')].map(e => ({
            text: e.textContent, x: e.getBoundingClientRect().x, width: e.getBoundingClientRect().width,
          })),
        }))
        await testInfo.attach('geometry', { body: JSON.stringify(measurements, null, 2), contentType: 'application/json' })
        expect(measurements.scrollWidth).toBeLessThanOrEqual(measurements.clientWidth + 1)
        await insideWidth(dialog.locator('input, textarea, button'), dialog)
        const scroll = dialog.getByRole('region', { name: 'Endpoint models', exact: true })
        await scroll.scrollIntoViewIfNeeded()
        await scroll.focus()
        const size = await scroll.evaluate(el => ({ client: el.clientWidth, total: el.scrollWidth }))
        if (size.total > size.client) {
          await page.keyboard.press('ArrowRight')
          await expect.poll(() => scroll.evaluate(el => el.scrollLeft)).toBeGreaterThan(0)
        }
        const lastColumn = scroll.getByRole('columnheader', { name: 'Avg Duration' })
        await lastColumn.scrollIntoViewIfNeeded()
        await insideWidth(lastColumn, scroll)
        await expect(scroll.getByRole('cell', { name: '1.3 s' })).toBeVisible()
        await testInfo.attach('models-visible', { body: await page.screenshot(), contentType: 'image/png' })
        await expect(dialog.getByLabel('Notes', { exact: true })).toHaveValue('preserve these notes')
        await page.keyboard.press('Escape')
        await expect(dialog).not.toBeVisible()
        await expect(trigger).toBeFocused()
        await trigger.click()
        await dialog.getByLabel('Display Name', { exact: true }).fill('Renamed Strata')
        await dialog.getByLabel('Notes', { exact: true }).fill('updated notes')
        await dialog.getByRole('button', { name: 'Save', exact: true }).click()
        await expect.poll(() => saves).toEqual([{ name: 'Renamed Strata', notes: 'updated notes' }])
        await testInfo.attach('form-visible', { body: await page.screenshot(), contentType: 'image/png' })
        await dialog.getByRole('button', { name: 'Close', exact: true }).first().click()
        await expect(dialog).not.toBeVisible()
        expect(errors).toEqual([])
      })
    }

    test('explains lifetime counters and opens recorded endpoint failures with keyboard (#943)', async ({ page }, testInfo) => {
      await page.setViewportSize({ width: 375, height: 720 })
      const { dialog, errors, failureQueries } = await openDetail(page)
      expect(failureQueries).toHaveLength(0)
      await expect(dialog.getByText(/79 successful \/ 100 total/)).toBeVisible()
      await expect(dialog.getByText(/Red: below 80%.*Warning: below 95%/).first()).toBeVisible()
      await expect(dialog.getByText('Consecutive health/status errors:', { exact: true })).toBeVisible()
      await expect(dialog.getByText(/not the cumulative number of failed inference requests/)).toBeVisible()
      const show = dialog.getByRole('button', { name: 'View failed requests', exact: true })
      await show.focus()
      await page.keyboard.press('Enter')
      const history = dialog.getByRole('region', { name: 'Failed requests', exact: true })
      await expect(history.getByText(/retention/)).toBeVisible()
      await expect(history.getByRole('combobox')).toHaveCount(0)
      const details = history.getByRole('button', { name: /View request details/ }).first()
      const bodyWidth = await dialog.getByRole('region', { name: 'Endpoint details', exact: true }).evaluate(el => ({ client: el.clientWidth, total: el.scrollWidth }))
      expect(bodyWidth.total).toBeLessThanOrEqual(bodyWidth.client + 1)
      await details.focus()
      await page.keyboard.press('Enter')
      const request = page.getByRole('dialog', { name: 'Request Details', exact: true })
      await expect(request.getByText('Recorded upstream timeout after 120 seconds', { exact: true })).toBeVisible()
      await expect(request.getByText('Timestamp', { exact: true })).toBeVisible()
      await expect(request.locator('time')).toHaveAttribute('datetime', '2026-10-25T12:34:56Z')
      await insideWidth(request.getByText('Recorded upstream timeout after 120 seconds', { exact: true }), request)
      await request.getByText('Recorded upstream timeout after 120 seconds', { exact: true }).scrollIntoViewIfNeeded()
      await testInfo.attach('recorded-error', { body: await page.screenshot(), contentType: 'image/png' })
      await page.keyboard.press('Escape')
      await expect(request).not.toBeVisible()
      await expect(details).toBeFocused()
      expect(failureQueries.length).toBeGreaterThan(0)
      expect(errors).toEqual([])
    })

    test('explains missing retained records without inventing a cause (#943)', async ({ page }) => {
      const { dialog, errors } = await openDetail(page, true)
      await dialog.getByRole('button', { name: 'View failed requests', exact: true }).click()
      const history = dialog.getByRole('region', { name: 'Failed requests', exact: true })
      await expect(history.getByRole('status')).toHaveText('No matching failures in retained history. The cause cannot be determined from the success rate alone.')
      await expect(history.getByText(/older records may have been deleted by retention/)).toBeVisible()
      await expect(history.getByRole('table')).toHaveCount(0)
      expect(errors).toEqual([])
    })

    test('can return to earlier failures after retention shrinks the current page (#943)', async ({ page }) => {
      const { dialog, errors } = await openDetail(page, false, true)
      await dialog.getByRole('button', { name: 'View failed requests', exact: true }).click()
      const history = dialog.getByRole('region', { name: 'Failed requests', exact: true })
      await history.getByRole('button', { name: 'Next failures', exact: true }).click()
      await expect(history.getByRole('status')).toContainText('No failures on this page.')
      await history.getByRole('button', { name: 'Previous failures', exact: true }).click()
      await expect(history.getByRole('button', { name: /View request details/ })).toBeVisible()
      expect(errors).toEqual([])
    })
  })
}
