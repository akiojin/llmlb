import { chromium } from 'playwright';

const BASE_URL = 'http://127.0.0.1:32768';

async function testPlaygroundChat() {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  
  console.log('1. Navigating to Playground...');
  await page.goto(`${BASE_URL}/playground`);
  await page.waitForLoadState('networkidle');
  
  console.log('2. Checking model dropdown...');
  const modelSelect = page.locator('#model-select');
  await modelSelect.click();
  await page.waitForTimeout(500);
  
  const options = page.locator('[role="option"]');
  const count = await options.count();
  console.log(`   Found ${count} model options`);
  
  if (count > 0) {
    const firstOption = await options.first().textContent();
    console.log(`   First option: ${firstOption}`);
    await options.first().click();
    await page.waitForTimeout(300);
  } else {
    console.log('   ERROR: No models available!');
    await browser.close();
    return;
  }
  
  console.log('3. Typing message...');
  const chatInput = page.locator('#chat-input');
  await chatInput.fill('What is 2+2?');
  
  console.log('4. Sending message...');
  const sendButton = page.locator('#send-button');
  
  const responsePromise = page.waitForResponse(
    resp => resp.url().includes('/v1/chat/completions'),
    { timeout: 30000 }
  );
  
  await sendButton.click();
  
  console.log('5. Waiting for LLM response...');
  try {
    const response = await responsePromise;
    const json = await response.json();
    console.log('6. LLM Response:');
    console.log(`   "${json.choices?.[0]?.message?.content || 'No content'}"`);
  } catch (e) {
    console.log('   Error:', e.message);
  }
  
  await browser.close();
  console.log('\nTest completed!');
}

testPlaygroundChat().catch(console.error);
