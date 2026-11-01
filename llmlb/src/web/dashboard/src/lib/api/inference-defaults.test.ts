import { describe, expect, it, vi } from 'vitest'
import { inferenceDefaultsApi } from './inference-defaults'
import * as client from './client'

describe('inference defaults API wrapper', () => {
  it('uses the authenticated dedicated GET and nullable PUT contract', async () => {
    const fetch = vi.spyOn(client, 'fetchWithAuth').mockResolvedValue({ default_text_model: null })
    await inferenceDefaultsApi.get()
    expect(fetch).toHaveBeenLastCalledWith('/api/dashboard/inference-defaults')
    await inferenceDefaultsApi.update({ default_text_model: 'model-a' })
    expect(fetch).toHaveBeenLastCalledWith('/api/dashboard/inference-defaults', {
      method: 'PUT', body: '{"default_text_model":"model-a"}',
    })
    await inferenceDefaultsApi.update({ default_text_model: null })
    expect(fetch).toHaveBeenLastCalledWith('/api/dashboard/inference-defaults', {
      method: 'PUT', body: '{"default_text_model":null}',
    })
  })

  it.each([{}, null, { default_text_model: 42 }])('rejects malformed response %j without reporting a successful clear', async (response) => {
    vi.spyOn(client, 'fetchWithAuth').mockResolvedValue(response)
    await expect(inferenceDefaultsApi.get()).rejects.toThrow('Invalid inference defaults response')
    await expect(inferenceDefaultsApi.update({ default_text_model: 'model-a' })).rejects.toThrow('Invalid inference defaults response')
  })
})
