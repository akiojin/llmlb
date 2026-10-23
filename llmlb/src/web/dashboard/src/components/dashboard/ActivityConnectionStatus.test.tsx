import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { ActivityConnectionStatus } from './ActivityConnectionStatus'

describe('activity connection status', () => {
  it('shows WebSocket live with a reduced-motion-aware green dot', () => {
    render(<ActivityConnectionStatus status="live" />)
    expect(screen.getByRole('status')).toHaveTextContent('Live · WebSocket')
    expect(screen.getByTestId('activity-connection-dot')).toHaveClass('bg-success', 'motion-safe:animate-pulse')
  })

  it('shows the one-second fallback and keeps stale updates explicit', () => {
    const { rerender } = render(<ActivityConnectionStatus status="polling" />)
    expect(screen.getByRole('status')).toHaveTextContent('Polling · 1s · WebSocket 未接続')
    expect(screen.getByTestId('activity-connection-dot')).not.toHaveClass('motion-safe:animate-pulse')
    rerender(<ActivityConnectionStatus status="polling" isStale />)
    expect(screen.getByRole('status')).toHaveTextContent('Update delayed')
  })

  it('retains the actual WebSocket connection when only the activity fetch fails', () => {
    render(<ActivityConnectionStatus status="live" isStale />)
    expect(screen.getByRole('status')).toHaveTextContent('Live · WebSocket')
    expect(screen.getByRole('status')).toHaveTextContent('Update delayed')
    expect(screen.getByRole('status')).not.toHaveTextContent('WebSocket 未接続')
  })
})
