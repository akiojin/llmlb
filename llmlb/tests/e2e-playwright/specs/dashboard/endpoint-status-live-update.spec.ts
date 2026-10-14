import { test, expect } from '@playwright/test';
import { ensureDashboardLogin, deleteEndpointsByName, listEndpoints } from '../../helpers/api-helpers';
import { startMockOpenAIEndpointServer, type MockOpenAIEndpointServer } from '../../helpers/mock-openai-endpoint';

const API_BASE = process.env.BASE_URL || 'http://127.0.0.1:32768';
const AUTH_HEADER = { Authorization: 'Bearer sk_debug' };

// The dashboard polls every 10s while the WebSocket is connected, so a status
// change that shows up well within that window was driven by the WebSocket event.
const LIVE_UPDATE_TIMEOUT_MS = 3000;

interface EndpointChangeFrame {
  changed: 'endpoints';
  id: string;
}

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`Endpoint Status Live Update (${colorScheme}) @dashboard`, () => {
    test.use({ colorScheme });

    test('endpoint status changes are reflected without manual refresh (#692)', async ({ page, request }) => {
      test.setTimeout(120_000);

      const endpointName = `e2e-status-live-${Date.now()}-${Math.random().toString(16).slice(2)}`;
      const statusFrames: EndpointChangeFrame[] = [];
      const pageErrors: string[] = [];
      const mock: MockOpenAIEndpointServer = await startMockOpenAIEndpointServer();

      // The pre-login session probe returns 401 by design; only collect errors after login.
      let loggedIn = false;
      page.on('pageerror', (err) => {
        if (loggedIn) pageErrors.push(err.message);
      });
      page.on('console', (msg) => {
        if (loggedIn && msg.type() === 'error') pageErrors.push(msg.text());
      });
      page.on('websocket', (ws) => {
        if (!ws.url().includes('/ws/dashboard')) return;
        ws.on('framereceived', ({ payload }) => {
          try {
            const event = JSON.parse(payload.toString());
            if (event.changed === 'endpoints') statusFrames.push(event);
          } catch {
            // ignore non-JSON frames
          }
        });
      });

      try {
        await page.clock.install();
        await ensureDashboardLogin(page);
        loggedIn = true;

        await page.getByRole('button', { name: 'Add Endpoint' }).click();
        await page.fill('#endpoint-name', endpointName);
        await page.fill('#endpoint-url', mock.baseUrl);
        await page.getByRole('button', { name: 'Create Endpoint' }).click();

        await page.getByPlaceholder('Search by name or URL...').fill(endpointName);
        const row = page.getByRole('row').filter({ hasText: endpointName });
        await expect(row).toBeVisible({ timeout: 20000 });
        const statusBadge = row.locator('td').nth(3).locator('div').first();

        const endpoint = (await listEndpoints(request)).find((e) => e.name === endpointName);
        expect(endpoint?.id).toBeTruthy();
        const endpointId = endpoint!.id;
        const framesSince = (checkpoint: number) =>
          statusFrames.slice(checkpoint).filter((f) => f.id === endpointId);

        // Registration may already trigger an automatic Online probe. Establish
        // that baseline, then force two new transitions with polling frozen.
        await expect(statusBadge).toHaveText('Online', { timeout: 20000 });
        await page.clock.pauseAt(Date.now() + 1000);
        const pausedAt = await page.evaluate(() => Date.now());

        const errorStart = statusFrames.length;
        mock.setHealthy(false);
        const failed = await request.post(`${API_BASE}/api/endpoints/${endpointId}/test`, { headers: AUTH_HEADER });
        expect(failed.ok()).toBe(true);
        expect((await failed.json()).success).toBe(false);
        await expect.poll(() => framesSince(errorStart).length, { timeout: 20000 }).toBeGreaterThan(0);
        await expect(statusBadge).toHaveText('Error', { timeout: LIVE_UPDATE_TIMEOUT_MS });

        const onlineStart = statusFrames.length;
        mock.setHealthy(true);
        const recovered = await request.post(`${API_BASE}/api/endpoints/${endpointId}/test`, { headers: AUTH_HEADER });
        expect(recovered.ok()).toBe(true);
        expect((await recovered.json()).success).toBe(true);
        await expect.poll(() => framesSince(onlineStart).length, { timeout: 20000 }).toBeGreaterThan(0);
        await expect(statusBadge).toHaveText('Online', { timeout: LIVE_UPDATE_TIMEOUT_MS });

        expect(statusFrames.slice(errorStart, onlineStart).filter((f) => f.id === endpointId)).toHaveLength(1);
        expect(framesSince(onlineStart)).toHaveLength(1);
        expect(await page.evaluate(() => Date.now())).toBe(pausedAt);
        expect(pageErrors).toEqual([]);
      } finally {
        await mock.close();
        await deleteEndpointsByName(request, endpointName);
      }
    });
  });
}
