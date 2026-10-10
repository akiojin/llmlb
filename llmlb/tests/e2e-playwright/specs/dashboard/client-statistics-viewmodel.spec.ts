import { test, expect, type Locator, type Route } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import { ensureDashboardLogin } from '../../helpers/api-helpers'

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Client, statistics and logs ViewModels (${colorScheme}) @dashboard`, () => {
    test.use({ colorScheme })

    test('preserves drilldown ownership, threshold refresh, periods and log polling (#870)', async ({ page }, testInfo) => {
      test.setTimeout(90000)
      const errors: string[] = []
      page.on('pageerror', (error) => errors.push(error.message))
      page.on('console', (message) => {
        if (message.type() === 'error') errors.push(message.text())
      })
      const login = await page.request.post('/api/auth/login', {
        data: { username: 'admin', password: 'test' },
      })
      expect(login.ok()).toBe(true)
      // Real authentication and CSRF remain active; only the five surfaces' data is controlled.
      await page.routeWebSocket('**/ws/dashboard', () => {})
      const gets = { ranking: 0, detail: 0, apiKeys: 0, daily: 0, monthly: 0, logs: 0 }
      const rankingPages: string[] = []
      let threshold = '100'
      let saveRoute: Route | undefined
      const ip = '192.0.2.10'
      const lastSeen = '2026-10-01T12:00:00Z'
      await page.route(/\/api\/dashboard\/clients(?:\?.*)?$/, (route) => {
        const url = new URL(route.request().url())
        expect(url.searchParams.get('per_page')).toBe('20')
        expect(url.searchParams.get('ip')).toBeNull()
        gets.ranking++
        rankingPages.push(url.searchParams.get('page')!)
        return route.fulfill({ json: {
          rankings: [{ ip, request_count: 1234, last_seen: lastSeen, is_alert: threshold === '100', api_key_count: 1 }],
          total_count: 21, page: Number(url.searchParams.get('page')), per_page: 20,
        } })
      })
      await page.route('**/api/dashboard/clients/timeline', (route) => route.fulfill({ json: [
        { hour: lastSeen, unique_ips: 1 },
      ] }))
      await page.route('**/api/dashboard/clients/models', (route) => route.fulfill({ json: [
        { model: 'client-vm-model', request_count: 1234, percentage: 100 },
      ] }))
      await page.route('**/api/dashboard/clients/heatmap*', (route) => route.fulfill({ json: [
        { day_of_week: 4, hour: 12, count: 1234 },
      ] }))
      await page.route(`**/api/dashboard/clients/${ip}/detail`, (route) => {
        gets.detail++
        return route.fulfill({ json: {
          total_requests: 1234, first_seen: lastSeen, last_seen: lastSeen,
          recent_requests: [{ id: 'client-vm-request', timestamp: lastSeen, model: 'drilldown-vm-model', status: 'success', duration_ms: 42 }],
          model_distribution: [], hourly_pattern: [{ hour: 12, count: 1234 }],
        } })
      })
      await page.route(`**/api/dashboard/clients/${ip}/api-keys`, (route) => {
        gets.apiKeys++
        return route.fulfill({ json: [{ api_key_id: 'client-vm-key', name: 'Client VM Key', request_count: 1234 }] })
      })
      await page.route('**/api/dashboard/settings/ip_alert_threshold', (route) => {
        if (route.request().method() === 'GET') {
          return route.fulfill({ json: { key: 'ip_alert_threshold', value: threshold } })
        }
        expect(route.request().method()).toBe('PUT')
        expect(route.request().headers()['x-csrf-token']).toBeTruthy()
        expect(route.request().postDataJSON()).toEqual({ value: '2500' })
        expect(saveRoute).toBeUndefined()
        saveRoute = route
      })
      await page.route('**/api/dashboard/stats/tokens/daily*', (route) => {
        gets.daily++
        expect(new URL(route.request().url()).searchParams.get('days')).toBe('7')
        return route.fulfill({ json: [{ date: '2026-10-01', request_count: 12,
          total_input_tokens: 1000, total_output_tokens: 2000, total_tokens: 3000 }] })
      })
      await page.route('**/api/dashboard/stats/tokens/monthly*', (route) => {
        gets.monthly++
        expect(new URL(route.request().url()).searchParams.get('months')).toBe('6')
        return route.fulfill({ json: [{ month: '2026-10', request_count: 24,
          total_input_tokens: 2000, total_output_tokens: 4000, total_tokens: 6000 }] })
      })
      await page.route('**/api/dashboard/logs/lb*', (route) => {
        gets.logs++
        expect(new URL(route.request().url()).searchParams.get('limit')).toBe('200')
        return route.fulfill({ json: { source: 'lb', entries: [
          { timestamp: lastSeen, level: 'INFO', target: 'client_vm', message: `log-poll-${gets.logs}` },
          { timestamp: lastSeen, level: 'ERROR', target: 'client_vm', message: 'controlled-error-entry' },
        ] } })
      })

      await page.clock.install()
      await page.clock.pauseAt(Date.now() + 1000)
      await ensureDashboardLogin(page)
      if (colorScheme === 'dark') await expect(page.locator('html')).toHaveClass(/\bdark\b/)
      else await expect(page.locator('html')).not.toHaveClass(/\bdark\b/)
      const visible = async (locator: Locator) => expect.poll(async () => {
        await page.clock.runFor(1)
        return locator.isVisible()
      }).toBe(true)
      const screenshot = async (name: string) => testInfo.attach(`${name}-${colorScheme}`, {
        body: await page.screenshot({ animations: 'disabled', fullPage: true }), contentType: 'image/png',
      })

      await page.getByRole('tab', { name: /traffic/i }).click()
      const rankingRow = page.getByRole('row').filter({ hasText: ip })
      await visible(rankingRow)
      expect(gets.ranking).toBe(1)
      expect(gets.detail).toBe(0)
      expect(gets.apiKeys).toBe(0)
      await expect(rankingRow.getByText('Alert', { exact: true })).toBeVisible()
      await rankingRow.click()
      await visible(page.getByText('drilldown-vm-model', { exact: true }))
      await visible(page.getByText('Client VM Key', { exact: true }))
      expect(gets.detail).toBe(1)
      expect(gets.apiKeys).toBe(1)
      expect(gets.ranking).toBe(1)
      await expect(page.getByText('42ms', { exact: true })).toBeVisible()
      await screenshot('clients-expanded')
      await rankingRow.click()
      await expect(page.getByText('drilldown-vm-model', { exact: true })).toBeHidden()

      const next = page.getByRole('button').filter({ has: page.locator('svg.lucide-chevron-right') })
      await next.click()
      await expect.poll(async () => { await page.clock.runFor(1); return rankingPages }).toEqual(['1', '2'])
      await visible(page.getByText('2 / 2', { exact: true }))
      await page.getByRole('button', { name: 'Edit', exact: true }).click()
      await page.getByRole('spinbutton').fill('2500')
      await page.getByRole('button', { name: 'Save', exact: true }).click()
      await expect.poll(() => Boolean(saveRoute)).toBe(true)
      await page.clock.runFor(1) // Deliver React Query's pending-state notification.
      await expect(page.getByRole('button', { name: 'Save', exact: true })).toBeDisabled()
      threshold = '2500'
      await saveRoute!.fulfill({ json: { key: 'ip_alert_threshold', value: threshold } })
      await visible(page.getByText('2500 requests', { exact: true }))
      await expect.poll(async () => { await page.clock.runFor(1); return gets.ranking }).toBe(3)
      await expect(rankingRow.getByText('Alert', { exact: true })).toBeHidden()
      // Closed details do not refetch when threshold saves, or on the next provider tick.
      expect(gets.detail).toBe(1)
      expect(gets.apiKeys).toBe(1)
      await page.getByRole('button', { name: 'Edit', exact: true }).click()
      await page.getByRole('spinbutton').fill('999')
      await page.getByRole('spinbutton').press('Escape')
      await visible(page.getByText('2500 requests', { exact: true }))
      await page.clock.runFor(5100)
      await expect.poll(() => gets.ranking).toBe(4)
      expect(gets.detail).toBe(1)
      expect(gets.apiKeys).toBe(1)

      await page.getByRole('tab', { name: /usage/i }).click()
      await visible(page.getByRole('img', { name: 'Token Statistics' }))
      const dailyDate = page.locator('div.font-medium').filter({ hasText: /^2026-10-01$/ })
      const monthlyDate = page.locator('div.font-medium').filter({ hasText: /^2026-10$/ })
      await expect(dailyDate).toBeVisible()
      expect(gets.daily).toBe(1)
      expect(gets.monthly).toBe(1)
      await page.getByRole('tab', { name: 'Monthly', exact: true }).click()
      await visible(monthlyDate)
      expect(gets.daily).toBe(1)
      expect(gets.monthly).toBe(1)
      await page.clock.runFor(1000) // Finish the chart's animation before capturing it.
      await screenshot('monthly-tokens')
      await page.getByRole('tab', { name: 'Daily', exact: true }).click()
      await visible(dailyDate)

      await page.getByRole('tab', { name: /system/i }).click()
      const logList = page.locator('#logs-router-list')
      await visible(logList.getByText('log-poll-1', { exact: true }))
      await page.clock.runFor(4900)
      expect(gets.logs).toBe(1)
      await page.clock.runFor(200)
      await visible(logList.getByText('log-poll-2', { exact: true }))
      expect(gets.logs).toBe(2)
      await page.locator('#logs-router-refresh').click()
      await visible(logList.getByText('log-poll-3', { exact: true }))
      expect(gets.logs).toBe(3)
      const logCard = page.locator('.bg-card').filter({ has: logList })
      await logCard.getByRole('combobox').click()
      await page.getByRole('option', { name: 'Error', exact: true }).click()
      await expect(logList.getByText('controlled-error-entry', { exact: true })).toBeVisible()
      await expect(logList.getByText('log-poll-3', { exact: true })).toBeHidden()
      await page.getByRole('switch', { name: 'Auto-scroll' }).click()
      await expect(page.getByRole('switch', { name: 'Auto-scroll' })).not.toBeChecked()
      const downloadPromise = page.waitForEvent('download')
      await logCard.getByRole('button', { name: 'Download', exact: true }).click()
      const download = await downloadPromise
      expect(download.suggestedFilename()).toMatch(/^logs-llmlb-\d{4}-\d{2}-\d{2}\.txt$/)
      const text = await readFile((await download.path())!, 'utf8')
      expect(text).toBe(`[${lastSeen}] [ERROR] [client_vm] controlled-error-entry`)
      await logCard.getByRole('button', { name: 'Clear', exact: true }).click()
      await visible(page.getByText('Log clearing is handled on the server side', { exact: true }))
      await screenshot('filtered-logs')
      expect(errors).toEqual([])
    })
  })
}
