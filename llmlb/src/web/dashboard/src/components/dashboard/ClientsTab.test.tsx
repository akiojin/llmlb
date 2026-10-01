import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import {
  clientsApi,
  type ClientApiKeyUsage,
  type ClientDetailResponse,
  type ClientIpRanking,
  type ClientRankingResponse,
} from '@/lib/api'
import { deferred, renderWithProviders } from '@/test/render'
import { ClientsTab } from './ClientsTab'

function ranking(overrides: Partial<ClientIpRanking> = {}): ClientIpRanking {
  return {
    ip: '192.0.2.10',
    request_count: 420,
    last_seen: '2026-10-01T00:00:00Z',
    is_alert: false,
    api_key_count: 1,
    ...overrides,
  }
}

function rankingPage(
  rankings: ClientIpRanking[],
  { total = rankings.length, page = 1 } = {},
): ClientRankingResponse {
  return { rankings, total_count: total, page, per_page: 20 }
}

function detail(overrides: Partial<ClientDetailResponse> = {}): ClientDetailResponse {
  return {
    total_requests: 77,
    first_seen: '2026-09-30T00:00:00Z',
    last_seen: null,
    recent_requests: [
      {
        id: 'req-1',
        timestamp: '2026-10-01T00:00:00Z',
        model: 'llama-3.1-8b',
        status: 'success',
        duration_ms: 35,
      },
      {
        id: 'req-2',
        timestamp: '2026-10-01T00:00:05Z',
        model: 'qwen2.5-coder',
        status: 'error',
        duration_ms: null,
      },
    ],
    model_distribution: [{ model: 'llama-3.1-8b', request_count: 77, percentage: 100 }],
    hourly_pattern: [{ hour: 9, count: 77 }],
    ...overrides,
  }
}

/** Stubs every request the tab makes on mount with an empty response. */
function stubClientsApi() {
  return {
    getClientRanking: vi.spyOn(clientsApi, 'getClientRanking').mockResolvedValue(rankingPage([])),
    getTimeline: vi.spyOn(clientsApi, 'getTimeline').mockResolvedValue([]),
    getModels: vi.spyOn(clientsApi, 'getModels').mockResolvedValue([]),
    getHeatmap: vi.spyOn(clientsApi, 'getHeatmap').mockResolvedValue([]),
    getAlertThreshold: vi
      .spyOn(clientsApi, 'getAlertThreshold')
      .mockResolvedValue({ key: 'ip_alert_threshold', value: '250' }),
  }
}

function stubDrilldown(details: Record<string, ClientDetailResponse>, apiKeys: ClientApiKeyUsage[]) {
  return {
    getClientDetail: vi
      .spyOn(clientsApi, 'getClientDetail')
      .mockImplementation((ip) => Promise.resolve(details[ip])),
    getClientApiKeys: vi.spyOn(clientsApi, 'getClientApiKeys').mockResolvedValue(apiKeys),
  }
}

/**
 * The tab reads its filter from the query string and clears it with a full
 * page navigation, which jsdom does not implement.
 */
function stubLocationSearch(search: string) {
  const location = { search }
  vi.stubGlobal('location', location)
  return location
}

function clientRow(ip: string) {
  return screen.getByRole('row', { name: new RegExp(ip.replace(/\./g, '\\.')) })
}

// The drilldown nests tables inside a ranking row, so a row looked up by its
// accessible name would also match the row that wraps the whole drilldown.
function rowContaining(text: string) {
  return within(screen.getByText(text).closest('tr')!)
}

// The pagination buttons are icon-only and have no accessible name.
function pagination(position: string) {
  const [previous, next] = within(screen.getByText(position).parentElement!).getAllByRole('button')
  return { previous, next }
}

