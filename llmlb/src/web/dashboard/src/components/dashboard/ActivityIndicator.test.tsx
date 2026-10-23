import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { ActivityIndicator } from './ActivityIndicator'

const series = Array.from({ length: 60 }, (_, i) => i === 58 ? 4 : 0)

describe('request activity indicator (Issue #937)', () => {
  it('shows current workload separately from the last sixty seconds of arrivals', () => {
    render(<ActivityIndicator activity={{ active_requests: 2, queued_requests: 1, requests_per_second: series }} />)

    const indicator = screen.getByTestId('request-activity')
    expect(indicator).toHaveAttribute('data-state', 'busy')
    expect(indicator).toHaveAttribute('data-active-requests', '2')
    expect(indicator).toHaveAttribute('data-queued-requests', '1')
    expect(screen.getByText('2 active')).toBeInTheDocument()
    expect(screen.getByText('1 queued')).toBeInTheDocument()
    const chart = screen.getByRole('img', { name: 'Requests per second over the last 60 seconds' })
    expect(chart).toHaveAttribute('data-sample-count', '60')
    expect(chart.querySelector('polyline')).toHaveAttribute('points', expect.stringContaining(','))
    expect(screen.getByText('4 req/s peak')).toBeInTheDocument()
  })

  it('shows an idle rail without hiding a short request that has already completed', () => {
    render(<ActivityIndicator activity={{ active_requests: 0, queued_requests: 0, requests_per_second: series }} />)

    expect(screen.getByTestId('request-activity')).toHaveAttribute('data-state', 'idle')
    expect(screen.getByText('Idle')).toBeInTheDocument()
    expect(screen.getByRole('img')).toBeInTheDocument()
    expect(screen.getByText('4 req/s peak')).toBeInTheDocument()
  })

  it('distinguishes an unavailable sample from idle and flags retained stale data', () => {
    const { rerender } = render(<ActivityIndicator />)
    expect(screen.getByText('Waiting for activity')).toBeInTheDocument()
    expect(screen.queryByText('Idle')).not.toBeInTheDocument()

    rerender(<ActivityIndicator activity={{ active_requests: 2, queued_requests: 0, requests_per_second: series }} isStale />)
    expect(screen.getByText('Last known activity')).toBeInTheDocument()
    expect(screen.getByTestId('request-activity')).toHaveAttribute('data-active-requests', '2')
  })

  it('makes workload animation optional for reduced motion and uses theme colors', () => {
    render(<ActivityIndicator activity={{ active_requests: 1, queued_requests: 1, requests_per_second: series }} compact />)
    const activeBar = screen.getByTestId('activity-active-bar')
    expect(activeBar.className).toContain('motion-safe:animate-pulse')
    expect(activeBar.className).toContain('motion-reduce:transition-none')
    expect(activeBar.className).toContain('bg-accent-foreground')
  })

  it('uses configured capacity and distinguishes waiting requests with a striped segment', () => {
    render(<ActivityIndicator activity={{ active_requests: 2, queued_requests: 1, requests_per_second: series }} capacity={4} compact />)
    expect(screen.getByTestId('activity-active-bar')).toHaveStyle({ width: '50%' })
    expect(screen.getByTestId('activity-queued-bar')).toHaveStyle({ width: '25%' })
    expect(screen.getByTestId('activity-queued-bar')).toHaveClass('activity-queue-stripes')
    expect(screen.getByText('2 / 4')).toBeInTheDocument()
    expect(screen.getByText('+1 待ち')).toBeInTheDocument()
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
  })
})
