import { act, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { adminUser, deferred, renderWithProviders, viewerUser, type TestUser } from '@/test/render'
import { authApi } from '@/lib/api/auth'
import App from './App'

const pageCalls = vi.hoisted(() => ({ playground: vi.fn(), lb: vi.fn(), audit: vi.fn() }))

// App owns routing only. Each page has its own test file, so the pages are
// replaced by markers that expose the props App passes to them.
vi.mock('@/pages/Dashboard', () => ({
  default: () => <div data-testid="page">dashboard</div>,
}))
vi.mock('@/pages/AuditLog', () => ({
  default: ({ onBack }: { onBack: () => void }) => { pageCalls.audit(); return (
    <div data-testid="page">
      audit-log <button onClick={onBack}>back</button>
    </div>
  ) },
}))
vi.mock('@/pages/LoadBalancerPlayground', () => ({
  default: ({ initialModel }: { initialModel?: string }) => { pageCalls.lb(); return (
    <div data-testid="page">lb-playground model={initialModel ?? 'none'}</div>
  ) },
}))
vi.mock('@/pages/EndpointPlayground', () => ({
  default: ({ endpointId }: { endpointId: string }) => { pageCalls.playground(); return (
    <div data-testid="page">playground endpoint={endpointId}</div>
  ) },
}))

afterEach(() => {
  window.location.hash = ''
  pageCalls.playground.mockClear()
  pageCalls.lb.mockClear()
  pageCalls.audit.mockClear()
})

async function renderAt(hash: string) {
  window.location.hash = hash
  renderWithProviders(<App />)
  return (await screen.findByTestId('page')).textContent
}

describe('App', () => {
  it('keeps unassigned direct playground URLs on the dashboard without mounting private pages', async () => {
    for (const hash of ['#lb-playground', '#playground/ep-42']) {
      window.location.hash = hash
      const view = renderWithProviders(<App />, { user: viewerUser })
      expect(await screen.findByTestId('page')).toHaveTextContent('dashboard')
      expect(pageCalls.lb).not.toHaveBeenCalled()
      expect(pageCalls.playground).not.toHaveBeenCalled()
      view.unmount()
    }
  })

  it('unmounts an allowed playground when current membership becomes unassigned', async () => {
    window.location.hash = '#lb-playground'
    renderWithProviders(<App />, { user: { ...viewerUser, organization_id: 'org-A', organization_role: 'member', scope: 'account' } })
    expect(await screen.findByTestId('page')).toHaveTextContent('lb-playground')
    const calls = pageCalls.lb.mock.calls.length
    vi.mocked(authApi.me).mockResolvedValue({ ...viewerUser, scope: 'unassigned', organization_id: null, organization_role: null })
    await act(async () => { window.dispatchEvent(new Event('llmlb:auth-scope-invalidated')) })
    expect(await screen.findByTestId('page')).toHaveTextContent('dashboard')
    expect(pageCalls.lb).toHaveBeenCalledTimes(calls)
  })

  it('keeps non-global audit URLs on the dashboard without mounting the audit page', async () => {
    window.location.hash = '#audit-log'
    renderWithProviders(<App />, { user: { ...viewerUser, organization_id: 'org-A', organization_role: 'member', scope: 'account' } })
    expect(await screen.findByTestId('page')).toHaveTextContent('dashboard')
    expect(pageCalls.audit).not.toHaveBeenCalled()
  })

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
})
