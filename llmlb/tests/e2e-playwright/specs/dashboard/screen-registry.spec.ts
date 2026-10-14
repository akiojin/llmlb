import { test, expect, type Page } from '@playwright/test';
import { execFileSync } from 'child_process';
import * as path from 'path';
import {
  CANONICAL_SCREENS,
  backNavigationCases,
  navigationCases,
  screenshotCases,
  type CanonicalScreen,
  type HashRouteScreen,
} from '../../helpers/screen-registry';

/**
 * Canonical screen registry contract (SPEC #582 US-006 / T006-T008, Issue #796)
 *
 * The registry is the single source of screen coverage. These tests pin what it
 * must contain and prove that the navigation, back-navigation and screenshot
 * suites follow it, so adding a screen means adding one registry entry.
 */

/** SPEC #582 US-006 "Canonical required coverage". */
const REQUIRED_COVERAGE: Record<CanonicalScreen['kind'], string[]> = {
  page: ['login', 'dashboard', 'change-password'],
  'hash-route': ['lb-playground', 'audit-log', 'endpoint-playground'],
  tab: ['tab-endpoints', 'tab-models', 'tab-statistics', 'tab-history', 'tab-clients', 'tab-logs'],
  modal: [
    'modal-api-keys',
    'modal-manage-users',
    'modal-endpoint-detail',
    'modal-request-detail',
    'modal-model-add-wizard',
  ],
};

/** SPEC #582 US-006 "Explicit exclusions": the invite flow is legacy coverage. */
const LEGACY_SCREEN = /invitation|register/i;

const E2E_ROOT = path.resolve(__dirname, '../..');

interface ListedSuite {
  specs?: Array<{ title: string }>;
  suites?: ListedSuite[];
}

function collectTitles(suite: ListedSuite): string[] {
  return [
    ...(suite.specs ?? []).map((spec) => spec.title),
    ...(suite.suites ?? []).flatMap(collectTitles),
  ];
}

/**
 * Titles Playwright actually collects from a spec file. Asking the runner is the
 * only way to observe the suite itself rather than the functions it should call.
 */
function listTestTitles(project: string, specFile: string, extraEnv: NodeJS.ProcessEnv = {}): string[] {
  // Runner state of this worker must not leak into the nested listing process.
  const env = Object.fromEntries(
    Object.entries(process.env).filter(([key]) => !/^(PLAYWRIGHT_|PW_|PWTEST_|TEST_)/.test(key))
  );
  const stdout = execFileSync(
    process.execPath,
    [
      require.resolve('@playwright/test/cli'),
      'test',
      '--list',
      '--reporter=json',
      `--project=${project}`,
      specFile,
    ],
    { cwd: E2E_ROOT, env: { ...env, ...extraEnv }, encoding: 'utf8' }
  );
  return collectTitles(JSON.parse(stdout) as ListedSuite);
}

function probeScreen(kind: 'hash-route'): HashRouteScreen;
function probeScreen(kind: Exclude<CanonicalScreen['kind'], 'hash-route'>): CanonicalScreen;
function probeScreen(kind: CanonicalScreen['kind']): CanonicalScreen {
  const base = {
    id: `probe-${kind}`,
    title: `Probe ${kind}`,
    reach: async () => {},
    landmark: (page: Page) => page.locator('body'),
  };
  return kind === 'hash-route'
    ? { ...base, kind, backButton: (page) => page.locator('button') }
    : { ...base, kind };
}

test.describe('Canonical Screen Registry @dashboard @navigation', () => {
  test('SR-01: registry covers the SPEC #582 US-006 canonical screens', () => {
    for (const [kind, requiredIds] of Object.entries(REQUIRED_COVERAGE)) {
      const registered = CANONICAL_SCREENS.filter((screen) => screen.kind === kind).map(
        (screen) => screen.id
      );
      expect(registered, `${kind} screens`).toEqual(expect.arrayContaining(requiredIds));
    }
  });

  test('SR-02: every entry has a unique file-safe id and a title', () => {
    const ids = CANONICAL_SCREENS.map((screen) => screen.id);
    expect(new Set(ids).size, 'ids are unique').toBe(ids.length);
    for (const screen of CANONICAL_SCREENS) {
      // The id becomes part of test titles and screenshot file names.
      expect(screen.id).toMatch(/^[a-z0-9]+(-[a-z0-9]+)*$/);
      expect(screen.title.trim()).not.toBe('');
    }
  });

  test('SR-03: legacy invite-flow screens are not canonical', () => {
    for (const screen of CANONICAL_SCREENS) {
      expect(`${screen.id} ${screen.title}`).not.toMatch(LEGACY_SCREEN);
    }
  });

  test('SR-04: adding a registry entry adds a case to every suite', () => {
    const probe = probeScreen('hash-route');
    const extended = [...CANONICAL_SCREENS, probe];

    for (const derive of [navigationCases, backNavigationCases, screenshotCases]) {
      const before = derive(CANONICAL_SCREENS).filter((c) => c.screen.id === probe.id);
      const after = derive(extended).filter((c) => c.screen.id === probe.id);
      expect(before, `${derive.name} without the entry`).toHaveLength(0);
      expect(after.length, `${derive.name} with the entry`).toBeGreaterThan(0);
    }
  });

  test('SR-05: navigation and screenshot cover every entry; back-navigation covers hash routes', () => {
    const extended = [
      ...CANONICAL_SCREENS,
      probeScreen('page'),
      probeScreen('tab'),
      probeScreen('modal'),
      probeScreen('hash-route'),
    ];
    const allIds = extended.map((screen) => screen.id);
    const hashRouteIds = extended
      .filter((screen) => screen.kind === 'hash-route')
      .map((screen) => screen.id);

    expect(navigationCases(extended).map((c) => c.screen.id)).toEqual(allIds);
    expect(screenshotCases(extended).map((c) => c.screen.id)).toEqual(allIds);
    // Only hash routes push a history entry, so only they have a way back.
    // Each is left twice: by the browser Back button and by the in-app one.
    expect(backNavigationCases(extended).map((c) => c.screen.id)).toEqual(
      hashRouteIds.flatMap((id) => [id, id])
    );

    const fileNames = screenshotCases(extended).map((c) => c.fileName);
    expect(new Set(fileNames).size, 'screenshot file names are unique').toBe(fileNames.length);
  });

  test('SR-06: the three suites enumerate the registry-derived cases', () => {
    const suites = [
      {
        cases: navigationCases(),
        listed: listTestTitles('chromium', 'specs/dashboard/screen-navigation.spec.ts'),
      },
      {
        cases: backNavigationCases(),
        listed: listTestTitles('chromium', 'specs/dashboard/back-navigation.spec.ts'),
      },
      {
        cases: screenshotCases(),
        listed: listTestTitles('screenshots', 'specs/workflows/navigation-screenshots.spec.ts', {
          PLAYWRIGHT_SCREENSHOTS: '1',
        }),
      },
    ];

    for (const { cases, listed } of suites) {
      expect(cases.length).toBeGreaterThan(0);
      for (const { title } of cases) {
        expect(listed.filter((listedTitle) => listedTitle === title), title).toHaveLength(1);
      }
      for (const listedTitle of listed) {
        expect(listedTitle).not.toMatch(LEGACY_SCREEN);
      }
    }
  });
});
