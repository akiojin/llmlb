import { randomUUID } from 'node:crypto';
import { test as base, expect } from '@playwright/test';
import { deleteEndpoint } from './api-helpers';
import { startMockOpenAIEndpointServer } from './mock-openai-endpoint';

export interface TestEndpoint {
  id: string;
  name: string;
  model: string;
}

/** Each consumer owns its endpoint/model, even with fullyParallel enabled. */
export const test = base.extend<{ endpoint: TestEndpoint }>({
  endpoint: async ({ request }, use) => {
    const name = `e2e-dashboard-${randomUUID()}`;
    const model = `${name}-model`;
    const mock = await startMockOpenAIEndpointServer({ models: [model] });
    const headers = { Authorization: 'Bearer sk_debug' };
    let id: string | undefined;
    try {
      const created = await request.post('/api/endpoints', {
        headers,
        data: { name, base_url: mock.baseUrl },
      });
      expect(created.ok(), 'fixture endpoint creation must succeed').toBeTruthy();
      id = ((await created.json()) as { id: string }).id;
      expect(id).toBeTruthy();
      for (const action of ['test', 'sync']) {
        const response = await request.post(`/api/endpoints/${id}/${action}`, { headers });
        expect(response.ok(), `fixture endpoint ${action} must succeed`).toBeTruthy();
        if (action === 'test') {
          expect((await response.json()).success, 'fixture connection must be healthy').toBe(true);
        }
      }
      await use({ id, name, model });
    } finally {
      try {
        if (id) expect(await deleteEndpoint(request, id), 'fixture cleanup must succeed').toBe(true);
      } finally {
        await mock.close();
      }
    }
  },
});

export { expect };
