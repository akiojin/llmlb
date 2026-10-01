import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  dashboardApi,
  modelsApi,
  systemApi,
  type DashboardOverview,
  type SystemInfo,
  type UpdateState,
} from '@/lib/api'
import { renderPage, viewerUser, type TestUser } from '@/test/render'
import Dashboard from './Dashboard'

const overview: DashboardOverview = {
  endpoints: [],
  operations: {
    health: 'empty',
    total_endpoints: 0,
    online_endpoints: 0,
    pending_endpoints: 0,
    registering_endpoints: 0,
    offline_endpoints: 0,
    error_endpoints: 0,
    total_requests: 0,
    successful_requests: 0,
    failed_requests: 0,
    success_rate: null,
    active_requests: 0,
    queued_requests: 0,
    average_response_time_ms: null,
    output_tps: null,
    total_input_tokens: 0,
    total_output_tokens: 0,
    total_tokens: 0,
    last_registered_at: null,
    last_seen_at: null,
  },
  capacity: {
    total_models: 0,
    gpu_capable_endpoints: 0,
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
  generation_time_ms: 1,
}

const NOW = new Date('2026-10-01T00:00:00Z')

const upToDate: UpdateState = { state: 'up_to_date', checked_at: null }

function available(payload: Extract<UpdateState, { state: 'available' }>['payload']): UpdateState {
  return {
    state: 'available',
    current: '6.2.0',
    latest: '6.3.0',
    release_url: 'https://example.test/releases/v6.3.0',
    payload,
    checked_at: '2026-10-01T00:00:00Z',
  }
}

const draining: UpdateState = {
  state: 'draining',
  latest: '6.3.0',
  in_flight: 3,
  requested_at: '2026-10-01T00:00:00Z',
  timeout_at: '2026-10-01T00:01:30Z',
}

const applying: UpdateState = {
  state: 'applying',
  latest: '6.3.0',
  method: 'installer',
  phase: 'running_installer',
  phase_message: 'Running the installer',
  timeout_at: '2026-10-01T00:02:05Z',
}

function failed(latest: string | null): UpdateState {
  return {
    state: 'failed',
    latest,
    release_url: latest ? 'https://example.test/releases/v6.3.0' : null,
    message: 'checksum mismatch',
    failed_at: '2026-10-01T00:00:00Z',
  }
}

interface Scenario {
  /** `'unavailable'` makes `/api/system` fail, so no update state is known. */
  update?: UpdateState | 'unavailable'
  system?: Partial<SystemInfo>
  user?: TestUser
}

function systemInfo(update: UpdateState, system: Partial<SystemInfo> = {}): SystemInfo {
  return { version: '6.2.0', pid: 1, in_flight: 0, update, ...system }
}

async function renderDashboard({ update = upToDate, system, user }: Scenario = {}) {
  vi.spyOn(dashboardApi, 'getOverview').mockResolvedValue(overview)
  vi.spyOn(dashboardApi, 'getEndpoints').mockResolvedValue([])
  vi.spyOn(dashboardApi, 'getRequestResponses').mockResolvedValue({
    records: [],
    total_count: 0,
    page: 1,
    per_page: 100,
  })
  vi.spyOn(systemApi, 'getVersion').mockResolvedValue({ version: '6.2.0' })
  vi.spyOn(modelsApi, 'getRegistered').mockResolvedValue([])
  const getSystem = vi.spyOn(systemApi, 'getSystem')
  if (update === 'unavailable') {
    getSystem.mockRejectedValue(new Error('system unavailable'))
  } else {
    getSystem.mockResolvedValue(systemInfo(update, system))
  }

  renderPage(<Dashboard />, { user })
  await screen.findByText('No endpoints')
  return { getSystem, user: userEvent.setup() }
}

const button = (name: string) => screen.getByRole('button', { name })
const queryButton = (name: string) => screen.queryByRole('button', { name })

// The countdowns are derived from the wall clock; only Date is faked so that
// React Query and Testing Library keep their real timers.
function freezeClock() {
  vi.useFakeTimers({ toFake: ['Date'], now: NOW })
}

afterEach(() => {
  vi.useRealTimers()
})

describe('Dashboard update banner', () => {
  describe('status', () => {
    it('stays visible with the check button while the update status is unavailable', async () => {
      await renderDashboard({ update: 'unavailable' })

      expect(screen.getByText('Update status unavailable')).toBeInTheDocument()
      expect(button('Check for updates')).toBeEnabled()
      expect(queryButton('Restart to update')).not.toBeInTheDocument()
      expect(queryButton('Force update now')).not.toBeInTheDocument()
    })

    it('reports an up-to-date installation without update actions', async () => {
      await renderDashboard({ update: upToDate })

      expect(await screen.findByText('Up to date')).toBeInTheDocument()
      expect(screen.getByText('Last checked: unknown')).toBeInTheDocument()
      expect(queryButton('Restart to update')).not.toBeInTheDocument()
      expect(queryButton('Force update now')).not.toBeInTheDocument()
      expect(queryButton('Rollback to previous version')).not.toBeInTheDocument()
    })

    it('offers a restart for an available update whose payload is still preparing', async () => {
      await renderDashboard({ update: available({ payload: 'not_ready' }) })

      expect(await screen.findByText('Update available: v6.3.0')).toBeInTheDocument()
      expect(screen.getByText('Current: v6.2.0')).toBeInTheDocument()
      expect(screen.getByText('Preparing...')).toBeInTheDocument()
      expect(screen.getByRole('link', { name: 'Release' })).toHaveAttribute(
        'href',
        'https://example.test/releases/v6.3.0',
      )
      expect(button('Restart to update')).toBeEnabled()
      expect(button('Force update now')).toBeDisabled()
      expect(button('Force update now')).toHaveAttribute('title', 'Update payload is still preparing')
      // Header badge
      expect(screen.getByText('v6.3.0 available')).toBeInTheDocument()
    })

    it('shows the download progress in bytes and percent', async () => {
      await renderDashboard({
        update: available({
          payload: 'downloading',
          started_at: '2026-10-01T00:00:00Z',
          downloaded_bytes: 5 * 1024 * 1024,
          total_bytes: 10 * 1024 * 1024,
        }),
      })

      expect(await screen.findByText('Downloading: 5MB / 10MB (50%)')).toBeInTheDocument()
      expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '50')
    })

    it('shows an indeterminate download when the size is unknown', async () => {
      await renderDashboard({
        update: available({ payload: 'downloading', started_at: '2026-10-01T00:00:00Z' }),
      })

      expect(await screen.findByText('Downloading...')).toBeInTheDocument()
      expect(screen.queryByRole('progressbar')).not.toBeInTheDocument()
    })

    it('enables the force update once the payload is ready', async () => {
      await renderDashboard({ update: available({ payload: 'ready', kind: 'portable' }) })

      expect(await screen.findByText('Ready')).toBeInTheDocument()
      expect(button('Force update now')).toBeEnabled()
    })

    it('reports a failed payload download', async () => {
      await renderDashboard({ update: available({ payload: 'error', message: 'disk full' }) })

      expect(await screen.findByText('Download failed')).toBeInTheDocument()
      expect(button('Force update now')).toBeDisabled()
    })

    it('blocks update actions and counts down while draining', async () => {
      freezeClock()
      await renderDashboard({ update: draining })

      expect(await screen.findByText('Updating to v6.3.0')).toBeInTheDocument()
      expect(screen.getByText('Waiting for in-flight requests: 3')).toBeInTheDocument()
      expect(screen.getByText('Drain timeout in 1:30')).toBeInTheDocument()
      expect(button('Waiting to update... (3)')).toBeDisabled()
      expect(button('Check for updates')).toBeDisabled()
      expect(button('Check for updates')).toHaveAttribute('title', 'Update is in progress')
      expect(queryButton('Force update now')).not.toBeInTheDocument()
      // Header badge
      expect(screen.getByText('Updating...')).toBeInTheDocument()
    })

    it('shows the phase message and the timeout while applying', async () => {
      freezeClock()
      await renderDashboard({ update: applying })

      expect(await screen.findByText('Applying update: v6.3.0')).toBeInTheDocument()
      expect(screen.getByText('Running the installer')).toBeInTheDocument()
      expect(screen.getByText('Apply timeout in 2:05')).toBeInTheDocument()
      expect(button('Applying update...')).toBeDisabled()
      expect(button('Check for updates')).toBeDisabled()
    })

    it('lets a failed update with a known target be retried', async () => {
      await renderDashboard({ update: failed('6.3.0') })

      expect(await screen.findByText('checksum mismatch')).toBeInTheDocument()
      // Banner title and header badge
      expect(screen.getAllByText('Update failed')).toHaveLength(2)
      expect(button('Restart to update')).toBeEnabled()
      expect(screen.getByRole('link', { name: 'Release' })).toBeInTheDocument()
      expect(queryButton('Force update now')).not.toBeInTheDocument()
    })

    it('offers no restart for a failed update without a target version', async () => {
      await renderDashboard({ update: failed(null) })

      expect(await screen.findByText('checksum mismatch')).toBeInTheDocument()
      expect(queryButton('Restart to update')).not.toBeInTheDocument()
      expect(screen.queryByRole('link', { name: 'Release' })).not.toBeInTheDocument()
    })

    it('hides the banner and skips the system query for viewers', async () => {
      const { getSystem } = await renderDashboard({
        update: available({ payload: 'not_ready' }),
        user: viewerUser,
      })

      expect(queryButton('Check for updates')).not.toBeInTheDocument()
      expect(screen.queryByText('Update available: v6.3.0')).not.toBeInTheDocument()
      expect(getSystem).not.toHaveBeenCalled()
    })
  })

  describe('operations', () => {
    it('checks for updates, shows the result and throttles the next check', async () => {
      const found = available({ payload: 'not_ready' })
      const checkUpdate = vi.spyOn(systemApi, 'checkUpdate').mockResolvedValue({ update: found })
      const { getSystem, user } = await renderDashboard({ update: upToDate })
      await screen.findByText('Up to date')
      getSystem.mockResolvedValue(systemInfo(found))

      await user.click(button('Check for updates'))

      expect(checkUpdate).toHaveBeenCalledOnce()
      expect(await screen.findByText('Checked for updates')).toBeInTheDocument()
      expect(await screen.findByText('Update available: v6.3.0')).toBeInTheDocument()
      expect(button('Check for updates')).toBeDisabled()
      expect(button('Check for updates')).toHaveAttribute(
        'title',
        'Please wait before checking again',
      )
    })

    it('reports a failed update check', async () => {
      vi.spyOn(systemApi, 'checkUpdate').mockRejectedValue(new Error('rate limited'))
      const { user } = await renderDashboard({ update: upToDate })
      await screen.findByText('Up to date')

      await user.click(button('Check for updates'))

      expect(await screen.findByText('Update check failed')).toBeInTheDocument()
      expect(screen.getByText('rate limited')).toBeInTheDocument()
    })

    it.each([
      { queued: true, toast: 'Update queued' },
      { queued: false, toast: 'Applying update' },
    ])('applies the update (queued=$queued)', async ({ queued, toast }) => {
      const applyUpdate = vi
        .spyOn(systemApi, 'applyUpdate')
        .mockResolvedValue({ queued, mode: 'normal' })
      const { user } = await renderDashboard({ update: available({ payload: 'not_ready' }) })

      await user.click(await screen.findByRole('button', { name: 'Restart to update' }))

      expect(applyUpdate).toHaveBeenCalledOnce()
      expect(await screen.findByText(toast)).toBeInTheDocument()
    })

    it('reports a failed apply', async () => {
      vi.spyOn(systemApi, 'applyUpdate').mockRejectedValue(new Error('another update is running'))
      const { user } = await renderDashboard({ update: available({ payload: 'not_ready' }) })

      await user.click(await screen.findByRole('button', { name: 'Restart to update' }))

      expect(await screen.findByText('Failed to apply update')).toBeInTheDocument()
      expect(screen.getByText('another update is running')).toBeInTheDocument()
    })

    it('forces the update only after confirmation', async () => {
      const applyForceUpdate = vi
        .spyOn(systemApi, 'applyForceUpdate')
        .mockResolvedValue({ queued: false, mode: 'force', dropped_in_flight: 2 })
      const { user } = await renderDashboard({
        update: available({ payload: 'ready', kind: 'portable' }),
      })

      await user.click(await screen.findByRole('button', { name: 'Force update now' }))
      const dialog = await screen.findByRole('alertdialog', { name: 'Force update now?' })
      expect(applyForceUpdate).not.toHaveBeenCalled()

      await user.click(within(dialog).getByRole('button', { name: 'Force update' }))

      expect(applyForceUpdate).toHaveBeenCalledOnce()
      expect(await screen.findByText('Force update started')).toBeInTheDocument()
      expect(screen.getByText('2 in-flight request(s) were terminated.')).toBeInTheDocument()
      await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
    })

    it('does not force the update when the confirmation is cancelled', async () => {
      const applyForceUpdate = vi.spyOn(systemApi, 'applyForceUpdate')
      const { user } = await renderDashboard({
        update: available({ payload: 'ready', kind: 'portable' }),
      })

      await user.click(await screen.findByRole('button', { name: 'Force update now' }))
      const dialog = await screen.findByRole('alertdialog')
      await user.click(within(dialog).getByRole('button', { name: 'Cancel' }))

      await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
      expect(applyForceUpdate).not.toHaveBeenCalled()
    })

    it('rolls back to the previous version after confirmation', async () => {
      const rollback = vi.spyOn(systemApi, 'rollback').mockResolvedValue({ rolling_back: true })
      const { user } = await renderDashboard({
        update: upToDate,
        system: { rollback_available: true },
      })

      await user.click(await screen.findByRole('button', { name: 'Rollback to previous version' }))
      const dialog = await screen.findByRole('alertdialog', { name: 'Rollback to previous version?' })
      expect(rollback).not.toHaveBeenCalled()

      await user.click(within(dialog).getByRole('button', { name: 'Rollback' }))

      expect(rollback).toHaveBeenCalledOnce()
      expect(await screen.findByText('Rolling back')).toBeInTheDocument()
    })

    it('shows the pending schedule and cancels it', async () => {
      const cancelSchedule = vi.spyOn(systemApi, 'cancelSchedule').mockResolvedValue({ cancelled: true })
      const { user } = await renderDashboard({
        update: available({ payload: 'ready', kind: 'portable' }),
        system: {
          schedule: {
            mode: 'idle',
            scheduled_by: 'admin',
            target_version: '6.3.0',
            created_at: '2026-10-01T00:00:00Z',
          },
        },
      })

      const schedule = await screen.findByText(/Scheduled by admin \(idle\)/)
      await user.click(within(schedule).getByRole('button', { name: 'Cancel' }))

      expect(cancelSchedule).toHaveBeenCalledOnce()
      expect(await screen.findByText('Schedule cancelled')).toBeInTheDocument()
    })

    it('schedules the update for when the load balancer is idle', async () => {
      const createSchedule = vi.spyOn(systemApi, 'createSchedule').mockResolvedValue({
        schedule: {
          mode: 'idle',
          scheduled_by: 'admin',
          target_version: '6.3.0',
          created_at: '2026-10-01T00:00:00Z',
        },
      })
      const { user } = await renderDashboard({ update: available({ payload: 'ready', kind: 'portable' }) })

      await user.click(await screen.findByRole('button', { name: 'Settings' }))
      const dialog = await screen.findByRole('dialog', { name: 'Settings' })
      expect(
        within(dialog)
          .getAllByRole('tab')
          .map((tab) => tab.textContent),
      ).toEqual(['Auth', 'Schedule', 'History'])

      await user.click(within(dialog).getByRole('tab', { name: 'Schedule' }))
      await user.click(within(dialog).getByRole('radio', { name: 'When idle (in_flight = 0)' }))
      await user.click(within(dialog).getByRole('button', { name: 'Schedule' }))

      expect(createSchedule).toHaveBeenCalledExactlyOnceWith({ mode: 'idle', scheduled_at: undefined })
      expect(await screen.findByText('Schedule created')).toBeInTheDocument()
      await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    })

    it('requires a time before a scheduled update can be created', async () => {
      const { user } = await renderDashboard({ update: available({ payload: 'ready', kind: 'portable' }) })

      await user.click(await screen.findByRole('button', { name: 'Settings' }))
      const dialog = await screen.findByRole('dialog', { name: 'Settings' })
      await user.click(within(dialog).getByRole('tab', { name: 'Schedule' }))
      expect(within(dialog).getByRole('button', { name: 'Apply now' })).toBeEnabled()

      await user.click(within(dialog).getByRole('radio', { name: 'Scheduled time' }))

      expect(within(dialog).getByRole('button', { name: 'Schedule' })).toBeDisabled()
    })

    it('explains that the update history is not available yet', async () => {
      const { user } = await renderDashboard({ update: upToDate })

      await user.click(await screen.findByRole('button', { name: 'Settings' }))
      const dialog = await screen.findByRole('dialog', { name: 'Settings' })
      await user.click(within(dialog).getByRole('tab', { name: 'History' }))

      expect(
        within(dialog).getByText('Update history will be available after the first update completes.'),
      ).toBeInTheDocument()
    })
  })
})
