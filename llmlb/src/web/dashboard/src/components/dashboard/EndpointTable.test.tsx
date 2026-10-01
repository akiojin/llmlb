import { screen, waitFor, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { endpointsApi, type DashboardEndpoint } from '@/lib/api'
import { deferred, renderWithProviders } from '@/test/render'
import { EndpointTable } from './EndpointTable'

function endpoint(overrides: Partial<DashboardEndpoint> = {}): DashboardEndpoint {
  return {
    id: 'ep-alpha',
    name: 'alpha',
    base_url: 'http://alpha.test:8080',
    status: 'online',
    endpoint_type: 'xllm',
    health_check_interval_secs: 30,
    inference_timeout_secs: 600,
    error_count: 0,
    registered_at: '2026-10-01T00:00:00Z',
    model_count: 0,
    total_requests: 0,
    successful_requests: 0,
    failed_requests: 0,
    ...overrides,
  }
}

const alpha = endpoint({
  latency_ms: 42,
  model_count: 3,
  total_requests: 200,
  successful_requests: 190,
  failed_requests: 10,
})
const beta = endpoint({
  id: 'ep-beta',
  name: 'beta',
  base_url: 'http://beta.test:11434',
  status: 'offline',
  endpoint_type: 'ollama',
  total_requests: 50,
  successful_requests: 50,
})
const gamma = endpoint({
  id: 'ep-gamma',
  name: 'gamma',
  base_url: 'http://gamma.test:8000',
  status: 'pending',
  endpoint_type: 'vllm',
  total_requests: 900,
  successful_requests: 900,
})

function renderTable(endpoints: DashboardEndpoint[], isLoading = false) {
  return renderWithProviders(<EndpointTable endpoints={endpoints} isLoading={isLoading} />)
}

function rowFor(name: string) {
  return screen.getByRole('cell', { name }).closest('tr') as HTMLElement
}

/** Endpoint names in the order the body rows are displayed. */
function rowNames() {
  return screen
    .getAllByRole('row')
    .slice(1)
    .map((row) => within(row).getAllByRole('cell')[0].textContent)
}

// The filter selects have no accessible name, so each one is located by the
// value it currently displays.
async function chooseFilter(user: UserEvent, current: string, option: string) {
  const select = screen.getAllByRole('combobox').find((el) => el.textContent === current)
  if (!select) throw new Error(`No select is showing "${current}"`)
  await user.click(select)
  await user.click(await screen.findByRole('option', { name: option }))
}

// Fills a dialog field in one input event. Typing re-renders the whole table
// on every keystroke, which is slow enough under jsdom to risk the timeout.
async function fill(user: UserEvent, label: string, value: string) {
  await user.click(screen.getByLabelText(label))
  await user.paste(value)
}

describe('EndpointTable', () => {
  it('shows no table and no empty state while the first load is pending', () => {
    renderTable([], true)

    expect(screen.getByText('Endpoints')).toBeInTheDocument()
    expect(screen.queryByRole('table')).not.toBeInTheDocument()
    expect(screen.queryByText('No endpoints registered')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Add Endpoint' })).not.toBeInTheDocument()
  })

  it('keeps the rows on screen during a background refresh', () => {
    renderTable([alpha, beta], true)

    expect(rowNames()).toHaveLength(2)
  })

  it('renders a row per endpoint', () => {
    renderTable([alpha, beta])

    const online = within(rowFor('alpha'))
    expect(online.getByText('http://alpha.test:8080')).toBeInTheDocument()
    expect(online.getByText('xLLM')).toBeInTheDocument()
    expect(online.getByText('Online')).toBeInTheDocument()
    expect(online.getByText('200 (95.0%)')).toBeInTheDocument()
    expect(online.getByText('42ms')).toBeInTheDocument()
    expect(online.getAllByRole('cell')[6]).toHaveTextContent('3')

    const offline = within(rowFor('beta'))
    expect(offline.getByText('http://beta.test:11434')).toBeInTheDocument()
    expect(offline.getByText('Ollama')).toBeInTheDocument()
    expect(offline.getByText('Offline')).toBeInTheDocument()
    // No latency measurement and no health check yet.
    expect(offline.getAllByRole('cell')[5]).toHaveTextContent('-')
    expect(offline.getAllByRole('cell')[7]).toHaveTextContent('-')
  })

  it('flags an endpoint whose last health check failed', () => {
    renderTable([
      alpha,
      endpoint({
        id: 'ep-down',
        name: 'down',
        status: 'error',
        last_error: 'connection refused',
        error_count: 3,
      }),
    ])

    const failing = within(rowFor('down'))
    expect(failing.getByText('(3 errors)')).toBeInTheDocument()
    expect(failing.getByText('Connection')).toBeInTheDocument()
    expect(within(rowFor('alpha')).queryByText(/errors\)/)).not.toBeInTheDocument()
  })

  it('shows the empty state when no endpoints are registered', () => {
    renderTable([])

    expect(screen.getByRole('heading', { name: 'No endpoints registered' })).toBeInTheDocument()
    expect(
      screen.getByText('Add an endpoint to start routing inference requests.'),
    ).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Add Endpoint' })).toBeEnabled()
  })

  it('filters by name or URL as the user types', async () => {
    const user = userEvent.setup()
    renderTable([alpha, beta, gamma])
    const search = screen.getByPlaceholderText('Search by name or URL...')

    await user.type(search, 'BET')
    expect(rowNames()).toEqual(['beta'])

    await user.clear(search)
    await user.type(search, ':8000')
    expect(rowNames()).toEqual(['gamma'])
  })

  it('shows the filter empty state when the search matches nothing', async () => {
    renderTable([alpha, beta])

    await userEvent.setup().type(screen.getByPlaceholderText('Search by name or URL...'), 'zzz')

    expect(
      screen.getByRole('heading', { name: 'No endpoints match the filter criteria' }),
    ).toBeInTheDocument()
    expect(screen.queryByText('No endpoints registered')).not.toBeInTheDocument()
  })

  it('filters by status', async () => {
    const user = userEvent.setup()
    renderTable([alpha, beta, gamma])

    await chooseFilter(user, 'All Status', 'Offline')
    expect(rowNames()).toEqual(['beta'])

    await chooseFilter(user, 'Offline', 'Error')
    expect(
      screen.getByRole('heading', { name: 'No endpoints match the filter criteria' }),
    ).toBeInTheDocument()
  })

  it('filters by endpoint type', async () => {
    const user = userEvent.setup()
    renderTable([alpha, beta, gamma])

    await chooseFilter(user, 'All Types', 'vLLM')

    expect(rowNames()).toEqual(['gamma'])
  })

  it('sorts by a column and reverses the order on a second click', async () => {
    const user = userEvent.setup()
    renderTable([alpha, beta, gamma])
    const requests = screen.getByRole('columnheader', { name: 'Requests' })

    await user.click(requests)
    expect(rowNames()).toEqual(['gamma', 'alpha', 'beta'])

    await user.click(requests)
    expect(rowNames()).toEqual(['beta', 'alpha', 'gamma'])
  })

  it('pages through more than ten endpoints', async () => {
    const user = userEvent.setup()
    const names = Array.from({ length: 12 }, (_, i) => `node-${String(i + 1).padStart(2, '0')}`)
    renderTable(names.map((name) => endpoint({ id: `ep-${name}`, name })))

    expect(screen.getByText('Showing 1 - 10 of 12')).toBeInTheDocument()
    expect(rowNames()).toEqual(names.slice(0, 10))

    // The pager buttons are icon-only and have no accessible name; they are
    // the two buttons around the page indicator.
    const pager = screen.getByText('1 / 2').parentElement as HTMLElement
    const [previous, next] = within(pager).getAllByRole('button')
    expect(previous).toBeDisabled()

    await user.click(next)
    expect(screen.getByText('Showing 11 - 12 of 12')).toBeInTheDocument()
    expect(rowNames()).toEqual(names.slice(10))
    expect(next).toBeDisabled()

    await user.click(previous)
    expect(rowNames()).toEqual(names.slice(0, 10))
  })

  it('opens the detail modal of the chosen endpoint', async () => {
    vi.spyOn(endpointsApi, 'getTodayStats').mockResolvedValue({
      date: '2026-10-01',
      total_requests: 0,
      successful_requests: 0,
      failed_requests: 0,
    })
    vi.spyOn(endpointsApi, 'getDailyStats').mockResolvedValue([])
    vi.spyOn(endpointsApi, 'getModelStats').mockResolvedValue([])
    vi.spyOn(endpointsApi, 'getModelTps').mockResolvedValue([])
    const getModels = vi
      .spyOn(endpointsApi, 'getModels')
      .mockResolvedValue({ endpoint_id: 'ep-beta', models: [] })
    renderTable([alpha, beta])

    await userEvent
      .setup()
      .click(within(rowFor('beta')).getByRole('button', { name: 'Details' }))

    const dialog = await screen.findByRole('dialog', { name: 'beta' })
    expect(within(dialog).getByText('http://beta.test:11434')).toBeInTheDocument()
    await waitFor(() => expect(getModels).toHaveBeenCalledWith('ep-beta'))
  })

  it('tests the connection of the chosen endpoint and refreshes the list', async () => {
    const result = deferred<Awaited<ReturnType<typeof endpointsApi.test>>>()
    const test = vi.spyOn(endpointsApi, 'test').mockReturnValue(result.promise)
    const { queryClient } = renderTable([alpha, beta])
    // The table owns no data: invalidating this query is how it asks the
    // dashboard to reload the endpoints.
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    const button = within(rowFor('beta')).getByRole('button', { name: 'Test Connection' })

    await userEvent.setup().click(button)

    expect(test).toHaveBeenCalledExactlyOnceWith('ep-beta')
    expect(button).toBeDisabled()
    expect(within(rowFor('alpha')).getByRole('button', { name: 'Test Connection' })).toBeEnabled()
    expect(invalidate).not.toHaveBeenCalled()

    result.resolve({ success: true, latency_ms: 12 })

    await waitFor(() => expect(button).toBeEnabled())
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['dashboard-endpoints'] })
  })

  it('syncs the models of an online endpoint and refreshes the list', async () => {
    const result = deferred<Awaited<ReturnType<typeof endpointsApi.sync>>>()
    const sync = vi.spyOn(endpointsApi, 'sync').mockReturnValue(result.promise)
    const { queryClient } = renderTable([alpha, beta])
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    const button = within(rowFor('alpha')).getByRole('button', { name: 'Sync Models' })

    await userEvent.setup().click(button)

    expect(sync).toHaveBeenCalledExactlyOnceWith('ep-alpha')
    expect(button).toBeDisabled()

    result.resolve({ synced_models: 3 })

    await waitFor(() => expect(button).toBeEnabled())
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['dashboard-endpoints'] })
  })

  it('does not offer a model sync for an endpoint that is not online', () => {
    renderTable([alpha, beta, gamma])

    expect(within(rowFor('alpha')).getByRole('button', { name: 'Sync Models' })).toBeEnabled()
    expect(within(rowFor('beta')).getByRole('button', { name: 'Sync Models' })).toBeDisabled()
    expect(within(rowFor('gamma')).getByRole('button', { name: 'Sync Models' })).toBeDisabled()
  })

  it('deletes the chosen endpoint after confirmation and refreshes the list', async () => {
    const user = userEvent.setup()
    const remove = vi.spyOn(endpointsApi, 'delete').mockResolvedValue(undefined)
    const { queryClient } = renderTable([alpha, beta])
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')

    await user.click(within(rowFor('beta')).getByRole('button', { name: 'Delete' }))

    const dialog = await screen.findByRole('alertdialog', { name: 'Delete Endpoint?' })
    expect(within(dialog).getByText(/This will delete "beta"\./)).toBeInTheDocument()
    expect(remove).not.toHaveBeenCalled()

    await user.click(within(dialog).getByRole('button', { name: 'Delete' }))

    expect(remove).toHaveBeenCalledExactlyOnceWith('ep-beta')
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
    await waitFor(() =>
      expect(invalidate).toHaveBeenCalledWith({ queryKey: ['dashboard-endpoints'] }),
    )
  })

  it('keeps the endpoint when the deletion is cancelled', async () => {
    const user = userEvent.setup()
    const remove = vi.spyOn(endpointsApi, 'delete')
    renderTable([alpha, beta])

    await user.click(within(rowFor('beta')).getByRole('button', { name: 'Delete' }))
    const dialog = await screen.findByRole('alertdialog', { name: 'Delete Endpoint?' })
    await user.click(within(dialog).getByRole('button', { name: 'Cancel' }))

    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
    expect(remove).not.toHaveBeenCalled()
    expect(rowFor('beta')).toBeInTheDocument()
  })

  it('creates an endpoint from the dialog and refreshes the list', async () => {
    const user = userEvent.setup()
    const create = vi
      .spyOn(endpointsApi, 'create')
      .mockResolvedValue(endpoint({ id: 'ep-edge', name: 'Edge Ollama' }))
    const { queryClient } = renderTable([alpha])
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')

    await user.click(screen.getByRole('button', { name: 'Add Endpoint' }))
    const dialog = await screen.findByRole('dialog', { name: 'Add New Endpoint' })
    const submit = within(dialog).getByRole('button', { name: 'Create Endpoint' })
    expect(submit).toBeDisabled()

    await fill(user, 'Name *', 'Edge Ollama')
    expect(submit).toBeDisabled()
    await fill(user, 'Base URL *', 'http://edge.test:11434')
    await fill(user, 'API Key (optional)', 'sk-edge')
    await fill(user, 'Notes (optional)', 'rack 4')
    await user.click(submit)

    expect(create).toHaveBeenCalledExactlyOnceWith({
      name: 'Edge Ollama',
      base_url: 'http://edge.test:11434',
      api_key: 'sk-edge',
      notes: 'rack 4',
    })
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['dashboard-endpoints'] })
  })

  it('shows the server error and keeps the dialog open when creation fails', async () => {
    const user = userEvent.setup()
    // The component logs the failure before showing it.
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(endpointsApi, 'create').mockRejectedValue(new Error('Endpoint URL already exists'))
    renderTable([alpha])

    await user.click(screen.getByRole('button', { name: 'Add Endpoint' }))
    const dialog = await screen.findByRole('dialog', { name: 'Add New Endpoint' })
    await fill(user, 'Name *', 'Duplicate')
    await fill(user, 'Base URL *', 'http://alpha.test:8080')
    await user.click(within(dialog).getByRole('button', { name: 'Create Endpoint' }))

    expect(await within(dialog).findByText('Endpoint URL already exists')).toBeInTheDocument()
    expect(within(dialog).getByLabelText('Name *')).toHaveValue('Duplicate')
    expect(within(dialog).getByRole('button', { name: 'Create Endpoint' })).toBeEnabled()
  })
})
