import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { dashboardApi, type DailyTokenStats, type MonthlyTokenStats } from '@/lib/api'
import { deferred, renderWithProviders } from '@/test/render'
import { TokenStatsSection } from './TokenStatsSection'

function daily(overrides: Partial<DailyTokenStats> = {}): DailyTokenStats {
  return {
    date: '2026-09-30',
    request_count: 12,
    total_input_tokens: 1500,
    total_output_tokens: 2500,
    total_tokens: 4000,
    ...overrides,
  }
}

function monthly(overrides: Partial<MonthlyTokenStats> = {}): MonthlyTokenStats {
  return {
    month: '2026-09',
    request_count: 340,
    total_input_tokens: 1_200_000,
    total_output_tokens: 3_400_000,
    total_tokens: 4_600_000,
    ...overrides,
  }
}

beforeEach(() => {
  // jsdom has no layout, so Recharts warns that each chart measures 0x0.
  // Only that warning is dropped; anything else still reaches the console.
  const warn = console.warn
  vi.spyOn(console, 'warn').mockImplementation((...args: unknown[]) => {
    if (String(args[0]).startsWith('The width(0) and height(0) of chart')) return
    warn(...args)
  })

  vi.spyOn(dashboardApi, 'getDailyTokenStats').mockResolvedValue([])
  vi.spyOn(dashboardApi, 'getMonthlyTokenStats').mockResolvedValue([])
})

describe('TokenStatsSection', () => {
  it('switches the daily tab from loading to the last 7 days of usage', async () => {
    const response = deferred<DailyTokenStats[]>()
    const getDaily = vi.spyOn(dashboardApi, 'getDailyTokenStats').mockReturnValue(response.promise)
    renderWithProviders(<TokenStatsSection />)

    expect(await screen.findByRole('status', { name: 'Loading' })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: 'Daily' })).toHaveAttribute('aria-selected', 'true')
    expect(getDaily).toHaveBeenCalledExactlyOnceWith(7)
    expect(screen.queryByText('No daily statistics available')).not.toBeInTheDocument()

    response.resolve([
      daily(),
      daily({
        date: '2026-09-29',
        request_count: 7,
        total_input_tokens: 800,
        total_output_tokens: 150,
        total_tokens: 950,
      }),
    ])

    expect(await screen.findByText('2026-09-30')).toBeInTheDocument()
    expect(screen.queryByRole('status', { name: 'Loading' })).not.toBeInTheDocument()
    expect(screen.getByText('Date')).toBeInTheDocument()
    for (const cell of ['12', '1.5K', '2.5K', '4.0K', '2026-09-29', '7', '800', '150', '950']) {
      expect(screen.getByText(cell)).toBeInTheDocument()
    }
    expect(screen.getByRole('img', { name: 'Token Statistics' })).toBeInTheDocument()
  })

  it('shows the daily empty state when no usage was recorded', async () => {
    renderWithProviders(<TokenStatsSection />)

    expect(await screen.findByText('No daily statistics available')).toBeInTheDocument()
    expect(screen.queryByRole('status', { name: 'Loading' })).not.toBeInTheDocument()
    expect(screen.queryByRole('img', { name: 'Token Statistics' })).not.toBeInTheDocument()
  })

  it('switches the monthly tab from loading to the last 6 months of usage', async () => {
    const response = deferred<MonthlyTokenStats[]>()
    const getMonthly = vi
      .spyOn(dashboardApi, 'getMonthlyTokenStats')
      .mockReturnValue(response.promise)
    vi.spyOn(dashboardApi, 'getDailyTokenStats').mockResolvedValue([daily()])
    renderWithProviders(<TokenStatsSection />)
    await screen.findByText('2026-09-30')

    await userEvent.setup().click(screen.getByRole('tab', { name: 'Monthly' }))

    expect(screen.getByRole('status', { name: 'Loading' })).toBeInTheDocument()
    expect(screen.queryByText('2026-09-30')).not.toBeInTheDocument()
    expect(getMonthly).toHaveBeenCalledExactlyOnceWith(6)

    response.resolve([monthly()])

    expect(await screen.findByText('2026-09')).toBeInTheDocument()
    expect(screen.queryByRole('status', { name: 'Loading' })).not.toBeInTheDocument()
    expect(screen.getByText('Month')).toBeInTheDocument()
    for (const cell of ['340', '1.2M', '3.4M', '4.6M']) {
      expect(screen.getByText(cell)).toBeInTheDocument()
    }
  })

  it('shows the monthly empty state independently of the daily data', async () => {
    vi.spyOn(dashboardApi, 'getDailyTokenStats').mockResolvedValue([daily()])
    renderWithProviders(<TokenStatsSection />)
    await screen.findByText('2026-09-30')

    await userEvent.setup().click(screen.getByRole('tab', { name: 'Monthly' }))

    expect(await screen.findByText('No monthly statistics available')).toBeInTheDocument()
    expect(screen.queryByText('No daily statistics available')).not.toBeInTheDocument()
  })

  it('returns to the daily usage from the monthly tab', async () => {
    vi.spyOn(dashboardApi, 'getDailyTokenStats').mockResolvedValue([daily()])
    vi.spyOn(dashboardApi, 'getMonthlyTokenStats').mockResolvedValue([monthly()])
    const user = userEvent.setup()
    renderWithProviders(<TokenStatsSection />)
    await screen.findByText('2026-09-30')

    await user.click(screen.getByRole('tab', { name: 'Monthly' }))
    expect(await screen.findByText('2026-09')).toBeInTheDocument()

    await user.click(screen.getByRole('tab', { name: 'Daily' }))
    expect(await screen.findByText('2026-09-30')).toBeInTheDocument()
    expect(screen.queryByText('2026-09')).not.toBeInTheDocument()
  })
})
