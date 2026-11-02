import { act, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { adminUser, deferred, renderWithProviders, type TestUser } from '@/test/render'
import App from './App'

// App owns routing only. Each page has its own test file, so the pages are
// replaced by markers that expose the props App passes to them.
vi.mock('@/pages/Dashboard', () => ({
  default: () => <div data-testid="page">dashboard</div>,
}))
vi.mock('@/pages/AuditLog', () => ({
  default: ({ onBack }: { onBack: () => void }) => (
    <div data-testid="page">
      audit-log <button onClick={onBack}>back</button>
    </div>
  ),
}))
vi.mock('@/pages/LoadBalancerPlayground', () => ({
  default: ({ initialModel }: { initialModel?: string }) => (
    <div data-testid="page">lb-playground model={initialModel ?? 'none'}</div>
  ),
}))
vi.mock('@/pages/EndpointPlayground', () => ({
  default: ({ endpointId }: { endpointId: string }) => (
    <div data-testid="page">playground endpoint={endpointId}</div>
  ),
}))
vi.mock('@/pages/Organizations', () => ({
  default: () => <div data-testid="page">organizations</div>,
}))

afterEach(() => {
  window.location.hash = ''
})

async function renderAt(hash: string) {
  window.location.hash = hash
  renderWithProviders(<App />)
  return (await screen.findByTestId('page')).textContent
}

describe('App', () => {
  it('shows a loading indicator until the session check completes', async () => {
    const session = deferred<TestUser>()
    renderWithProviders(<App />, { user: session.promise })

    expect(screen.getByText('Loading...')).toBeInTheDocument()
    expect(screen.queryByTestId('page')).not.toBeInTheDocument()

    await act(async () => session.resolve(adminUser))

    expect(screen.queryByText('Loading...')).not.toBeInTheDocument()
    expect(screen.getByTestId('page')).toHaveTextContent('dashboard')
  })

  it.each([
    ['', 'dashboard'],
    ['#audit-log', 'audit-log back'],
    ['#organizations', 'organizations'],
    ['#lb-playground', 'lb-playground model=none'],
    ['#lb-playground?model=gpt-oss%3A20b', 'lb-playground model=gpt-oss:20b'],
    ['#playground/ep-42', 'playground endpoint=ep-42'],
    ['#playground/', 'dashboard'],
    ['#unknown-route', 'dashboard'],
  ])('routes %j to %j', async (hash, expected) => {
    expect(await renderAt(hash)).toBe(expected)
  })

  it('follows hash changes after the first render', async () => {
    await renderAt('')

    await act(async () => {
      window.location.hash = '#audit-log'
      window.dispatchEvent(new HashChangeEvent('hashchange'))
    })

    expect(screen.getByTestId('page')).toHaveTextContent('audit-log')
  })

  it('returns to the dashboard and clears the hash when a page calls onBack', async () => {
    await renderAt('#audit-log')

    await userEvent.setup().click(screen.getByRole('button', { name: 'back' }))

    expect(screen.getByTestId('page')).toHaveTextContent('dashboard')
    expect(window.location.hash).toBe('')
  })

  it.each(['#lb-playground', '#playground/endpoint-A', '#audit-log', '#organizations'])('does not mount %s while signed out', async (hash) => {
    window.location.hash = hash
    renderWithProviders(<App />, { user: null })
    await waitFor(() => expect(screen.queryByText('Loading...')).not.toBeInTheDocument())
    expect(screen.queryByTestId('page')).not.toBeInTheDocument()
  })

  it('does not mount a private page for an account that must change its password', async () => {
    window.location.hash = '#lb-playground'
    renderWithProviders(<App />, { user: { ...adminUser, must_change_password: true } })
    await waitFor(() => expect(screen.queryByText('Loading...')).not.toBeInTheDocument())
    expect(screen.queryByTestId('page')).not.toBeInTheDocument()
  })

  it('blocks direct audit navigation for an assigned organization administrator', async () => {
    window.location.hash = '#audit-log'
    renderWithProviders(<App />, { user: { ...adminUser, role: 'viewer',
      organization_id: 'org-A', organization_role: 'admin', scope: 'organization:org-A' } })
    expect(await screen.findByText(/System administrator access required/i)).toBeInTheDocument()
    expect(screen.queryByText('audit-log back')).not.toBeInTheDocument()
  })

  it.each(['#lb-playground', '#playground/endpoint-A'])('requires membership before mounting %s for a viewer', async (hash) => {
    window.location.hash = hash
    renderWithProviders(<App />, { user: { ...adminUser, role: 'viewer', organization_id: null, organization_role: null, scope: 'unassigned' } })
    expect(await screen.findByText(/Ask a system administrator to assign your organization/i)).toBeInTheDocument()
    expect(screen.queryByTestId('page')).not.toBeInTheDocument()
  })
})
