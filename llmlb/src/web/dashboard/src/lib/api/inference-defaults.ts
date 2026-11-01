import { fetchWithAuth } from './client'

/** Global text-generation default. Null keeps model mandatory. */
export interface InferenceDefaults {
  default_text_model: string | null
}

function readDefaults(response: unknown): InferenceDefaults {
  if (response && typeof response === 'object' && 'default_text_model' in response) {
    const value = response.default_text_model
    if (value === null || typeof value === 'string') return { default_text_model: value }
  }
  throw new Error('Invalid inference defaults response')
}

export const inferenceDefaultsApi = {
  get: async () => readDefaults(await fetchWithAuth<unknown>('/api/dashboard/inference-defaults')),
  update: async (defaults: InferenceDefaults) =>
    readDefaults(await fetchWithAuth<unknown>('/api/dashboard/inference-defaults', {
      method: 'PUT',
      body: JSON.stringify(defaults),
    })),
}
