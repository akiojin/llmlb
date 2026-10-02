/**
 * Canonical screen registry (SPEC #582 US-006 / T006-T008, Issue #796)
 *
 * The single source of screen coverage for the navigation, back-navigation and
 * screenshot suites. Each entry names a screen and knows how to reach it; the
 * suites derive their test cases from this list, so covering a new screen means
 * adding one entry here.
 *
 * The Invitation Codes modal and `/dashboard/register.html` are deliberately
 * absent: SPEC #582 excludes the invite flow from required coverage. They stay
 * covered as legacy flows by `specs/auth/invitation.spec.ts` and `register.spec.ts`.
 */

import {
  test as base,
  expect,
  type APIRequestContext,
  type Locator,
  type Page,
} from '@playwright/test';
import { LoginPage } from '../pages/auth.page';
import { DashboardPage } from '../pages/dashboard.page';
import { createUser, deleteEndpoint, deleteUser } from './api-helpers';
import { startMockOpenAIEndpointServer } from './mock-openai-endpoint';
import { DashboardSelectors } from './selectors';

// ============================================================================
// Types
// ============================================================================

/** What a reach procedure may use. It always starts from a fresh, signed-out browser context. */
export interface ScreenVisit {
  page: Page;
  request: APIRequestContext;
  /** Registers teardown for data the reach procedure provisioned. Runs after the test, newest first. */
  onCleanup(cleanup: () => Promise<void>): void;
}

interface ScreenBase {
  /** Stable identifier. Test titles and screenshot file names derive from it. */
  id: string;
  title: string;
  /** URL the screen is served at, when the screen has one of its own. */
  url?: RegExp;
  /** Navigates to the screen. */
  reach(visit: ScreenVisit): Promise<void>;
  /** The element that proves the screen is displayed. */
  landmark(page: Page): Locator;
}

export interface StaticScreen extends ScreenBase {
  kind: 'page' | 'tab' | 'modal';
}

/** Hash routes push a history entry, so they are the screens that can be left by going back. */
export interface HashRouteScreen extends ScreenBase {
  kind: 'hash-route';
  /** The in-app control that returns to the dashboard. */
  backButton(page: Page): Locator;
}

export type CanonicalScreen = StaticScreen | HashRouteScreen;

// ============================================================================
// Reach procedures
// ============================================================================

const API_AUTH = { Authorization: 'Bearer sk_debug' };
const PROVISIONED_PREFIX = 'e2e-screen-';

function provisionedName(): string {
  return `${PROVISIONED_PREFIX}${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

async function openDashboard({ page }: ScreenVisit): Promise<DashboardPage> {
  const dashboard = new DashboardPage(page);
  await dashboard.goto();
  return dashboard;
}

interface ProvisionedEndpoint {
  name: string;
  model: string;
}

/** Registers an online endpoint backed by a mock server that serves one model no other test uses. */
async function provisionEndpoint({ request, onCleanup }: ScreenVisit): Promise<ProvisionedEndpoint> {
  const name = provisionedName();
  const model = `${name}-model`;
  const mock = await startMockOpenAIEndpointServer({ models: [model] });
  onCleanup(() => mock.close());

  const created = await request.post('/api/endpoints', {
    headers: API_AUTH,
    data: { name, base_url: mock.baseUrl },
  });
  expect(created.ok(), 'endpoint creation should succeed').toBeTruthy();
  const { id } = (await created.json()) as { id: string };
  onCleanup(async () => {
    await deleteEndpoint(request, id);
  });

  // The connection test brings the endpoint online; the sync registers its model.
  for (const action of ['test', 'sync']) {
    const response = await request.post(`/api/endpoints/${id}/${action}`, { headers: API_AUTH });
    expect(response.ok(), `endpoint ${action} should succeed`).toBeTruthy();
  }
  return { name, model };
}

async function openEndpointDetail(visit: ScreenVisit): Promise<void> {
  const endpoint = await provisionEndpoint(visit);
  await openDashboard(visit);
  // Search narrows the table to this endpoint regardless of pagination.
  await visit.page.getByPlaceholder('Search by name or URL...').fill(endpoint.name);
  const row = visit.page.getByRole('row').filter({ hasText: endpoint.name });
  await row.locator('button[title="Details"]').click();
}

function endpointDetailModal(page: Page): Locator {
  return page
    .getByRole('dialog')
    .filter({ has: page.getByRole('button', { name: 'Open Playground' }) });
}

function playgroundBackButton(page: Page): Locator {
  return page.getByRole('button', { name: 'Back to Dashboard' });
}

// ============================================================================
// Registry
// ============================================================================

/** The dashboard home. Back navigation from every hash route lands here. */
export const DASHBOARD_SCREEN: StaticScreen = {
  id: 'dashboard',
  kind: 'page',
  title: 'Dashboard',
  // `/dashboard/#` is what the in-app back button leaves behind.
  url: /\/dashboard\/#?$/,
  reach: async (visit) => {
    await openDashboard(visit);
  },
  landmark: (page) => page.locator(DashboardSelectors.header.themeToggle),
};

