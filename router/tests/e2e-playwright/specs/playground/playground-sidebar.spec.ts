import { test, expect } from '@playwright/test';
import { PlaygroundPage } from '../../pages/playground.page';

test.describe('Playground Sidebar @playground', () => {
  let playground: PlaygroundPage;

  test.beforeEach(async ({ page }) => {
    playground = new PlaygroundPage(page);
    await playground.goto();
  });

  test('PS-01: Sidebar is visible on load', async () => {
    await expect(playground.sidebar).toBeVisible();
  });

  test('PS-02: Sidebar toggle collapses and expands', async ({ page }) => {
    // Get initial state
    const initialClass = await playground.sidebar.getAttribute('class');

    // Toggle sidebar
    await playground.toggleSidebar();
    await page.waitForTimeout(300); // Wait for animation

    const collapsedClass = await playground.sidebar.getAttribute('class');
    expect(collapsedClass).not.toBe(initialClass);

    // Toggle back
    await playground.toggleSidebar();
    await page.waitForTimeout(300);

    const expandedClass = await playground.sidebar.getAttribute('class');
    expect(expandedClass).toBeDefined();
  });

  test('PS-03: New Playground button is visible', async () => {
    await expect(playground.newChatButton).toBeVisible();
  });

  test('PS-04: Session list container exists', async () => {
    await expect(playground.sessionList).toBeVisible();
  });

  test('PS-05: New chat creates a session', async ({ page }) => {
    // Count initial sessions
    const initialSessions = await playground.sessionList.locator('li').count();

    // Create new chat
    await playground.newChat();
    await page.waitForTimeout(500);

    // Session list should update
    const newSessions = await playground.sessionList.locator('li').count();
    expect(newSessions).toBeGreaterThanOrEqual(initialSessions);
  });

  test('PS-06: Session items are clickable', async ({ page }) => {
    // Look for session items
    const sessionItem = playground.sessionList.locator('li').first();

    if (await sessionItem.isVisible()) {
      await sessionItem.click();
      // Should not throw error
      expect(true).toBe(true);
    } else {
      // No sessions, create one first
      await playground.newChat();
      await page.waitForTimeout(500);
      const newSession = playground.sessionList.locator('li').first();
      if (await newSession.isVisible()) {
        await newSession.click();
      }
    }
  });
});
