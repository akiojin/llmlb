import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { organizationsApi } from '@/lib/api/organizations'
import { usageApi } from '@/lib/api/usage'
import { adminUser, renderPage } from '@/test/render'
import OrganizationsPage from './Organizations'

const alpha = { id: 'org-A', name: 'Alpha', created_at: '2026-11-01T00:00:00Z', updated_at: '2026-11-01T00:00:00Z', archived_at: null }
const account = { id: 'alice', username: 'alice', role: 'viewer', organization_id: 'org-A', organization_role: 'member' as const, disabled: false }
const totals = { request_count: 12, input_tokens: 40, output_tokens: 20, total_tokens: 60,
  measured_requests: 9, estimated_requests: 2, missing_requests: 1 }

beforeEach(() => {
  vi.spyOn(organizationsApi, 'list').mockResolvedValue([alpha])
  vi.spyOn(organizationsApi, 'accounts').mockResolvedValue([account])
  vi.spyOn(usageApi, 'list').mockResolvedValue({
    group_by: 'organization', from: '2026-11-01', to: '2026-11-02', page: 1, per_page: 25,
    total: 1, rows: [{ ...totals, organization_id: 'org-A', organization_name: 'Alpha',
      organization_archived: false, account_id: null, account_name: null, account_deleted: false }],
    totals, coverage_start: '2026-11-01T00:00:00Z',
  })
})

describe('Organizations page', () => {
  it('exposes the page sections and selected account detail as level two headings', async () => {
    renderPage(<OrganizationsPage onBack={vi.fn()} />, { user: { ...adminUser, scope: 'global' } })
    expect(await screen.findByRole('heading', { name: 'Organizations & usage', level: 1 })).toBeInTheDocument()
    for (const name of ['Organizations', 'Create organization', 'Usage', 'Accounts']) {
      expect(screen.getByRole('heading', { name, level: 2 })).toBeInTheDocument()
    }
    await userEvent.setup().click(await screen.findByRole('button', { name: 'Alpha' }))
    expect(screen.getByRole('heading', { name: 'Organization details', level: 2 })).toBeInTheDocument()
    await userEvent.setup().click(screen.getByRole('button', { name: 'View account alice' }))
    expect(screen.getByRole('heading', { name: 'Account detail · alice', level: 2 })).toBeInTheDocument()
  })

  it('provides real organization creation and UTC usage with honest coverage and quota status', async () => {
    const create = vi.spyOn(organizationsApi, 'create').mockResolvedValue(alpha)
    renderPage(<OrganizationsPage onBack={vi.fn()} />, { user: { ...adminUser, scope: 'global' } })
    expect(await screen.findByRole('heading', { name: 'Organizations & usage' })).toBeInTheDocument()
    expect(screen.getByText('All organizations')).toBeInTheDocument()
    expect(screen.getByLabelText('From (UTC, inclusive)')).toHaveAttribute('type', 'date')
    expect(screen.getByLabelText('To (UTC, inclusive)')).toHaveAttribute('type', 'date')
    expect(await screen.findByText('9 measured · 2 estimated · 1 missing')).toBeInTheDocument()
    expect(screen.getByText(/Monthly quota is not available/i)).toBeInTheDocument()
    await userEvent.setup().type(screen.getByLabelText('New organization name'), 'Alpha')
    await userEvent.setup().click(screen.getByRole('button', { name: 'Create organization' }))
    await waitFor(() => expect(create).toHaveBeenCalledExactlyOnceWith('Alpha'))
  })

  it('shows an organization administrator a read-only current organization view', async () => {
    renderPage(<OrganizationsPage onBack={vi.fn()} />, { user: { ...adminUser,
      role: 'viewer', organization_id: 'org-A', organization_role: 'admin', scope: 'organization:org-A' } })
    expect(await screen.findByText('Your organization')).toBeInTheDocument()
    expect(await screen.findByText('alice')).toBeInTheDocument()
    expect(screen.queryByLabelText('New organization name')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /Edit membership/ })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /Issue API key for/ })).not.toBeInTheDocument()
  })

  it('shows a member their own account without an authority preview selector', async () => {
    renderPage(<OrganizationsPage onBack={vi.fn()} />, { user: { ...adminUser,
      user_id: 'alice', role: 'viewer', organization_id: 'org-A', organization_role: 'member', scope: 'self:org-A:alice' } })
    expect(await screen.findByText('Your account')).toBeInTheDocument()
    expect(await screen.findByText('alice')).toBeInTheDocument()
    expect(screen.queryByLabelText(/View as|Preview role/i)).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Create organization' })).not.toBeInTheDocument()
  })

  it('labels legacy and deleted usage without assigning it to an existing organization', async () => {
    vi.mocked(usageApi.list).mockResolvedValue({
      group_by: 'account', from: '2026-11-01', to: '2026-11-02', page: 1, per_page: 25,
      total: 1, rows: [{ ...totals, organization_id: null, organization_name: null,
        organization_archived: false, account_id: 'deleted-A', account_name: null, account_deleted: true }],
      totals, coverage_start: '2026-11-01T00:00:00Z',
    })
    renderPage(<OrganizationsPage onBack={vi.fn()} />, { user: { ...adminUser, scope: 'global' } })
    expect(await screen.findByText('Unattributed (legacy)')).toBeInTheDocument()
    expect(screen.getByText('Deleted account')).toBeInTheDocument()
    expect(screen.getByText(/Coverage begins/)).toBeInTheDocument()
  })

  it('gives an unassigned account a recovery message without loading private APIs', async () => {
    renderPage(<OrganizationsPage onBack={vi.fn()} />, { user: { ...adminUser,
      role: 'viewer', organization_id: null, organization_role: null, scope: 'unassigned' } })
    expect(await screen.findByText(/Ask a system administrator to assign your organization/i)).toBeInTheDocument()
    expect(organizationsApi.list).not.toHaveBeenCalled()
    expect(usageApi.list).not.toHaveBeenCalled()
  })
})
