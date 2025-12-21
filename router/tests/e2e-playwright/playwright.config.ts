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
        command:
          'LLM_CONVERT_SCRIPT=router/tests/e2e-playwright/mock_convert.py LLM_ROUTER_SKIP_API_KEY=1 cargo run --release -p llm-router',
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
