import type { ComponentProps } from 'react'
import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  dashboardApi,
  endpointsApi,
  type DashboardEndpoint,
  type ModelStatEntry,
  type ModelTpsEntry,
  type RegisteredModelView,
} from '@/lib/api'
import { renderWithProviders, viewerUser, type TestUser } from '@/test/render'
import { ModelsTable } from './ModelsTable'

function model(overrides: Partial<RegisteredModelView> = {}): RegisteredModelView {
  return {
    name: 'gpt-oss:20b',
    lifecycle_status: 'registered',
    ready: true,
    tags: [],
    endpoint_ids: [],
    ...overrides,
  }
}

function endpoint(overrides: Partial<DashboardEndpoint> = {}): DashboardEndpoint {
  return {
    id: 'ep-1',
    name: 'xllm-gpu-1',
    base_url: 'http://192.0.2.20:8080',
    status: 'online',
    endpoint_type: 'xllm',
    health_check_interval_secs: 30,
    inference_timeout_secs: 120,
    error_count: 0,
    registered_at: '2026-10-01T00:00:00Z',
    model_count: 1,
    total_requests: 0,
    successful_requests: 0,
    failed_requests: 0,
    ...overrides,
  }
}

function stat(overrides: Partial<ModelStatEntry> = {}): ModelStatEntry {
  return {
    model_id: 'gpt-oss:20b',
    total_requests: 0,
    successful_requests: 0,
    failed_requests: 0,
    ...overrides,
  }
}

function tps(overrides: Partial<ModelTpsEntry> = {}): ModelTpsEntry {
  return {
    model_id: 'gpt-oss:20b',
    api_kind: 'chat_completions',
    source: 'production',
    tps: 42.5,
    request_count: 3,
    total_output_tokens: 300,
    average_duration_ms: 100,
    ...overrides,
  }
}

type Props = ComponentProps<typeof ModelsTable>

/** Renders the table and waits for the session check and stats request to settle. */
async function renderTable(props: Partial<Props> = {}, user?: TestUser) {
  const view = renderWithProviders(
    <ModelsTable models={[]} endpoints={[]} isLoading={false} {...props} />,
    { user },
  )
  await waitFor(() => expect(view.queryClient.isFetching()).toBe(0))
  return view
}

function rowFor(modelId: string) {
  return screen.getByRole('row', { name: (name) => name.includes(modelId) })
}

const fleet = [
  model({ name: 'alpha-chat', supported_apis: ['chat'] }),
  model({ name: 'bravo-embed', supported_apis: ['embeddings'], lifecycle_status: 'error' }),
  model({ name: 'charlie-tts', supported_apis: ['tts'], lifecycle_status: 'pending' }),
]

/** Model IDs of `fleet` in the order the table lists them. */
function listedModels() {
  return screen.queryAllByText(/^(alpha-chat|bravo-embed|charlie-tts)$/).map((el) => el.textContent)
}

beforeEach(() => {
  vi.spyOn(dashboardApi, 'getAllModelStats').mockResolvedValue([])
  vi.spyOn(endpointsApi, 'getModelStats').mockResolvedValue([])
  vi.spyOn(endpointsApi, 'getModelTps').mockResolvedValue([])
})

afterEach(() => {
  window.location.hash = ''
})

