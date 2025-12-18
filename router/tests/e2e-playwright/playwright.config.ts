import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright E2E Test Configuration for LLM Router
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
    baseURL: process.env.BASE_URL || 'http://localhost:8080',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  // Server auto-start (optional - set SKIP_SERVER=1 to disable)
  webServer: process.env.SKIP_SERVER
    ? undefined
    : {
        // Use debug build by default so development auth key `sk_debug` works.
        // Set ROUTER_RELEASE=1 to run the release binary instead.
        command: process.env.ROUTER_RELEASE
          ? 'cargo run --release -p llm-router'
          : 'cargo run -p llm-router',
        url: 'http://localhost:8080/dashboard',
        reuseExistingServer: !process.env.CI,
        timeout: 120000,
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
