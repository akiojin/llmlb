import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright E2E Test Configuration for LLM Load Balancer
 * @see https://playwright.dev/docs/test-configuration
 */
export default defineConfig({
  testDir: './specs',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: [
    ['html', { outputFolder: 'reports/html' }],
    ['json', { outputFile: 'reports/results.json' }],
    ['list'],
  ],
  use: {
    baseURL: process.env.BASE_URL || 'http://127.0.0.1:32768',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  // Server auto-start (optional - set SKIP_SERVER=1 to disable)
  webServer: process.env.SKIP_SERVER
    ? undefined
    : {
        command:
          process.platform === 'win32'
            ? 'set LLMLB_DATABASE_URL=sqlite:router\\tests\\e2e-playwright\\.llmlb\\router.db&& set LLMLB_LOG_DIR=router\\tests\\e2e-playwright\\.llmlb\\logs&& cargo run -p llmlb'
            : 'LLMLB_DATABASE_URL=sqlite:router/tests/e2e-playwright/.llmlb/router.db LLMLB_LOG_DIR=router/tests/e2e-playwright/.llmlb/logs cargo run -p llmlb',
        url: 'http://127.0.0.1:32768/dashboard',
        reuseExistingServer: !process.env.CI,
        timeout: 300000,
        cwd: '../../../',
      },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
    // Uncomment for multi-browser testing
    // {
    //   name: 'firefox',
    //   use: { ...devices['Desktop Firefox'] },
    // },
    // {
    //   name: 'webkit',
    //   use: { ...devices['Desktop Safari'] },
    // },
  ],
});
