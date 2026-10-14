import { test, expect, type Page, type Route } from '@playwright/test';
import { DashboardPage } from '../../pages/dashboard.page';

/**
 * Model Add Wizard modal (SPEC #582 US-006 / FR-045)
 *
 * The HuggingFace catalog and endpoint download APIs are stubbed so the wizard
 * flow is deterministic and never triggers a real download.
 */

const REPO_ID = 'e2e-org/tiny-llm-GGUF';
const OTHER_REPO_ID = 'e2e-org/other-llm';

const ENDPOINTS = [
  { id: 'e2e-ep-ok', name: 'E2E Endpoint OK', endpoint_type: 'xllm', can_download: true, has_model: false },
  { id: 'e2e-ep-fail', name: 'E2E Endpoint Fail', endpoint_type: 'ollama', can_download: true, has_model: true },
  { id: 'e2e-ep-nodl', name: 'E2E Endpoint NoDownload', endpoint_type: 'vllm', can_download: false, has_model: false },
];

interface CatalogStub {
  searchQueries: string[];
  downloadRequests: Array<{ endpointId: string; body: Record<string, unknown> }>;
}

async function stubCatalog(page: Page): Promise<CatalogStub> {
  const stub: CatalogStub = { searchQueries: [], downloadRequests: [] };

  await page.route('**/api/catalog/**', async (route: Route) => {
    const url = new URL(route.request().url());
    const path = url.pathname;

    if (path === '/api/catalog/search') {
      const q = url.searchParams.get('q') ?? '';
      stub.searchQueries.push(q);
      const models = q.includes('nothing')
        ? []
        : [
            {
              repo_id: REPO_ID,
              description: 'Tiny LLM for E2E',
              downloads: 12345,
              tags: ['gguf', 'text-generation', 'llama', 'e2e'],
              engine_names: { xllm: 'tiny-llm', ollama: null },
              supports_download: ['xllm', 'ollama'],
            },
            {
              repo_id: OTHER_REPO_ID,
              description: 'Another model',
              downloads: 10,
              tags: [],
              engine_names: {},
              supports_download: [],
            },
          ];
      return route.fulfill({ json: { models } });
    }

    if (path.startsWith('/api/catalog/recommend-endpoints/')) {
      return route.fulfill({ json: { endpoints: ENDPOINTS } });
    }

    if (path === `/api/catalog/${REPO_ID}`) {
      return route.fulfill({
        json: {
          repo_id: REPO_ID,
          description: 'Tiny LLM for E2E',
          downloads: 12345,
          tags: ['gguf', 'text-generation'],
          siblings: [{ rfilename: 'tiny-llm.Q4_K_M.gguf' }, { rfilename: 'README.md' }],
          pipeline_tag: 'text-generation',
          engine_names: { xllm: 'tiny-llm', ollama: null },
          supports_download: ['xllm', 'ollama'],
        },
      });
    }

    return route.fallback();
  });

  await page.route('**/api/endpoints/*/download', async (route: Route) => {
    const endpointId = new URL(route.request().url()).pathname.split('/')[3];
    stub.downloadRequests.push({ endpointId, body: route.request().postDataJSON() });
    if (endpointId === 'e2e-ep-fail') {
      return route.fulfill({ status: 500, json: { error: 'download failed' } });
    }
    return route.fulfill({ json: { task_id: `task-${endpointId}` } });
  });

  return stub;
}

async function openWizard(page: Page) {
  await page.click('button[role="tab"]:has-text("Models")');
  const addButton = page.getByRole('button', { name: 'Add Model' });
  await expect(addButton).toBeVisible({ timeout: 10000 });
  await addButton.click();
  const wizard = page.getByRole('dialog');
  await expect(wizard).toBeVisible({ timeout: 5000 });
  await expect(wizard.getByRole('heading', { name: 'Search HuggingFace Models' })).toBeVisible();
  return wizard;
}

async function searchAndSelect(page: Page, wizard: ReturnType<Page['getByRole']>) {
  await wizard.getByPlaceholder('Search models (e.g., llama, mistral, phi)...').fill('tiny');
  const row = wizard.getByRole('row').filter({ hasText: REPO_ID });
  await expect(row).toBeVisible({ timeout: 5000 });
  await row.click();
  await expect(wizard.getByRole('heading', { name: 'Model Details' })).toBeVisible();
  await expect(wizard.getByText('tiny-llm.Q4_K_M.gguf')).toBeVisible({ timeout: 5000 });
}

