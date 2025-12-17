import { chromium } from '@playwright/test';

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage();

// Capture console logs
page.on('console', msg => {
  console.log('CONSOLE ' + msg.type().toUpperCase() + ':', msg.text());
});

// Capture page errors
page.on('pageerror', err => {
  console.error('PAGE ERROR:', err.message);
});

console.log('Navigating to login page...');
await page.goto('http://localhost:8080/dashboard/login.html', { waitUntil: 'networkidle', timeout: 30000 });

// Wait a bit for any lazy loading
await page.waitForTimeout(3000);

// Take a screenshot
await page.screenshot({ path: '/tmp/debug-login.png', fullPage: true });
console.log('Screenshot saved to /tmp/debug-login.png');

// Check if any elements exist
const rootContent = await page.locator('#root').innerHTML();
console.log('Root content length:', rootContent.length);
if (rootContent.length > 0) {
  console.log('Root content preview:', rootContent.substring(0, 500));
} else {
  console.log('Root element is empty - React app did not render');
}

await browser.close();
