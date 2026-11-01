import { act, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { modelsApi, type RegisteredModelView } from '@/lib/api/models'
import { inferenceDefaultsApi, type InferenceDefaults } from '@/lib/api/inference-defaults'
import { deferred, renderWithProviders, viewerUser } from '@/test/render'
import { InferenceDefaultsSettings } from './InferenceDefaultsSettings'

const textModel = (name: string, ready = true): RegisteredModelView => ({
  name, ready, lifecycle_status: 'registered', tags: [], endpoint_ids: [],
  capabilities: {
    chat_completion: true, completion: true, embeddings: false, fine_tune: false,
    inference: true, text_to_speech: false, speech_to_text: false, image_generation: false,
  },
})

function stub(defaultModel: string | null = null, models = [textModel('model-a'), textModel('model-b')]) {
  const get = vi.spyOn(inferenceDefaultsApi, 'get').mockResolvedValue({ default_text_model: defaultModel })
  const catalog = vi.spyOn(modelsApi, 'getRegistered').mockResolvedValue(models)
  return { get, catalog }
}

describe('Inference defaults settings (Issue #950)', () => {
  it('keeps an unset default rather than choosing the first catalog model', async () => {
    stub()
    renderWithProviders(<InferenceDefaultsSettings />)
    expect(await screen.findByLabelText('Default text model')).toHaveValue('')
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled()
    expect(screen.getByText(/Does not change embedding, image generation, or audio defaults/)).toBeInTheDocument()
  })

  it('saves only after confirmation and locks fields until the DB response arrives', async () => {
    stub('model-a')
    const pending = deferred<InferenceDefaults>()
    const update = vi.spyOn(inferenceDefaultsApi, 'update').mockReturnValue(pending.promise)
    const user = userEvent.setup()
    renderWithProviders(<InferenceDefaultsSettings />)
    const field = await screen.findByLabelText('Default text model')
    await user.selectOptions(field, 'model-b')
    expect(screen.getByText('Unsaved changes')).toBeInTheDocument()
    expect(update).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: 'Save' }))
    await waitFor(() => expect(field).toBeDisabled())
    expect(screen.getByRole('button', { name: 'Saving...' })).toBeDisabled()
    expect(screen.queryByText(/Saved. Applies to new requests/)).not.toBeInTheDocument()
    expect(update).toHaveBeenCalledExactlyOnceWith({ default_text_model: 'model-b' })
    await act(async () => pending.resolve({ default_text_model: 'model-b' }))
    expect(await screen.findByText('Saved. Applies to new requests that omit model.')).toBeInTheDocument()
    expect(field).toHaveValue('model-b')
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled()
  })

  it('clears with an explicit null and preserves the previous choice until Save', async () => {
    stub('model-a')
    const update = vi.spyOn(inferenceDefaultsApi, 'update').mockResolvedValue({ default_text_model: null })
    const user = userEvent.setup()
    renderWithProviders(<InferenceDefaultsSettings />)
    await user.selectOptions(await screen.findByLabelText('Default text model'), '')
    expect(update).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: 'Save' }))
    await waitFor(() => expect(update).toHaveBeenCalledExactlyOnceWith({ default_text_model: null }))
    expect(await screen.findByText('Saved. Applies to new requests that omit model.')).toBeInTheDocument()
  })

  it('keeps the edited value and reports a save failure without claiming success', async () => {
    const { get } = stub('model-a')
    vi.spyOn(inferenceDefaultsApi, 'update').mockRejectedValue(new Error('Save rejected'))
    const user = userEvent.setup()
    renderWithProviders(<InferenceDefaultsSettings />)
    const field = await screen.findByLabelText('Default text model')
    await user.selectOptions(field, 'model-b')
    await user.click(screen.getByRole('button', { name: 'Save' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Failed to save inference defaults: Save rejected')
    expect(field).toHaveValue('model-b')
    expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled()
    expect(screen.queryByText(/Saved. Applies to new requests/)).not.toBeInTheDocument()
    expect(get).toHaveBeenCalledOnce()
  })

  it('retains a removed saved model and permits clearing it', async () => {
    stub('removed/model', [textModel('model-b')])
    vi.spyOn(inferenceDefaultsApi, 'update').mockResolvedValue({ default_text_model: null })
    const user = userEvent.setup()
    renderWithProviders(<InferenceDefaultsSettings />)
    const field = await screen.findByLabelText('Default text model')
    expect(field).toHaveValue('removed/model')
    expect(screen.getByRole('option', { name: 'removed/model (unavailable)' })).toBeInTheDocument()
    expect(screen.getByText(/Saved model is unavailable/)).toBeInTheDocument()
    await user.selectOptions(field, '')
    expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled()
  })

  it('offers offline text models and excludes models with only a dedicated capability', async () => {
    const embedding = textModel('embedding-only')
    embedding.capabilities = { ...embedding.capabilities!, chat_completion: false, completion: false, embeddings: true }
    stub(null, [textModel('offline-text', false), embedding])
    renderWithProviders(<InferenceDefaultsSettings />)
    await screen.findByLabelText('Default text model')
    expect(screen.getByRole('option', { name: 'offline-text (offline)' })).toBeInTheDocument()
    expect(screen.queryByRole('option', { name: 'embedding-only' })).not.toBeInTheDocument()
  })

  it('does not treat a failed GET as an unset default and can retry', async () => {
    const { get } = stub()
    get.mockRejectedValueOnce(new Error('Read failed'))
    const user = userEvent.setup()
    renderWithProviders(<InferenceDefaultsSettings />)
    expect(await screen.findByRole('alert')).toHaveTextContent('Failed to load inference defaults: Read failed')
    expect(screen.queryByLabelText('Default text model')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Retry settings' }))
    expect(await screen.findByLabelText('Default text model')).toHaveValue('')
  })

  it('keeps a saved model on catalog failure and still permits clearing', async () => {
    const { catalog } = stub('model-a')
    catalog.mockRejectedValue(new Error('Catalog failed'))
    const user = userEvent.setup()
    renderWithProviders(<InferenceDefaultsSettings />)
    const field = await screen.findByLabelText('Default text model')
    expect(field).toHaveValue('model-a')
    expect(await screen.findByRole('alert')).toHaveTextContent('Failed to load model catalog: Catalog failed')
    await user.selectOptions(field, '')
    expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled()
  })

  it('does not let a stale GET overwrite a successful save', async () => {
    const { get } = stub('model-a')
    vi.spyOn(inferenceDefaultsApi, 'update').mockResolvedValue({ default_text_model: 'model-b' })
    const user = userEvent.setup()
    const { queryClient } = renderWithProviders(<InferenceDefaultsSettings />)
    const field = await screen.findByLabelText('Default text model')
    const stale = deferred<InferenceDefaults>()
    get.mockReturnValue(stale.promise)
    void queryClient.invalidateQueries({ queryKey: ['inference-defaults'] })
    await waitFor(() => expect(get).toHaveBeenCalledTimes(2))
    await user.selectOptions(field, 'model-b')
    await user.click(screen.getByRole('button', { name: 'Save' }))
    await screen.findByText('Saved. Applies to new requests that omit model.')
    await act(async () => stale.resolve({ default_text_model: 'model-a' }))
    expect(field).toHaveValue('model-b')
    expect(queryClient.getQueryData(['inference-defaults'])).toEqual({ default_text_model: 'model-b' })
  })

  it('keeps an edited selection when that candidate disappears during a catalog refresh', async () => {
    stub('model-a')
    const user = userEvent.setup()
    const { queryClient } = renderWithProviders(<InferenceDefaultsSettings />)
    const field = await screen.findByLabelText('Default text model')
    await user.selectOptions(field, 'model-b')
    act(() => queryClient.setQueryData(['viewer-models', 'canonical'], [textModel('model-a')]))
    expect(await screen.findByRole('option', { name: 'model-b (unavailable)' })).toBeInTheDocument()
    expect(field).toHaveValue('model-b')
  })

  it('reports a background settings read failure while preserving the last saved value', async () => {
    const { get } = stub('model-a')
    const { queryClient } = renderWithProviders(<InferenceDefaultsSettings />)
    const field = await screen.findByLabelText('Default text model')
    get.mockRejectedValue(new Error('Refresh failed'))
    await act(async () => { await queryClient.invalidateQueries({ queryKey: ['inference-defaults'] }) })
    expect(await screen.findByRole('alert')).toHaveTextContent('Failed to load inference defaults: Refresh failed')
    expect(field).toHaveValue('model-a')
  })

  it.each([viewerUser, { ...viewerUser, role: 'org_manager' }, null])('does not expose settings or fetch for non-admin %j', async (user) => {
    const { get, catalog } = stub()
    renderWithProviders(<InferenceDefaultsSettings />, { user })
    await act(async () => Promise.resolve())
    expect(screen.queryByLabelText('Default text model')).not.toBeInTheDocument()
    expect(get).not.toHaveBeenCalled()
    expect(catalog).not.toHaveBeenCalled()
  })
})
