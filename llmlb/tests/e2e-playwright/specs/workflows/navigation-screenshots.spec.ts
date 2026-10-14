import { DashboardPage } from '../../pages/dashboard.page';
import { DASHBOARD_SCREEN, screenshotCases, test } from '../../helpers/screen-registry';
import { captureScreen } from '../../helpers/screenshot-helpers';

/**
 * Navigation Screenshots - Captures screenshots of all major screens.
 *
 * Run with: pnpm exec playwright test --project=screenshots --headed
 * Screenshots are saved to reports/screenshots/
 */
test.describe('Navigation Screenshots @screenshots', () => {
  // Cases come from the canonical screen registry (SPEC #582 US-006).
  for (const { title, screen, fileName } of screenshotCases()) {
    test(title, async ({ page, visit }) => {
      await visit(screen);
      // Let entrance animations settle before capturing.
      await page.waitForTimeout(500);
      await captureScreen(page, fileName);
    });
  }

  test('SS-10: Dark theme screenshot', async ({ page, visit }) => {
    await visit(DASHBOARD_SCREEN);
    // Ensure dark theme
    const isDark = await page.evaluate(() =>
      document.documentElement.classList.contains('dark')
    );
    if (!isDark) {
      await new DashboardPage(page).toggleTheme();
      await page.waitForTimeout(300);
    }
    await captureScreen(page, 'ss-10-dark-theme');
  });

  test('SS-11: Mobile viewport screenshot', async ({ page, visit }) => {
    await visit(DASHBOARD_SCREEN);
    await page.setViewportSize({ width: 375, height: 667 });
    await page.waitForTimeout(500);
    await captureScreen(page, 'ss-11-mobile-viewport');
  });
});
