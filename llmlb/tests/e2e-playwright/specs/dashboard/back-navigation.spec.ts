import {
  DASHBOARD_SCREEN,
  backNavigationCases,
  expectScreen,
  test,
} from '../../helpers/screen-registry';

test.describe('Back Navigation @dashboard @navigation', () => {
  // Cases come from the canonical screen registry (SPEC #582 US-006):
  // every hash route is left by the browser Back button and by its in-app one.
  for (const { title, screen, leave } of backNavigationCases()) {
    test(title, async ({ page, visit }) => {
      await visit(screen);

      await leave(page);

      await expectScreen(page, DASHBOARD_SCREEN);
    });
  }
});
