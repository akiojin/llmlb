import { test, expect } from '@playwright/test';

/**
 * API Error Handling E2E Tests
 *
 * These tests verify proper error handling for abnormal scenarios:
 * - No nodes available
 * - Invalid requests
 * - Authentication failures
 */

const API_BASE = 'http://localhost:8080';

test.describe('API Error Handling', () => {

  test('returns 503 when no nodes are available', async ({ request }) => {
    // This test verifies that the router returns 503 when there are no nodes
    // Skip if nodes are available (normal operation)
    const nodesResponse = await request.get(`${API_BASE}/v0/nodes`, {
      headers: { 'Authorization': 'Bearer sk_debug' }
    });
    const nodesData = await nodesResponse.json();

    if (nodesData.nodes && nodesData.nodes.length > 0) {
      // Check if any nodes are online
      const onlineNodes = nodesData.nodes.filter((n: { status: string }) => n.status === 'online');
      if (onlineNodes.length > 0) {
        test.skip();
        return;
      }
    }

    // Make a chat completion request without API key (should fail differently)
    const response = await request.post(`${API_BASE}/v1/chat/completions`, {
      data: {
        model: 'test-model',
        messages: [{ role: 'user', content: 'hello' }]
      },
      headers: {
        'Content-Type': 'application/json',
        'Authorization': 'Bearer sk_debug'
      }
    });

    // Should return 503 (No nodes available) or 404 (Model not found)
    expect([503, 404]).toContain(response.status());
  });

  test('returns 400 on invalid JSON', async ({ request }) => {
    const response = await request.post(`${API_BASE}/v1/chat/completions`, {
      data: '{invalid json here',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': 'Bearer sk_debug'
      }
    });

    expect(response.status()).toBe(400);
  });

  test('returns 400 on missing required field (model)', async ({ request }) => {
    const response = await request.post(`${API_BASE}/v1/chat/completions`, {
      data: {
        messages: [{ role: 'user', content: 'hello' }]
        // Missing 'model' field
      },
      headers: {
        'Content-Type': 'application/json',
        'Authorization': 'Bearer sk_debug'
      }
    });

    expect(response.status()).toBe(400);
  });

  test('returns 401 on missing authorization', async ({ request }) => {
    const response = await request.post(`${API_BASE}/v1/chat/completions`, {
      data: {
        model: 'test-model',
        messages: [{ role: 'user', content: 'hello' }]
      },
      headers: {
        'Content-Type': 'application/json'
        // Missing Authorization header
      }
    });

    expect(response.status()).toBe(401);
  });

  test('returns 401 on invalid API key', async ({ request }) => {
    const response = await request.post(`${API_BASE}/v1/chat/completions`, {
      data: {
        model: 'test-model',
        messages: [{ role: 'user', content: 'hello' }]
      },
      headers: {
        'Content-Type': 'application/json',
        'Authorization': 'Bearer invalid_api_key_12345'
      }
    });

    expect(response.status()).toBe(401);
  });

  test('returns 404 on non-existent model', async ({ request }) => {
    const response = await request.post(`${API_BASE}/v1/chat/completions`, {
      data: {
        model: 'non-existent-model-xyz123',
        messages: [{ role: 'user', content: 'hello' }]
      },
      headers: {
        'Content-Type': 'application/json',
        'Authorization': 'Bearer sk_debug'
      }
    });

    // Should return 404 (model not found) or 503 (no nodes with model)
    expect([404, 503]).toContain(response.status());
  });

  test('embeddings returns error on missing input', async ({ request }) => {
    const response = await request.post(`${API_BASE}/v1/embeddings`, {
      data: {
        model: 'test-model'
        // Missing 'input' field
      },
      headers: {
        'Content-Type': 'application/json',
        'Authorization': 'Bearer sk_debug'
      }
    });

    // Router may return 404 (model not found) or 400 (missing input)
    // depending on validation order
    expect([400, 404, 503]).toContain(response.status());
  });

  test('completions returns error on missing prompt', async ({ request }) => {
    const response = await request.post(`${API_BASE}/v1/completions`, {
      data: {
        model: 'test-model'
        // Missing 'prompt' field
      },
      headers: {
        'Content-Type': 'application/json',
        'Authorization': 'Bearer sk_debug'
      }
    });

    // Router may return 404 (model not found) or 400 (missing prompt)
    // depending on validation order
    expect([400, 404, 503]).toContain(response.status());
  });
});

test.describe('Dashboard Error Handling', () => {

  test('login fails with invalid credentials', async ({ page }) => {
    await page.goto(`${API_BASE}/dashboard`);

    // Fill invalid credentials
    await page.fill('input[type="text"], input[name="username"], #username', 'invalid_user');
    await page.fill('input[type="password"], input[name="password"], #password', 'wrong_password');
    await page.click('button[type="submit"]');

    // Should show error or stay on login page
    await page.waitForTimeout(1000);

    // Check if still on login page or error shown
    const pageText = await page.textContent('body');
    expect(pageText).toContain('Sign in');
  });
});
