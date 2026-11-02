import { registrationRequestsApi } from '@/lib/api'
import { readFileSync } from 'node:fs'
import { act, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  dashboardApi,
  apiKeysApi,
  modelsApi,
  systemApi,
  type DashboardEndpoint,
  type DashboardOverview,
  type RequestResponsesPage,
  type SystemInfo,
} from '@/lib/api'
import { FakeWebSocket } from '@/test/fake-websocket'
import { deferred, renderPage, viewerUser } from '@/test/render'
import Dashboard from './Dashboard'

const endpoint: DashboardEndpoint = {
  id: 'ep-1',
  name: 'gpu-box-1',
  base_url: 'http://192.0.2.20:8080',
  status: 'online',
  endpoint_type: 'xllm',
  health_check_interval_secs: 30,
  inference_timeout_secs: 600,
  error_count: 0,
  registered_at: '2026-09-30T00:00:00Z',
  model_count: 2,
  total_requests: 10,
  successful_requests: 9,
  failed_requests: 1,
}

const overview: DashboardOverview = {
  endpoints: [endpoint],
  operations: {
    health: 'healthy',
    total_endpoints: 1,
    online_endpoints: 1,
    pending_endpoints: 0,
    registering_endpoints: 0,
    offline_endpoints: 0,
    error_endpoints: 0,
    total_requests: 10,
    successful_requests: 9,
    failed_requests: 1,
    success_rate: 0.9,
    active_requests: 0,
    queued_requests: 0,
    average_response_time_ms: 120,
    output_tps: 42.5,
    total_input_tokens: 1000,
    total_output_tokens: 2000,
    total_tokens: 3000,
    last_registered_at: null,
    last_seen_at: null,
  },
  capacity: {
    total_models: 2,
    gpu_capable_endpoints: 1,
    gpu_telemetry_endpoints: 0,
    total_gpu_memory_bytes: null,
    used_gpu_memory_bytes: null,
    gpu_memory_usage_percent: null,
    telemetry_status: 'unavailable',
  },
  action_items: [],
  history: [],
  endpoint_tps: [],
  generated_at: '2026-10-01T00:00:00Z',
  generation_time_ms: 3,
}

const systemInfo: SystemInfo = {
  version: '6.2.0',
  pid: 4242,
  in_flight: 0,
  update: { state: 'up_to_date' },
}

const requestResponses: RequestResponsesPage = {
  records: [
    {
      id: 'req-1',
      timestamp: '2026-10-01T00:00:00Z',
      request_type: 'chat',
      model: 'history-only-model',
      endpoint_id: 'ep-1',
      endpoint_name: 'gpu-box-1',
      duration_ms: 250,
      status: { type: 'success' },
    },
  ],
  total_count: 1,
  page: 1,
  per_page: 100,
}

function stubDashboardApis() {
  return {
    getDailyTokenStats: vi.spyOn(dashboardApi, 'getDailyTokenStats').mockResolvedValue([]),
    getMonthlyTokenStats: vi.spyOn(dashboardApi, 'getMonthlyTokenStats').mockResolvedValue([]),
    getActivity: vi.spyOn(dashboardApi, 'getActivity').mockResolvedValue({
      timestamp: '2026-10-01T00:00:00Z', bucket_seconds: 1, window_seconds: 60,
      total: { active_requests: 0, queued_requests: 0, requests_per_second: Array(60).fill(0) },
      endpoints: [],
    }),
    getOverview: vi.spyOn(dashboardApi, 'getOverview').mockResolvedValue(overview),
    getEndpoints: vi.spyOn(dashboardApi, 'getEndpoints').mockResolvedValue([endpoint]),
    getRequestResponses: vi
      .spyOn(dashboardApi, 'getRequestResponses')
      .mockResolvedValue(requestResponses),
    getSystem: vi.spyOn(systemApi, 'getSystem').mockResolvedValue(systemInfo),
    getVersion: vi.spyOn(systemApi, 'getVersion').mockResolvedValue({ version: '6.2.0' }),
    getRegistered: vi.spyOn(modelsApi, 'getRegistered').mockResolvedValue([]),
  }
}