/** Dashboard tabs: the Radix tab value and the label the tab shows. */
const DASHBOARD_TABS = [
  { value: 'endpoints', label: 'Endpoints' },
  { value: 'models', label: 'Models' },
  { value: 'statistics', label: 'Usage' },
  { value: 'history', label: 'Requests' },
  { value: 'clients', label: 'Traffic' },
  { value: 'logs', label: 'System' },
];

function tabScreen({ value, label }: (typeof DASHBOARD_TABS)[number]): StaticScreen {
  return {
    id: `tab-${value}`,
    kind: 'tab',
    title: `Dashboard ${label} tab`,
    reach: async (visit) => {
      await openDashboard(visit);
      await visit.page.getByRole('tab', { name: label, exact: true }).click();
    },
    landmark: (page) =>
      page.locator(`[role="tabpanel"][data-state="active"][id$="-content-${value}"]`),
  };
}

export const CANONICAL_SCREENS: readonly CanonicalScreen[] = [
  // --- Top-level pages ---
  {
    id: 'login',
    kind: 'page',
    title: 'Login page',
    url: /\/dashboard\/login\.html/,
    reach: async ({ page }) => {
      await new LoginPage(page).goto();
    },
    landmark: (page) => page.locator('#username'),
  },
  DASHBOARD_SCREEN,
  {
    id: 'change-password',
    kind: 'page',
    title: 'Change Password page',
    url: /\/dashboard\/change-password\.html/,
    // A user created by an admin must change the generated password on first sign-in.
    reach: async ({ page, request, onCleanup }) => {
      const username = provisionedName();
      const user = await createUser(request, username, '', 'viewer');
      expect(user.generated_password, 'server should return a generated password').toBeTruthy();
      onCleanup(async () => {
        await deleteUser(request, user.id);
      });

      const login = new LoginPage(page);
      await login.goto();
      await login.login(username, user.generated_password!);
    },
    landmark: (page) => page.locator('#new-password'),
  },

  // --- Hash routes ---
  {
    id: 'lb-playground',
    kind: 'hash-route',
    title: 'LB Playground',
    url: /\/dashboard\/#lb-playground$/,
    reach: async (visit) => {
      const dashboard = await openDashboard(visit);
      await dashboard.openPlayground();
    },
    landmark: (page) => page.locator('#lb-playground-sidebar'),
    backButton: playgroundBackButton,
  },
  {
    id: 'audit-log',
    kind: 'hash-route',
    title: 'Audit Log',
    url: /\/dashboard\/#audit-log$/,
    reach: async (visit) => {
      const dashboard = await openDashboard(visit);
      await dashboard.openAuditLog();
    },
    landmark: (page) => page.getByRole('heading', { name: 'Audit Log', exact: true }),
    backButton: (page) => page.getByRole('button', { name: 'Dashboard', exact: true }),
  },
  {
    id: 'endpoint-playground',
    kind: 'hash-route',
    title: 'Endpoint Playground',
    url: /\/dashboard\/#playground\/[^/]+$/,
    reach: async (visit) => {
      await openEndpointDetail(visit);
      await endpointDetailModal(visit.page).getByRole('button', { name: 'Open Playground' }).click();
    },
    landmark: (page) => page.getByText('Start a conversation'),
    backButton: playgroundBackButton,
  },

  // --- Dashboard tabs ---
  ...DASHBOARD_TABS.map(tabScreen),

  // --- Modals ---
  {
    id: 'modal-api-keys',
    kind: 'modal',
    title: 'API Keys modal',
    reach: async (visit) => {
      const dashboard = await openDashboard(visit);
      await dashboard.openApiKeys();
    },
    landmark: (page) => page.locator(DashboardSelectors.modals.apiKeysModal),
  },
  {
    id: 'modal-manage-users',
    kind: 'modal',
    title: 'Manage Users modal',
    reach: async (visit) => {
      const dashboard = await openDashboard(visit);
      await dashboard.openManageUsersModal();
    },
    landmark: (page) => page.locator(DashboardSelectors.modals.userModal),
  },
  {
    id: 'modal-endpoint-detail',
    kind: 'modal',
    title: 'Endpoint Detail modal',
    reach: openEndpointDetail,
    landmark: endpointDetailModal,
  },
  {
    id: 'modal-request-detail',
    kind: 'modal',
    title: 'Request Detail modal',
    reach: async (visit) => {
      const endpoint = await provisionEndpoint(visit);
      const completion = await visit.request.post('/v1/chat/completions', {
        headers: API_AUTH,
        data: { model: endpoint.model, messages: [{ role: 'user', content: 'screen registry' }] },
      });
      expect(completion.ok(), 'chat completion should succeed').toBeTruthy();

      const dashboard = await openDashboard(visit);
      await dashboard.goToHistoryTab();
      await dashboard.getHistoryRows().filter({ hasText: endpoint.model }).first().click();
    },
    landmark: (page) => page.locator(DashboardSelectors.modals.requestModal),
  },
  {
    id: 'modal-model-add-wizard',
    kind: 'modal',
    title: 'Model Add Wizard modal',
    reach: async (visit) => {
      await openDashboard(visit);
      await visit.page.getByRole('tab', { name: 'Models', exact: true }).click();
      await visit.page.getByRole('button', { name: 'Add Model' }).click();
    },
    landmark: (page) =>
      page
        .getByRole('dialog')
        .filter({ has: page.getByRole('heading', { name: 'Search HuggingFace Models' }) }),
  },
];

