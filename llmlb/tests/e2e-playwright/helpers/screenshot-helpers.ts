import { type Page } from '@playwright/test';
import * as path from 'path';

const SCREENSHOTS_DIR = path.join('reports', 'screenshots');

/**
 * Captures a full-page screenshot and saves it to reports/screenshots/.
 *
 * @param page - Playwright page instance
 * @param name - Screenshot name (without extension)
 */
export async function captureScreen(page: Page, name: string): Promise<string> {
  const filePath = path.join(SCREENSHOTS_DIR, `${name}.png`);
  await page.screenshot({ path: filePath, fullPage: true });
  return filePath;
}

/**
 * Captures a screenshot of a specific element.
 *
 * @param page - Playwright page instance
 * @param selector - CSS selector for the element
 * @param name - Screenshot name (without extension)
 */
export async function captureElement(
  page: Page,
  selector: string,
  name: string
): Promise<string> {
  const filePath = path.join(SCREENSHOTS_DIR, `${name}.png`);
  const element = page.locator(selector);
  await element.screenshot({ path: filePath });
  return filePath;
}