describe('Dashboard', () => {
  let api: ReturnType<typeof stubDashboardApis>

  beforeEach(() => {
  vi.spyOn(registrationRequestsApi, 'list').mockResolvedValue([])
    api = stubDashboardApis()
    // Freeze polling without freezing React Query's notifications or DOM waits.
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] })
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('keeps data, state and commands in the dashboard ViewModel', () => {
    const source = readFileSync('src/pages/Dashboard.tsx', 'utf8')

    expect(source).toMatch(/\buseDashboardViewModel\s*\(/)
    const hooks = [...source.matchAll(/\b(use[A-Z]\w*)\s*(?:<[^>]*>)?\s*\(/g)]
      .map((match) => match[1])
    expect(hooks).toEqual(['useDashboardViewModel'])
    expect(source).not.toMatch(/\b\w+Api\s*\.\s*\w+\s*\(/)
    expect(source).not.toMatch(/\bSYSTEM_INFO_QUERY_KEY\b|\binvalidateQueries\s*\(/)
  })

  it('switches from the loading state to the operations overview', async () => {
    const response = deferred<DashboardOverview>()
    api.getOverview.mockReturnValue(response.promise)
    renderPage(<Dashboard />)

    const operations = await screen.findByRole('region', { name: 'Operations overview' })
    expect(within(operations).queryByText('Healthy')).not.toBeInTheDocument()

    response.resolve(overview)

    expect(await within(operations).findByText('Healthy')).toBeInTheDocument()
    expect(within(operations).getByText('42.5 tok/s')).toBeInTheDocument()
  })

  it('lists the registered endpoints on the default tab', async () => {
    renderPage(<Dashboard />)

    expect(await screen.findByText('gpu-box-1')).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: 'Endpoints' })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual([
      'Endpoints',
      'Models',
      'Usage',
      'Requests',
      'Traffic',
      'System',
    ])
  })

  it('offers organization administration to the global administrator', async () => {
    renderPage(<Dashboard />)
    expect(await screen.findByRole('button', { name: 'Organizations' })).toBeVisible()
  })

  it('lets an organization administrator read usage and requests without system controls', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({ rows: [], totals: {
      request_count: 0, input_tokens: 0, output_tokens: 0, total_tokens: 0, measured_requests: 0,
      estimated_requests: 0, missing_requests: 0 }, total_count: 0, page: 1, per_page: 10 }))))
    renderPage(<Dashboard />, { user: { ...viewerUser, organization_id: 'org-a', organization_role: 'admin', scope: 'organization' } })
    expect(await screen.findByRole('tab', { name: 'Usage' })).toBeVisible()
    expect(screen.getByRole('tab', { name: 'Requests' })).toBeVisible()
    expect(screen.queryByRole('tab', { name: 'System' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Settings' })).not.toBeInTheDocument()
    await userEvent.setup().click(screen.getByRole('tab', { name: 'Requests' }))
    expect(await screen.findByText('history-only-model')).toBeVisible()
  })

  it('creates an organization through the management dialog', async () => {
    const calls: { url: string; method: string; body?: string }[] = []
    let organizations: unknown[] = []
    vi.stubGlobal('fetch', vi.fn(async (url: string, options?: RequestInit) => {
      calls.push({ url, method: options?.method ?? 'GET', body: options?.body as string })
      if (url === '/api/users' || url.endsWith('/members')) return new Response(JSON.stringify({ users: [] }))
      if (options?.method === 'POST') {
        const name = JSON.parse(options.body as string).name
        organizations = [{ id: 'org-a', name, archived_at: null }]
        return new Response(JSON.stringify(organizations[0]), { status: 201 })
      }
      return new Response(JSON.stringify({ organizations }))
    }))
    renderPage(<Dashboard />)
    await userEvent.setup().click(await screen.findByRole('button', { name: 'Organizations' }))
    const dialog = await screen.findByRole('dialog', { name: 'Organizations' })
    await userEvent.setup().type(within(dialog).getByLabelText('Organization name'), 'Research team')
    await userEvent.setup().click(within(dialog).getByRole('button', { name: 'Create organization' }))
    expect(await within(dialog).findByRole('heading', { name: 'Research team' })).toBeVisible()
    expect(calls).toContainEqual({ url: '/api/organizations', method: 'POST', body: JSON.stringify({ name: 'Research team' }) })
  })

  it('shows scoped durable usage with missing-token coverage and an inclusive UTC period', async () => {
    const requests: string[] = []
    const counts = { request_count: 12, input_tokens: 110, output_tokens: 220, total_tokens: 330,
      measured_requests: 8, estimated_requests: 1, missing_requests: 3 }
    vi.stubGlobal('fetch', vi.fn(async (url: string) => {
      requests.push(url)
      return new Response(JSON.stringify({ scope: 'global', group_by: 'organization',
        from: '2026-10-01', to: '2026-10-31', to_exclusive: '2026-11-01',
        rows: [{ ...counts, organization_id: 'org-a', account_id: null, organization_name: 'Research team',
          account_name: null, organization_archived: false, account_deleted: false }],
        totals: counts, page: 1, per_page: 10, total_count: 1, coverage_start: '2026-10-01T00:00:00Z',
      }))
    }))
    renderPage(<Dashboard />)
    await userEvent.setup().click(await screen.findByRole('tab', { name: 'Usage' }))
    const usage = await screen.findByRole('region', { name: 'Organization and account usage' })
    expect(await within(usage).findByText('Research team')).toBeVisible()
    expect(within(usage).getByText('3 missing')).toBeVisible()
    expect(within(usage).getByText('Quota: not configured')).toBeVisible()
    const request = requests.find((url) => url.startsWith('/api/dashboard/usage?'))!
    expect(new URL(request, 'http://localhost').searchParams.get('group_by')).toBe('organization')
    expect(within(usage).getByLabelText('From (UTC)')).toHaveAttribute('type', 'date')
    expect(within(usage).getByLabelText('Through (UTC, inclusive)')).toHaveAttribute('type', 'date')
  })

  it('issues an inference key with an explicit account and organization, keeping plaintext out of cache', async () => {
    vi.stubGlobal('fetch', vi.fn(async (url: string) => new Response(JSON.stringify(url.endsWith('/members')
      ? { users: [{ id: 'account-a', username: 'alice', role: 'viewer', organization_id: 'org-a', organization_role: 'member' }] }
      : url === '/api/users' ? { users: [] } : { organizations: [{ id: 'org-a', name: 'Research team', archived_at: null }] }))))
    const create = vi.spyOn(apiKeysApi, 'create').mockResolvedValue({ id: 'key-a', name: 'Research access',
      key: 'sk_secret_at_creation', key_prefix: 'sk_secret', created_at: '2026-10-30T00:00:00Z', permissions: ['openai.inference'] })
    const { queryClient } = renderPage(<Dashboard />)
    await userEvent.setup().click(await screen.findByRole('button', { name: 'Organizations' }))
    const dialog = await screen.findByRole('dialog', { name: 'Organizations' })
    await userEvent.setup().click(await within(dialog).findByRole('button', { name: 'Issue inference key for alice' }))
    await userEvent.setup().type(within(dialog).getByLabelText('Key name'), 'Research access')
    await userEvent.setup().click(within(dialog).getByRole('button', { name: 'Issue key' }))
    expect(await within(dialog).findByText('sk_secret_at_creation')).toBeVisible()
    expect(create).toHaveBeenCalledExactlyOnceWith({ name: 'Research access', usage_account_id: 'account-a', organization_id: 'org-a', permissions: ['openai.inference', 'openai.models.read'] })
    expect(JSON.stringify(queryClient.getQueryCache().getAll().map((q) => q.state.data))).not.toContain('sk_secret_at_creation')
  })

  it('labels a transferred account without treating it as deleted or inventing its current name', async () => {
    const counts = { request_count: 1, input_tokens: 11, output_tokens: 7, total_tokens: 18,
      measured_requests: 1, estimated_requests: 0, missing_requests: 0 }
    vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({ scope: 'organization', group_by: 'account',
      rows: [{ ...counts, organization_id: 'org-a', account_id: 'account-a', organization_name: 'Research team',
        account_name: null, account_deleted: false, organization_archived: false }],
      totals: counts, page: 1, per_page: 10, total_count: 1, coverage_start: null }))))
    renderPage(<Dashboard />, { user: { ...viewerUser, organization_id: 'org-a', organization_role: 'admin', scope: 'organization' } })
    expect(await screen.findByRole('button', { name: 'Former account account-' })).toBeVisible()
    expect(screen.queryByText('Deleted account')).not.toBeInTheDocument()
  })

  it('shows the request history when the Requests tab is selected', async () => {
    renderPage(<Dashboard />)
    await screen.findByText('gpu-box-1')
    expect(screen.queryByText('history-only-model')).not.toBeInTheDocument()

    await userEvent.setup().click(screen.getByRole('tab', { name: 'Requests' }))

    expect(await screen.findByText('history-only-model')).toBeInTheDocument()
    expect(api.getRequestResponses).toHaveBeenCalledWith({ limit: 100 })
  })

  it('shows the error state and reloads on Try again', async () => {
    api.getOverview.mockRejectedValueOnce(new Error('overview unavailable'))
    renderPage(<Dashboard />)

    expect(await screen.findByRole('heading', { name: 'Failed to load dashboard' })).toBeInTheDocument()
    expect(screen.getByText('overview unavailable')).toBeInTheDocument()
    expect(screen.queryByRole('tab')).not.toBeInTheDocument()

    await userEvent.setup().click(screen.getByRole('button', { name: 'Try again' }))

    // The polling clock is frozen: retry must issue the request immediately.
    expect(api.getOverview).toHaveBeenCalledTimes(2)
    expect(await screen.findByText('gpu-box-1')).toBeInTheDocument()
  })

  it('polls the overview every five seconds while the live connection is closed', async () => {
    renderPage(<Dashboard />)
    await screen.findByText('gpu-box-1')
    expect(api.getOverview).toHaveBeenCalledTimes(1)

    await act(() => vi.advanceTimersByTimeAsync(4999))
    expect(api.getOverview).toHaveBeenCalledTimes(1)

    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(api.getOverview).toHaveBeenCalledTimes(2)
  })

  it('polls the overview every ten seconds while the live connection is open', async () => {
    renderPage(<Dashboard />)
    await screen.findByText('gpu-box-1')
    act(() => FakeWebSocket.latest().onopen?.())
    expect(api.getOverview).toHaveBeenCalledTimes(1)

    await act(() => vi.advanceTimersByTimeAsync(9999))
    expect(api.getOverview).toHaveBeenCalledTimes(1)

    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(api.getOverview).toHaveBeenCalledTimes(2)
  })

  it('returns to five-second overview polling when the live connection closes', async () => {
    renderPage(<Dashboard />)
    await screen.findByText('gpu-box-1')
    act(() => FakeWebSocket.latest().onopen?.())
    act(() => FakeWebSocket.latest().onclose?.())
    expect(api.getOverview).toHaveBeenCalledTimes(1)

    await act(() => vi.advanceTimersByTimeAsync(4999))
    expect(api.getOverview).toHaveBeenCalledTimes(1)

    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(api.getOverview).toHaveBeenCalledTimes(2)
  })

  it('subscribes admins to live dashboard events', async () => {
    renderPage(<Dashboard />)
    await screen.findByText('gpu-box-1')

    await waitFor(() =>
      expect(FakeWebSocket.latest().url).toBe(`ws://${window.location.host}/ws/dashboard`),
    )
  })

  it('keeps the WebSocket pill live when only the activity fetch is delayed', async () => {
    renderPage(<Dashboard />)
    await screen.findByText('gpu-box-1')
    act(() => FakeWebSocket.latest().onopen?.())
    expect(screen.getByTestId('activity-connection-status')).toHaveTextContent('Live · WebSocket')
    api.getActivity.mockRejectedValue(new Error('activity unavailable'))
    act(() => FakeWebSocket.latest().receive(JSON.stringify({ changed: 'metrics' })))
    await waitFor(() => expect(screen.getByTestId('activity-connection-status')).toHaveTextContent('Update delayed'))
    expect(screen.getByTestId('activity-connection-status')).toHaveTextContent('Live · WebSocket')
    expect(screen.getByTestId('activity-connection-status')).not.toHaveTextContent('WebSocket 未接続')
    expect(screen.getByRole('region', { name: 'Request activity' })).toHaveTextContent('Last known activity')
  })

  // Keep the real ViewModels and query hooks: these counts tie their declared
  // subscriptions to the queries the page runs. With polling frozen, only the
  // event can trigger these refetches.
  it('reloads the overview and the request history when an endpoint event arrives', async () => {
    renderPage(<Dashboard />)
    await screen.findByText('gpu-box-1')
    await waitFor(() => expect(api.getRequestResponses).toHaveBeenCalledTimes(1))
    expect(api.getOverview).toHaveBeenCalledTimes(1)

    act(() => {
      FakeWebSocket.latest().receive(
        JSON.stringify({ changed: 'endpoints', id: 'ep-2' }),
      )
    })

    expect(api.getOverview).toHaveBeenCalledTimes(2)
    expect(api.getRequestResponses).toHaveBeenCalledTimes(2)
    expect(api.getSystem).toHaveBeenCalledTimes(1)
  })

  it('reloads the system info when the update state changes', async () => {
    renderPage(<Dashboard />)
    await screen.findByText('gpu-box-1')
    await waitFor(() => expect(api.getSystem).toHaveBeenCalledTimes(1))

    act(() => {
      FakeWebSocket.latest().receive(JSON.stringify({ changed: 'system' }))
    })

    expect(api.getSystem).toHaveBeenCalledTimes(2)
    expect(api.getOverview).toHaveBeenCalledTimes(1)
  })

  it('shows unassigned viewers membership guidance without requesting private data', async () => {
    renderPage(<Dashboard />, { user: viewerUser })

    expect(await screen.findByText(/Organization membership required/)).toBeInTheDocument()
    expect(screen.queryByRole('tab')).not.toBeInTheDocument()
    expect(api.getRegistered).not.toHaveBeenCalled()
    expect(api.getEndpoints).not.toHaveBeenCalled()
    expect(api.getRequestResponses).not.toHaveBeenCalled()
    expect(api.getSystem).not.toHaveBeenCalled()
    expect(api.getActivity).not.toHaveBeenCalled()
    expect(api.getOverview).not.toHaveBeenCalled()
    expect(FakeWebSocket.instances).toHaveLength(0)
  })

  it('disables unassigned key and inference actions on desktop and mobile without fetching keys', async () => {
    const keys = vi.spyOn(apiKeysApi, 'list').mockResolvedValue([])
    const user = userEvent.setup()
    renderPage(<Dashboard />, { user: viewerUser })
    await screen.findByText(/Organization membership required/)
    for (const name of ['API Keys', 'LB Playground']) {
      const action = screen.getByRole('button', { name })
      expect(action).toBeDisabled()
      await user.click(action)
    }
    await user.click(screen.getByRole('button', { name: 'User menu' }))
    const menu = screen.getByRole('menu')
    for (const name of ['API Keys', 'LB Playground']) {
      expect(within(menu).getByRole('menuitem', { name })).toHaveAttribute('aria-disabled', 'true')
    }
    expect(screen.queryByRole('dialog', { name: 'API Keys' })).not.toBeInTheDocument()
    expect(keys).not.toHaveBeenCalled()
  })

  it('keeps global key viewing available and explains Organizations issuance without self membership', async () => {
    const keys = vi.spyOn(apiKeysApi, 'list').mockResolvedValue([])
    const user = userEvent.setup()
    renderPage(<Dashboard />)
    await screen.findByRole('button', { name: 'API Keys' })
    await user.click(screen.getByRole('button', { name: 'API Keys' }))
    const modal = await screen.findByRole('dialog', { name: 'API Keys' })
    await waitFor(() => expect(keys).toHaveBeenCalledOnce())
    expect(within(modal).getByRole('button', { name: 'Create Key' })).toBeDisabled()
    expect(within(modal).getByText(/Organizations.*issue|issue.*Organizations/)).toBeInTheDocument()
  })
})