test.describe('Model Add Wizard @dashboard @navigation', () => {
  let stub: CatalogStub;
  let pageErrors: string[];

  test.beforeEach(async ({ page }) => {
    pageErrors = [];
    page.on('pageerror', (err) => pageErrors.push(err.message));
    stub = await stubCatalog(page);
    await new DashboardPage(page).goto();
  });

  test.afterEach(() => {
    expect(pageErrors, 'no uncaught page errors').toEqual([]);
  });

  test('WIZ-01: opens from Models tab and closes via Cancel', async ({ page }) => {
    const wizard = await openWizard(page);

    await expect(wizard.getByText('Search for models on HuggingFace to add to your endpoints')).toBeVisible();
    await expect(wizard.getByText('Enter at least 2 characters to search')).toBeVisible();

    await wizard.getByRole('button', { name: 'Cancel' }).click();
    await expect(wizard).not.toBeVisible({ timeout: 5000 });
  });

  test('WIZ-02: closes via Escape and reopens reset to the search step', async ({ page }) => {
    let wizard = await openWizard(page);
    await searchAndSelect(page, wizard);

    await page.keyboard.press('Escape');
    await expect(page.getByRole('dialog')).not.toBeVisible({ timeout: 5000 });

    wizard = await openWizard(page);
    await expect(wizard.getByPlaceholder('Search models (e.g., llama, mistral, phi)...')).toHaveValue('');
    await expect(wizard.getByText('Enter at least 2 characters to search')).toBeVisible();
  });

  test('WIZ-03: search shows results and empty state', async ({ page }) => {
    const wizard = await openWizard(page);
    const input = wizard.getByPlaceholder('Search models (e.g., llama, mistral, phi)...');

    // Single character does not query the catalog
    await input.fill('t');
    await page.waitForTimeout(500);
    expect(stub.searchQueries).toEqual([]);

    await input.fill('tiny');
    await expect(wizard.getByRole('row').filter({ hasText: REPO_ID })).toBeVisible({ timeout: 5000 });
    await expect(wizard.getByRole('row').filter({ hasText: OTHER_REPO_ID })).toBeVisible();
    await expect(wizard.getByText('12,345')).toBeVisible();
    await expect(wizard.getByText('+1')).toBeVisible();

    await input.fill('nothing');
    await expect(wizard.getByText('No models found for "nothing"')).toBeVisible({ timeout: 5000 });
  });

  test('WIZ-04: detail step shows model info and Back returns to search', async ({ page }) => {
    const wizard = await openWizard(page);
    await searchAndSelect(page, wizard);

    await expect(wizard.getByText(`Details for ${REPO_ID}`)).toBeVisible();
    await expect(wizard.getByText('Files (2)')).toBeVisible();
    await expect(wizard.getByText('xllm: tiny-llm')).toBeVisible();
    await expect(wizard.getByText('Supports Download Via')).toBeVisible();

    await wizard.getByRole('button', { name: 'Back' }).click();
    await expect(wizard.getByRole('heading', { name: 'Search HuggingFace Models' })).toBeVisible();
  });

  test('WIZ-05: selects endpoints, sends downloads, and finishes with Done', async ({ page }) => {
    const wizard = await openWizard(page);
    await searchAndSelect(page, wizard);

    await wizard.getByRole('button', { name: 'Select Endpoints' }).click();
    await expect(wizard.getByRole('heading', { name: 'Select Endpoints' })).toBeVisible();
    await expect(wizard.getByText('E2E Endpoint OK')).toBeVisible({ timeout: 5000 });
    await expect(wizard.getByText('E2E Endpoint Fail')).toBeVisible();
    await expect(wizard.getByText('Already has model')).toBeVisible();
    // Endpoints that cannot download are not offered
    await expect(wizard.getByText('E2E Endpoint NoDownload')).toHaveCount(0);

    const downloadButton = wizard.getByRole('button', { name: /Download to \d+ Endpoints?/ });
    await expect(downloadButton).toBeDisabled();

    await wizard.getByText('E2E Endpoint OK').click();
    await expect(downloadButton).toHaveText(/Download to 1 Endpoint$/);
    await wizard.getByText('E2E Endpoint Fail').click();
    await expect(downloadButton).toHaveText(/Download to 2 Endpoints/);
    await expect(downloadButton).toBeEnabled();

    await downloadButton.click();
    await expect(wizard.getByRole('heading', { name: 'Download Progress' })).toBeVisible();
    await expect(
      wizard.getByText('All download requests have been sent. Check endpoint details for progress.')
    ).toBeVisible({ timeout: 10000 });
    await expect(page.getByText('Download requests sent', { exact: true })).toBeVisible();

    expect(stub.downloadRequests.map((r) => r.endpointId).sort()).toEqual(['e2e-ep-fail', 'e2e-ep-ok']);
    for (const req of stub.downloadRequests) {
      expect(req.body).toMatchObject({ model: REPO_ID, hf_repo: REPO_ID });
    }

    await wizard.getByRole('button', { name: 'Done' }).click();
    await expect(page.getByRole('dialog')).not.toBeVisible({ timeout: 5000 });
  });
});