describe('ClientsTab', () => {
  it('switches from the loading state to the client ranking', async () => {
    const api = stubClientsApi()
    const response = deferred<ClientRankingResponse>()
    api.getClientRanking.mockReturnValue(response.promise)
    renderWithProviders(<ClientsTab />)

    expect(await screen.findByText('Loading client data...')).toBeInTheDocument()
    expect(api.getClientRanking).toHaveBeenCalledWith({ page: 1, per_page: 20, ip: undefined })
    expect(screen.queryByRole('table')).not.toBeInTheDocument()

    response.resolve(
      rankingPage([
        ranking(),
        ranking({ ip: '198.51.100.7', request_count: 36, is_alert: true }),
      ]),
    )

    const quiet = await screen.findByRole('row', { name: /192\.0\.2\.10/ })
    expect(screen.queryByText('Loading client data...')).not.toBeInTheDocument()
    expect(within(quiet).getByText('420')).toBeInTheDocument()
    expect(within(quiet).queryByText('Alert')).not.toBeInTheDocument()
    const noisy = clientRow('198.51.100.7')
    expect(within(noisy).getByText('36')).toBeInTheDocument()
    expect(within(noisy).getByText('Alert')).toBeInTheDocument()
    expect(screen.queryByText('No client data available')).not.toBeInTheDocument()
    expect(screen.queryByText(/clients total/)).not.toBeInTheDocument()
    expect(screen.queryByText(/^Filtered:/)).not.toBeInTheDocument()
  })

  it('shows the empty states when no client has sent a request', async () => {
    stubClientsApi()
    renderWithProviders(<ClientsTab />)

    const table = await screen.findByRole('table')
    expect(within(table).getByText('No client data available')).toBeInTheDocument()
    expect(screen.getByText('No timeline data available')).toBeInTheDocument()
    expect(screen.getByText('No model data available')).toBeInTheDocument()
    expect(screen.getByTitle('Mon 09:00 - 0 requests')).toBeInTheDocument()
  })

  it('renders the traffic breakdown', async () => {
    const api = stubClientsApi()
    api.getClientRanking.mockResolvedValue(rankingPage([ranking()]))
    api.getTimeline.mockResolvedValue([{ hour: '2026-10-01T00:00:00Z', unique_ips: 4 }])
    api.getModels.mockResolvedValue([
      { model: 'llama-3.1-8b', request_count: 420, percentage: 100 },
    ])
    api.getHeatmap.mockResolvedValue([
      { day_of_week: 1, hour: 9, count: 5 },
      { day_of_week: 0, hour: 23, count: 2 },
    ])
    renderWithProviders(<ClientsTab />)

    expect(await screen.findByTitle('Mon 09:00 - 5 requests')).toBeInTheDocument()
    expect(screen.getByTitle('Sun 23:00 - 2 requests')).toBeInTheDocument()
    expect(screen.getByTitle('Tue 09:00 - 0 requests')).toBeInTheDocument()
    expect(api.getHeatmap).toHaveBeenCalledWith({ ip: undefined })
    await waitFor(() =>
      expect(screen.queryByText('No timeline data available')).not.toBeInTheDocument(),
    )
    await waitFor(() =>
      expect(screen.queryByText('No model data available')).not.toBeInTheDocument(),
    )
  })

  it('requests the next page of the ranking', async () => {
    const api = stubClientsApi()
    api.getClientRanking.mockImplementation((params) =>
      Promise.resolve(
        params?.page === 2
          ? rankingPage([ranking({ ip: '203.0.113.5' })], { total: 45, page: 2 })
          : rankingPage([ranking()], { total: 45 }),
      ),
    )
    renderWithProviders(<ClientsTab />)

    expect(await screen.findByText('45 clients total')).toBeInTheDocument()
    expect(pagination('1 / 3').previous).toBeDisabled()
    await userEvent.setup().click(pagination('1 / 3').next)

    expect(await screen.findByText('2 / 3')).toBeInTheDocument()
    expect(api.getClientRanking).toHaveBeenLastCalledWith({ page: 2, per_page: 20, ip: undefined })
    expect(clientRow('203.0.113.5')).toBeInTheDocument()
    expect(screen.queryByText('192.0.2.10')).not.toBeInTheDocument()
    expect(pagination('2 / 3').previous).toBeEnabled()
    expect(pagination('2 / 3').next).toBeEnabled()
  })

  it('opens the drilldown for a client and closes it again', async () => {
    const api = stubClientsApi()
    api.getClientRanking.mockResolvedValue(rankingPage([ranking()]))
    const drilldown = stubDrilldown({ '192.0.2.10': detail() }, [
      { api_key_id: 'key-1', name: 'ci-key', request_count: 70 },
      { api_key_id: 'key-2', name: null, request_count: 7 },
    ])
    renderWithProviders(<ClientsTab />)
    const user = userEvent.setup()

    await user.click(await screen.findByRole('row', { name: /192\.0\.2\.10/ }))

    expect(await screen.findByText('77 requests')).toBeInTheDocument()
    expect(drilldown.getClientDetail).toHaveBeenCalledExactlyOnceWith('192.0.2.10')
    expect(drilldown.getClientApiKeys).toHaveBeenCalledExactlyOnceWith('192.0.2.10')
    expect(screen.getByText('First:')).toBeInTheDocument()
    expect(screen.queryByText('Last:')).not.toBeInTheDocument()
    expect(rowContaining('llama-3.1-8b').getByText('35ms')).toBeInTheDocument()
    expect(rowContaining('qwen2.5-coder').getByText('-')).toBeInTheDocument()
    expect(screen.queryByText('No hourly data')).not.toBeInTheDocument()
    await screen.findByText('ci-key')
    expect(rowContaining('ci-key').getByText('70')).toBeInTheDocument()
    expect(rowContaining('Deleted').getByText('7')).toBeInTheDocument()

    await user.click(clientRow('192.0.2.10'))

    expect(screen.queryByText('77 requests')).not.toBeInTheDocument()
    expect(screen.queryByText('Recent Requests')).not.toBeInTheDocument()
  })

  it('switches the drilldown to another client', async () => {
    const api = stubClientsApi()
    api.getClientRanking.mockResolvedValue(
      rankingPage([ranking(), ranking({ ip: '198.51.100.7', request_count: 36 })]),
    )
    const drilldown = stubDrilldown(
      {
        '192.0.2.10': detail(),
        '198.51.100.7': detail({ total_requests: 36, hourly_pattern: [{ hour: 9, count: 0 }] }),
      },
      [],
    )
    renderWithProviders(<ClientsTab />)
    const user = userEvent.setup()

    await user.click(await screen.findByRole('row', { name: /192\.0\.2\.10/ }))
    await screen.findByText('77 requests')
    await user.click(clientRow('198.51.100.7'))

    expect(await screen.findByText('36 requests')).toBeInTheDocument()
    expect(drilldown.getClientDetail).toHaveBeenLastCalledWith('198.51.100.7')
    expect(screen.queryByText('77 requests')).not.toBeInTheDocument()
    expect(screen.getByText('No hourly data')).toBeInTheDocument()
    expect(await screen.findByText('No API key data')).toBeInTheDocument()
  })

  it('reports a client without recorded requests', async () => {
    const api = stubClientsApi()
    api.getClientRanking.mockResolvedValue(rankingPage([ranking()]))
    stubDrilldown({ '192.0.2.10': detail({ total_requests: 0, recent_requests: [] }) }, [])
    renderWithProviders(<ClientsTab />)

    await userEvent.setup().click(await screen.findByRole('row', { name: /192\.0\.2\.10/ }))

    expect(await screen.findByText('No data for this IP')).toBeInTheDocument()
    expect(screen.queryByText('Recent Requests')).not.toBeInTheDocument()
  })

  it('filters by the ip query parameter', async () => {
    stubLocationSearch('?tab=clients&ip=192.0.2.10')
    const api = stubClientsApi()
    api.getClientRanking.mockResolvedValue(rankingPage([ranking()]))
    renderWithProviders(<ClientsTab />)

    await screen.findByRole('row', { name: /192\.0\.2\.10/ })
    expect(api.getClientRanking).toHaveBeenCalledWith({ page: 1, per_page: 20, ip: '192.0.2.10' })
    expect(api.getHeatmap).toHaveBeenCalledWith({ ip: '192.0.2.10' })
    expect(screen.getAllByText('Filtered: 192.0.2.10')).toHaveLength(2)
    expect(screen.getAllByRole('button', { name: 'Clear IP filter' })).toHaveLength(2)
  })

  it('clears the ip filter and keeps the other query parameters', async () => {
    const location = stubLocationSearch('?tab=clients&ip=192.0.2.10')
    const api = stubClientsApi()
    api.getClientRanking.mockResolvedValue(rankingPage([ranking()]))
    renderWithProviders(<ClientsTab />)

    const [clear] = await screen.findAllByRole('button', { name: 'Clear IP filter' })
    await userEvent.setup().click(clear)

    expect(location.search).toBe('tab=clients')
  })

  it('saves a new alert threshold and refreshes the ranking', async () => {
    const api = stubClientsApi()
    api.getAlertThreshold
      .mockResolvedValueOnce({ key: 'ip_alert_threshold', value: '250' })
      .mockResolvedValue({ key: 'ip_alert_threshold', value: '500' })
    const update = vi
      .spyOn(clientsApi, 'updateAlertThreshold')
      .mockResolvedValue({ key: 'ip_alert_threshold', value: '500' })
    renderWithProviders(<ClientsTab />)
    const user = userEvent.setup()

    expect(await screen.findByText('250 requests')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Edit' }))
    const input = screen.getByRole('spinbutton')
    expect(input).toHaveValue(250)
    await user.clear(input)
    await user.type(input, '500')
    await user.click(screen.getByRole('button', { name: 'Save' }))

    expect(update).toHaveBeenCalledExactlyOnceWith('500')
    expect(await screen.findByText('500 requests')).toBeInTheDocument()
    expect(screen.queryByRole('spinbutton')).not.toBeInTheDocument()
    await waitFor(() => expect(api.getClientRanking).toHaveBeenCalledTimes(2))
  })

  it('does not save a threshold below one request', async () => {
    stubClientsApi()
    const update = vi.spyOn(clientsApi, 'updateAlertThreshold')
    renderWithProviders(<ClientsTab />)
    const user = userEvent.setup()

    await screen.findByText('250 requests')
    await user.click(screen.getByRole('button', { name: 'Edit' }))
    await user.clear(screen.getByRole('spinbutton'))
    await user.type(screen.getByRole('spinbutton'), '0')
    await user.click(screen.getByRole('button', { name: 'Save' }))

    expect(update).not.toHaveBeenCalled()
  })

  it('discards the edited threshold on cancel', async () => {
    stubClientsApi()
    const update = vi.spyOn(clientsApi, 'updateAlertThreshold')
    renderWithProviders(<ClientsTab />)
    const user = userEvent.setup()

    await screen.findByText('250 requests')
    await user.click(screen.getByRole('button', { name: 'Edit' }))
    await user.clear(screen.getByRole('spinbutton'))
    await user.type(screen.getByRole('spinbutton'), '900')
    await user.click(screen.getByRole('button', { name: 'Cancel' }))

    expect(screen.getByText('250 requests')).toBeInTheDocument()
    expect(screen.queryByRole('spinbutton')).not.toBeInTheDocument()
    expect(update).not.toHaveBeenCalled()
  })
})
