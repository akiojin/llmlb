import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { OperationsOverview } from './OperationsOverview'

const activity = {
  timestamp: '2026-10-22T00:00:00Z',
  bucket_seconds: 1,
  window_seconds: 60,
  total: { active_requests: 2, queued_requests: 1, requests_per_second: Array(60).fill(0) },
  endpoints: [],
}

describe('operations request activity (Issue #937)', () => {
  it('shows overall activity with an explicit polling status', () => {
    const { container } = render(<OperationsOverview isLoading={false} activity={activity} activityStatus="polling" />)

    const region = screen.getByRole('region', { name: 'Request activity' })
    expect(within(region).getByText('Polling · 1s')).toBeInTheDocument()
    expect(within(region).getByTestId('request-activity')).toHaveAttribute('data-active-requests', '2')
    expect(container.querySelector('[data-stat="active-requests"]')).toHaveTextContent('2')
    expect(container.querySelector('[data-stat="queued-requests"]')).toHaveTextContent('1')
  })

  it('distinguishes the live connection from a delayed update', () => {
    const { rerender } = render(<OperationsOverview isLoading={false} activity={activity} activityStatus="live" />)
    expect(screen.getByText('Live · 1s')).toBeInTheDocument()

    rerender(<OperationsOverview isLoading={false} activity={activity} activityStatus="stale" />)
    expect(screen.getByText('Update delayed')).toBeInTheDocument()
    expect(screen.getByText('Last known activity')).toBeInTheDocument()
  })

  it('reproduces the six overview tiles, five-second average and current area graph', () => {
    const samples = [...Array(55).fill(0), 2, 4, 6, 8, 10]
    const { container } = render(<OperationsOverview isLoading={false} activity={{ ...activity, total: { ...activity.total, requests_per_second: samples } }} />)

    const tiles = screen.getByTestId('operations-tiles')
    expect(tiles).toHaveClass('grid-cols-3')
    expect(tiles.children).toHaveLength(6)
    expect(container.querySelector('[data-stat="requests-per-second"]')).toHaveTextContent('6.0')
    expect(screen.getByText('直近5秒平均')).toBeInTheDocument()
    expect(screen.getByText('処理中')).toHaveClass('motion-safe:animate-pulse')
    const region = screen.getByRole('region', { name: 'Request activity' })
    expect(within(region).getByTestId('activity-current-rate')).toHaveTextContent('10')
    expect(within(region).getByText('req/s（現在）')).toBeInTheDocument()
    const graph = within(region).getByRole('img', { name: 'Requests per second over the last 60 seconds' })
    expect(graph.querySelector('[data-testid="activity-area"]')).toBeInTheDocument()
    expect(graph.querySelector('[data-testid="activity-latest-point"]')).toBeInTheDocument()
    expect(within(region).getByText('-60s')).toBeInTheDocument()
    expect(within(region).getByText('-30s')).toBeInTheDocument()
    expect(within(region).getByText('now')).toBeInTheDocument()
    fireEvent.focus(screen.getByRole('slider', { name: 'Inspect request rate' }))
    expect(screen.getByRole('tooltip')).toHaveTextContent('10 req/s')
    fireEvent.keyDown(screen.getByRole('slider'), { key: 'ArrowLeft' })
    expect(screen.getByRole('tooltip')).toHaveTextContent('8 req/s')
  })

  it('shows the idle tile without a pulse when no requests are active', () => {
    render(<OperationsOverview isLoading={false} activity={{ ...activity, total: { ...activity.total, active_requests: 0, queued_requests: 0 } }} />)
    expect(screen.getByText('アイドル')).not.toHaveClass('motion-safe:animate-pulse')
  })

  it('uses the daily request count and leaves it unknown until daily statistics arrive', () => {
    const { container, rerender } = render(<OperationsOverview isLoading={false} activity={activity} />)
    expect(container.querySelector('[data-stat="total-requests"]')).toHaveTextContent('—')
    rerender(<OperationsOverview isLoading={false} activity={activity} todayRequests={27} />)
    expect(container.querySelector('[data-stat="total-requests"]')).toHaveTextContent('27')
    expect(container.querySelector('[data-stat="total-requests"]')).toHaveTextContent('本日累計')
  })
})