describe('ModelsTable', () => {
  it('renders a row per model with its status, endpoint count and routed requests', async () => {
    vi.spyOn(dashboardApi, 'getAllModelStats').mockResolvedValue([
      stat({ total_requests: 42, successful_requests: 40, failed_requests: 2 }),
    ])
    await renderTable({
      models: [
        model({ endpoint_ids: ['ep-1', 'ep-2'] }),
        model({ name: 'nomic-embed-text', lifecycle_status: 'caching', ready: false }),
      ],
    })

    const served = rowFor('gpt-oss:20b')
    expect(await within(served).findByText('42')).toBeInTheDocument()
    expect(within(served).getByText('Registered')).toBeInTheDocument()
    expect(within(served).getByTitle('Ready')).toBeInTheDocument()
    expect(within(served).getByText('2')).toBeInTheDocument()

    const caching = rowFor('nomic-embed-text')
    expect(within(caching).getByText('Caching')).toBeInTheDocument()
    expect(within(caching).getByTitle('Not Ready')).toBeInTheDocument()
  })

  it('shows the APIs a model supports, falling back to its capabilities', async () => {
    await renderTable({
      models: [
        model({ name: 'by-apis', supported_apis: ['chat', 'embeddings'] }),
        model({
          name: 'by-capabilities',
          capabilities: {
            chat_completion: false,
            completion: false,
            embeddings: false,
            fine_tune: false,
            inference: false,
            text_to_speech: true,
            speech_to_text: false,
            image_generation: false,
          },
        }),
        model({ name: 'unreported' }),
      ],
    })

    const byApis = within(rowFor('by-apis'))
    expect(byApis.getByText('Chat')).toBeInTheDocument()
    expect(byApis.getByText('Embed')).toBeInTheDocument()
    expect(byApis.queryByText('TTS')).not.toBeInTheDocument()
    expect(within(rowFor('by-capabilities')).getByText('TTS')).toBeInTheDocument()
    expect(within(rowFor('unreported')).getByText('Not reported')).toBeInTheDocument()
  })

  it('marks canonical models and lists the raw names they aggregate', async () => {
    await renderTable({
      models: [
        model({
          name: 'gpt-oss:20b',
          canonical_name: 'gpt-oss:20b',
          is_canonical: true,
          aliases: ['gpt-oss-20b-GGUF'],
        }),
        model({ name: 'raw-llama-q4', canonical_name: 'llama3.2:3b' }),
      ],
    })

    const canonical = within(rowFor('gpt-oss:20b'))
    expect(canonical.getByText('canonical')).toBeInTheDocument()
    expect(canonical.getByText('gpt-oss-20b-GGUF')).toBeInTheDocument()
    expect(within(rowFor('raw-llama-q4')).getByText('llama3.2:3b')).toBeInTheDocument()
  })

  it('shows neither the table nor the empty state while the first load is pending', async () => {
    await renderTable({ isLoading: true })

    expect(screen.getByText('Models')).toBeInTheDocument()
    expect(screen.queryByRole('table')).not.toBeInTheDocument()
    expect(screen.queryByText('No models registered')).not.toBeInTheDocument()
  })

  it('keeps the rows visible while a refresh is in flight', async () => {
    await renderTable({ isLoading: true, models: [model()] })

    expect(rowFor('gpt-oss:20b')).toBeInTheDocument()
  })

  it('shows the empty state when no models are registered', async () => {
    await renderTable()

    expect(screen.getByText('No models registered')).toBeInTheDocument()
    expect(
      screen.getByText('Register a model or connect an endpoint that serves models.'),
    ).toBeInTheDocument()
  })

  it('filters rows by model ID and reports when nothing matches', async () => {
    const user = userEvent.setup()
    await renderTable({ models: fleet })
    const search = screen.getByPlaceholderText('Search by model ID...')

    await user.type(search, 'BRAVO')
    expect(listedModels()).toEqual(['bravo-embed'])

    await user.clear(search)
    await user.type(search, 'no-such-model')
    expect(listedModels()).toEqual([])
    expect(screen.getByText('No models match the filter criteria')).toBeInTheDocument()
  })

  it('filters rows by lifecycle status', async () => {
    const user = userEvent.setup()
    await renderTable({ models: fleet })

    await user.click(screen.getByRole('combobox'))
    await user.click(await screen.findByRole('option', { name: 'Error' }))

    expect(listedModels()).toEqual(['bravo-embed'])
  })

  it('filters rows by supported API', async () => {
    const user = userEvent.setup()
    await renderTable({ models: fleet })

    await user.click(screen.getByRole('button', { name: 'APIs' }))
    await user.click(await screen.findByRole('menuitemcheckbox', { name: 'TTS' }))

    await waitFor(() => expect(listedModels()).toEqual(['charlie-tts']))
  })

  it('sorts by model ID and by routed requests', async () => {
    vi.spyOn(dashboardApi, 'getAllModelStats').mockResolvedValue([
      stat({ model_id: 'alpha-chat', total_requests: 5 }),
      stat({ model_id: 'bravo-embed', total_requests: 90 }),
      stat({ model_id: 'charlie-tts', total_requests: 30 }),
    ])
    const user = userEvent.setup()
    await renderTable({ models: [fleet[1], fleet[2], fleet[0]] })

    expect(listedModels()).toEqual(['alpha-chat', 'bravo-embed', 'charlie-tts'])

    await user.click(screen.getByRole('columnheader', { name: 'Model ID' }))
    expect(listedModels()).toEqual(['charlie-tts', 'bravo-embed', 'alpha-chat'])

    await user.click(screen.getByRole('columnheader', { name: 'Routed Requests' }))
    expect(listedModels()).toEqual(['alpha-chat', 'charlie-tts', 'bravo-embed'])
  })

  it('adds an optional column from the Columns menu', async () => {
    const user = userEvent.setup()
    await renderTable({ models: [model({ description: 'Open-weight reasoning model' })] })
    expect(screen.queryByText('Open-weight reasoning model')).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Columns' }))
    await user.click(await screen.findByRole('menuitemcheckbox', { name: 'Description' }))

    expect(await screen.findByRole('columnheader', { name: 'Description' })).toBeInTheDocument()
    expect(screen.getByText('Open-weight reasoning model')).toBeInTheDocument()
  })

  it('reflects the view mode and switches it through onViewChange', async () => {
    const onViewChange = vi.fn()
    const user = userEvent.setup()
    await renderTable({ view: 'canonical', onViewChange })

    const toggle = within(screen.getByRole('group', { name: 'Model view mode' }))
    expect(toggle.getByRole('button', { name: 'Canonical' })).toHaveAttribute('aria-pressed', 'true')
    expect(toggle.getByRole('button', { name: 'Detail' })).toHaveAttribute('aria-pressed', 'false')

    await user.click(toggle.getByRole('button', { name: 'Detail' }))

    expect(onViewChange).toHaveBeenCalledExactlyOnceWith('detail')
  })

  it('calls onRefresh from the Refresh button', async () => {
    const onRefresh = vi.fn()
    await renderTable({ onRefresh })

    await userEvent.setup().click(screen.getByRole('button', { name: 'Refresh' }))

    expect(onRefresh).toHaveBeenCalledOnce()
  })

  it('expands a row to show the endpoints serving the model with their stats', async () => {
    const getModelStats = vi.spyOn(endpointsApi, 'getModelStats').mockResolvedValue([
      stat({ total_requests: 12, successful_requests: 10, failed_requests: 2 }),
      stat({ model_id: 'other-model', total_requests: 99 }),
    ])
    vi.spyOn(endpointsApi, 'getModelTps').mockResolvedValue([
      tps(),
      tps({ api_kind: 'completions', source: 'benchmark', tps: 99.9 }),
      tps({ model_id: 'other-model', tps: 7 }),
    ])
    const user = userEvent.setup()
    await renderTable({
      models: [model({ endpoint_ids: ['ep-1'] })],
      endpoints: [endpoint(), endpoint({ id: 'ep-2', name: 'vllm-other' })],
    })
    expect(screen.queryByText('xllm-gpu-1')).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Expand row' }))

    expect(screen.getByText('Endpoints (1 source)')).toBeInTheDocument()
    expect(screen.getByText('xllm-gpu-1')).toBeInTheDocument()
    expect(screen.queryByText('vllm-other')).not.toBeInTheDocument()
    expect(await screen.findByText('Total: 12')).toBeInTheDocument()
    expect(screen.getByText('OK: 10')).toBeInTheDocument()
    expect(screen.getByText('Fail: 2')).toBeInTheDocument()
    expect(await screen.findByText('TPS: chat 42.5 tok/s')).toBeInTheDocument()
    expect(screen.getByRole('link')).toHaveAttribute('href', '#playground/ep-1')
    expect(getModelStats).toHaveBeenCalledWith('ep-1')

    await user.click(screen.getByRole('button', { name: 'Collapse row' }))

    expect(screen.queryByText('xllm-gpu-1')).not.toBeInTheDocument()
  })

  it('explains when no registered endpoint serves an expanded model', async () => {
    await renderTable({ models: [model()], endpoints: [endpoint()] })

    await userEvent.setup().click(screen.getByRole('button', { name: 'Expand row' }))

    expect(screen.getByText('No endpoints currently serve this model')).toBeInTheDocument()
  })

  it('deletes a model from an endpoint after confirmation', async () => {
    const deleteModel = vi.spyOn(endpointsApi, 'deleteModel').mockResolvedValue(undefined)
    const user = userEvent.setup()
    await renderTable({ models: [model({ endpoint_ids: ['ep-1'] })], endpoints: [endpoint()] })

    await user.click(screen.getByRole('button', { name: 'Expand row' }))
    await user.click(screen.getByRole('button', { name: 'Delete model from endpoint' }))
    const dialog = await screen.findByRole('dialog', { name: 'Delete Model' })
    expect(dialog).toHaveTextContent(
      'This action will permanently remove gpt-oss:20b from xllm-gpu-1.',
    )
    expect(deleteModel).not.toHaveBeenCalled()

    await user.click(within(dialog).getByRole('button', { name: 'Delete' }))

    expect(await screen.findByText('Model deleted')).toBeInTheDocument()
    expect(deleteModel).toHaveBeenCalledExactlyOnceWith('ep-1', 'gpt-oss:20b')
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('reports a failed deletion and keeps the confirmation open', async () => {
    vi.spyOn(endpointsApi, 'deleteModel').mockRejectedValue(new Error('endpoint unreachable'))
    const user = userEvent.setup()
    await renderTable({ models: [model({ endpoint_ids: ['ep-1'] })], endpoints: [endpoint()] })

    await user.click(screen.getByRole('button', { name: 'Expand row' }))
    await user.click(screen.getByRole('button', { name: 'Delete model from endpoint' }))
    const dialog = await screen.findByRole('dialog', { name: 'Delete Model' })
    await user.click(within(dialog).getByRole('button', { name: 'Delete' }))

    expect(await screen.findByText('Delete failed')).toBeInTheDocument()
    expect(screen.getByText('endpoint unreachable')).toBeInTheDocument()
    expect(screen.getByRole('dialog', { name: 'Delete Model' })).toBeInTheDocument()
  })

  it('does not delete when the confirmation is cancelled', async () => {
    const deleteModel = vi.spyOn(endpointsApi, 'deleteModel').mockResolvedValue(undefined)
    const user = userEvent.setup()
    await renderTable({ models: [model({ endpoint_ids: ['ep-1'] })], endpoints: [endpoint()] })

    await user.click(screen.getByRole('button', { name: 'Expand row' }))
    await user.click(screen.getByRole('button', { name: 'Delete model from endpoint' }))
    const dialog = await screen.findByRole('dialog', { name: 'Delete Model' })
    await user.click(within(dialog).getByRole('button', { name: 'Cancel' }))

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(deleteModel).not.toHaveBeenCalled()
  })

  it('offers deletion only on endpoint types that support it', async () => {
    await renderTable({
      models: [model({ endpoint_ids: ['ep-1'] })],
      endpoints: [endpoint({ name: 'vllm-gpu-1', endpoint_type: 'vllm' })],
    })

    await userEvent.setup().click(screen.getByRole('button', { name: 'Expand row' }))

    expect(screen.getByText('vllm-gpu-1')).toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: 'Delete model from endpoint' }),
    ).not.toBeInTheDocument()
  })

  it('opens the playground for a ready model without expanding its row', async () => {
    await renderTable({
      models: [model(), model({ name: 'still-caching', lifecycle_status: 'caching', ready: false })],
    })
    const ready = within(rowFor('gpt-oss:20b'))

    await userEvent.setup().click(ready.getByRole('button', { name: 'Open in playground' }))

    expect(window.location.hash).toBe('#lb-playground?model=gpt-oss%3A20b')
    expect(ready.getByRole('button', { name: 'Expand row' })).toBeInTheDocument()
    expect(
      within(rowFor('still-caching')).getByRole('button', { name: 'Open in playground' }),
    ).toBeDisabled()
  })

  it('opens the add-model wizard', async () => {
    await renderTable()

    await userEvent.setup().click(screen.getByRole('button', { name: 'Add Model' }))

    expect(
      await screen.findByRole('dialog', { name: 'Search HuggingFace Models' }),
    ).toBeInTheDocument()
  })

  describe('viewer mode', () => {
    it('renders a read-only list without management controls or stats requests', async () => {
      const getAllModelStats = vi.spyOn(dashboardApi, 'getAllModelStats')
      await renderTable(
        {
          viewerMode: true,
          models: [model({ description: 'Open-weight reasoning model', endpoint_ids: ['ep-1'] })],
          endpoints: [endpoint()],
        },
        viewerUser,
      )

      const row = within(rowFor('gpt-oss:20b'))
      expect(row.getByText('Registered')).toBeInTheDocument()
      expect(row.getByText('Open-weight reasoning model')).toBeInTheDocument()
      expect(screen.getAllByRole('columnheader').map((th) => th.textContent)).toEqual([
        'Model ID',
        'Status',
        'Description',
      ])
      expect(screen.queryByRole('button', { name: 'Add Model' })).not.toBeInTheDocument()
      expect(screen.queryByRole('button', { name: 'Expand row' })).not.toBeInTheDocument()
      expect(screen.queryByRole('button', { name: 'Open in playground' })).not.toBeInTheDocument()
      expect(screen.queryByRole('button', { name: 'Columns' })).not.toBeInTheDocument()
      expect(screen.queryByRole('combobox')).not.toBeInTheDocument()
      expect(getAllModelStats).not.toHaveBeenCalled()
    })

    it('filters rows by model ID and reports when nothing matches', async () => {
      const user = userEvent.setup()
      await renderTable({ viewerMode: true, models: fleet }, viewerUser)
      const search = screen.getByPlaceholderText('Search by model ID...')

      await user.type(search, 'charlie')
      expect(listedModels()).toEqual(['charlie-tts'])

      await user.clear(search)
      await user.type(search, 'no-such-model')
      expect(listedModels()).toEqual([])
      expect(screen.getByText('No models match your search')).toBeInTheDocument()
    })

    it('shows the empty state when no models are registered', async () => {
      await renderTable({ viewerMode: true }, viewerUser)

      expect(screen.getByText('No models registered')).toBeInTheDocument()
    })

    it('keeps the view switch and refresh available', async () => {
      const onViewChange = vi.fn()
      const onRefresh = vi.fn()
      const user = userEvent.setup()
      await renderTable({ viewerMode: true, view: 'detail', onViewChange, onRefresh }, viewerUser)

      await user.click(screen.getByRole('button', { name: 'Canonical' }))
      await user.click(screen.getByRole('button', { name: 'Refresh' }))

      expect(onViewChange).toHaveBeenCalledExactlyOnceWith('canonical')
      expect(onRefresh).toHaveBeenCalledOnce()
    })
  })
})
