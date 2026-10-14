import { readFileSync } from 'node:fs'
import { act, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  dashboardApi,
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

  it('limits viewers to the model list', async () => {
    renderPage(<Dashboard />, { user: viewerUser })

    expect(await screen.findByText('Healthy')).toBeInTheDocument()
    expect(screen.queryByRole('tab')).not.toBeInTheDocument()
    expect(api.getRegistered).toHaveBeenCalledWith('canonical')
    expect(api.getEndpoints).not.toHaveBeenCalled()
    expect(api.getRequestResponses).not.toHaveBeenCalled()
    expect(api.getSystem).not.toHaveBeenCalled()
    expect(FakeWebSocket.instances).toHaveLength(0)
  })
})
