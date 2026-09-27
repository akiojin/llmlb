import { test, expect, type Page } from '@playwright/test';
import { DashboardPage } from '../../pages/dashboard.page';

/**
 * Issue #722: モデル一覧で canonical モデル名と、エンドポイントが報告した
 * 生のモデル名を区別して表示する。
 */

const CANONICAL_ID = 'Qwen/Qwen3.6-35B-A3B';
const RAW_IDS = ['qwen/qwen3.6-35b-a3b', 'qwen3.6-35b-a3b:Q4_K_M'];
const UNKNOWN_ID = 'vendor/not-canonical';

function modelEntry(overrides: Record<string, unknown>) {
  return {
    object: 'model',
    created: 0,
    owned_by: 'load balancer',
    lifecycle_status: 'registered',
    ready: true,
    supported_apis: ['chat_completions', 'responses'],
    max_tokens: null,
    aliases: [],
    ...overrides,
  };
}

const canonicalView = [
  modelEntry({
    id: CANONICAL_ID,
    endpoint_ids: ['endpoint-a', 'endpoint-b'],
    canonical_name: CANONICAL_ID,
    is_canonical: true,
    aliases: RAW_IDS,
  }),
  modelEntry({
    id: UNKNOWN_ID,
    endpoint_ids: ['endpoint-c'],
    canonical_name: UNKNOWN_ID,
    is_canonical: false,
  }),
];

const detailView = [
  modelEntry({
    id: RAW_IDS[0],
    endpoint_ids: ['endpoint-a'],
    canonical_name: CANONICAL_ID,
    is_canonical: false,
  }),
  modelEntry({
    id: RAW_IDS[1],
    endpoint_ids: ['endpoint-b'],
    canonical_name: CANONICAL_ID,
    is_canonical: false,
  }),
];

async function mockDashboardModels(page: Page) {
  await page.route('**/api/dashboard/models*', async (route) => {
    const view = new URL(route.request().url()).searchParams.get('view');
    await route.fulfill({
      contentType: 'application/json',
      body: JSON.stringify({
        object: 'list',
        data: view === 'detail' ? detailView : canonicalView,
      }),
    });
  });
}

function modelRow(page: Page, id: string) {
  return page
    .getByRole('row')
    .filter({ has: page.locator(`[data-model-id="${id}"]`) });
}

for (const colorScheme of ['dark', 'light'] as const) {
  test.describe(`Model canonical tags (${colorScheme}) @dashboard`, () => {
    test.use({ colorScheme });

    let pageErrors: string[];

    test.beforeEach(async ({ page }) => {
      await mockDashboardModels(page);
      const dashboard = new DashboardPage(page);
      // ログイン前の未認証リクエスト（401）は対象外。ログイン後の再読み込みから監視する。
      await dashboard.goto();
      pageErrors = [];
      page.on('pageerror', (error) => pageErrors.push(`pageerror: ${error.message}`));
      page.on('console', (message) => {
        if (message.type() === 'error') pageErrors.push(`console: ${message.text()}`);
      });
      await dashboard.gotoModels();
    });

    test.afterEach(() => {
      expect(pageErrors).toEqual([]);
    });

    test('MCT-01: canonical row is tagged and keeps raw endpoint names visible', async ({
      page,
    }) => {
      const row = modelRow(page, CANONICAL_ID);
      await expect(row).toHaveCount(1);
      await expect(row.locator('[data-model-canonical-badge]')).toHaveCount(1);
      for (const raw of RAW_IDS) {
        await expect(row.locator(`[data-model-alias="${raw}"]`)).toBeVisible();
      }
    });

    test('MCT-02: unknown self-fallback model is not tagged canonical', async ({ page }) => {
      const row = modelRow(page, UNKNOWN_ID);
      await expect(row).toHaveCount(1);
      await expect(row.locator('[data-model-canonical-badge]')).toHaveCount(0);
    });

    test('MCT-03: detail view links each raw name to the shared canonical name', async ({
      page,
    }) => {
      await page.getByRole('button', { name: 'Detail', exact: true }).click();

      for (const raw of RAW_IDS) {
        const row = modelRow(page, raw);
        await expect(row).toHaveCount(1);
        const canonical = row.locator(`[data-model-canonical-name="${CANONICAL_ID}"]`);
        await expect(canonical).toBeVisible();
        await expect(canonical.locator('[data-model-canonical-badge]')).toHaveCount(1);
      }
    });
  });
}