// ============================================================================
// Suite cases
// ============================================================================

export interface ScreenCase<S extends CanonicalScreen = CanonicalScreen> {
  title: string;
  screen: S;
}

export interface BackNavigationCase extends ScreenCase<HashRouteScreen> {
  /** Leaves the screen the way this case covers. */
  leave(page: Page): Promise<void>;
}

export interface ScreenshotCase extends ScreenCase {
  /** Screenshot name, without extension. */
  fileName: string;
}

/** `specs/dashboard/screen-navigation.spec.ts`: every screen is reachable. */
export function navigationCases(
  screens: readonly CanonicalScreen[] = CANONICAL_SCREENS
): ScreenCase[] {
  return screens.map((screen) => ({
    title: `NAV [${screen.id}]: reaches ${screen.title}`,
    screen,
  }));
}

/** `specs/dashboard/back-navigation.spec.ts`: every hash route returns to the dashboard both ways. */
export function backNavigationCases(
  screens: readonly CanonicalScreen[] = CANONICAL_SCREENS
): BackNavigationCase[] {
  return screens
    .filter((screen): screen is HashRouteScreen => screen.kind === 'hash-route')
    .flatMap((screen) => [
      {
        title: `BB [${screen.id}]: browser Back returns to Dashboard`,
        screen,
        leave: async (page: Page) => {
          await page.goBack();
        },
      },
      {
        title: `BB [${screen.id}]: in-app back button returns to Dashboard`,
        screen,
        leave: (page: Page) => screen.backButton(page).click(),
      },
    ]);
}

/** `specs/workflows/navigation-screenshots.spec.ts`: every screen is captured. */
export function screenshotCases(
  screens: readonly CanonicalScreen[] = CANONICAL_SCREENS
): ScreenshotCase[] {
  return screens.map((screen) => ({
    title: `SS [${screen.id}]: ${screen.title} screenshot`,
    screen,
    fileName: `ss-${screen.id}`,
  }));
}

// ============================================================================
// Fixture
// ============================================================================

// Reaching a screen usually ends in a sign-in, which is slow on a debug build
// when several tests sign in at once.
const SCREEN_TIMEOUT_MS = 15000;

/** Asserts that `screen` is the one currently displayed. */
export async function expectScreen(page: Page, screen: CanonicalScreen): Promise<void> {
  if (screen.url) {
    await expect(page).toHaveURL(screen.url, { timeout: SCREEN_TIMEOUT_MS });
  }
  await expect(screen.landmark(page)).toBeVisible({ timeout: SCREEN_TIMEOUT_MS });
}

interface ScreenFixtures {
  /** Reaches a screen, asserts it is displayed, and tears down what reaching it provisioned. */
  visit(screen: CanonicalScreen): Promise<void>;
}

export const test = base.extend<ScreenFixtures>({
  visit: async ({ page, request }, use) => {
    const cleanups: Array<() => Promise<void>> = [];
    await use(async (screen) => {
      await screen.reach({ page, request, onCleanup: (cleanup) => cleanups.push(cleanup) });
      await expectScreen(page, screen);
    });
    for (const cleanup of cleanups.reverse()) {
      await cleanup();
    }
  },
});

export { expect };
