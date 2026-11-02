import type { ReactNode } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { MutationCache, QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { AuthProvider } from '@/hooks/useAuth'
import { authApi } from '@/lib/api/auth'
import { organizationsApi, type OrganizationAccount } from '@/lib/api/organizations'
import { usageApi } from '@/lib/api/usage'
import { setAuthSession } from '@/lib/authSession'
import { adminUser, deferred, type TestUser } from '@/test/render'
import { useOrganizationsViewModel } from './useOrganizationsViewModel'

const orgA = { id: 'org-A', name: 'Alpha', created_at: '2026-11-01T00:00:00Z', updated_at: '2026-11-01T00:00:00Z', archived_at: null }
const orgB = { ...orgA, id: 'org-B', name: 'Beta' }
const accountA: OrganizationAccount = {
  id: 'account-A', username: 'alice', role: 'viewer', organization_id: 'org-A',
  organization_role: 'member', disabled: false,
}
const accountB: OrganizationAccount = { ...accountA, id: 'account-B', username: 'amy' }
const totals = { request_count: 120, input_tokens: 40, output_tokens: 20, total_tokens: 60,
  measured_requests: 90, estimated_requests: 20, missing_requests: 10 }
const orgAdmin: TestUser = { ...adminUser, user_id: 'org-admin', role: 'viewer',
  organization_id: 'org-A', organization_role: 'admin', scope: 'organization:org-A' }
const member: TestUser = { ...orgAdmin, user_id: 'account-A', organization_role: 'member', scope: 'self:org-A:account-A' }

function setup(user: TestUser = { ...adminUser, scope: 'global' }, mutationCache?: MutationCache) {
  vi.spyOn(authApi, 'me').mockResolvedValue(user)
  const client = new QueryClient({ mutationCache, defaultOptions: { queries: { retry: false, gcTime: Infinity } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}><AuthProvider>{children}</AuthProvider></QueryClientProvider>
  )
  return { client, ...renderHook(() => useOrganizationsViewModel(), { wrapper }) }
}

beforeEach(() => {
  vi.spyOn(organizationsApi, 'list').mockResolvedValue([orgA, orgB])
  vi.spyOn(organizationsApi, 'accounts').mockResolvedValue([accountA, accountB])
  vi.spyOn(usageApi, 'list').mockResolvedValue({
    group_by: 'organization', from: '2026-11-01', to: '2026-11-02', page: 1, per_page: 25,
    total: 51, rows: [{ ...totals, organization_id: 'org-A', organization_name: 'Alpha',
      organization_archived: false, account_id: null, account_name: null, account_deleted: false }],
    totals, coverage_start: '2026-11-01T00:00:00Z',
  })
})

describe('organization and usage ViewModel', () => {
  it('uses UTC dates, preserves all-page totals and resets pagination when the interval changes', async () => {
    const { result } = setup()
    await waitFor(() => expect(result.current.usageRows).toHaveLength(1))
    const utcToday = new Date().toISOString().slice(0, 10)
    expect(usageApi.list).toHaveBeenLastCalledWith(expect.objectContaining({
      from: `${utcToday.slice(0, 7)}-01`, to: utcToday, group_by: 'organization', page: 1, per_page: 25,
    }))
    expect(result.current.usageTotals.request_count).toBe(120)
    expect(result.current.totalPages).toBe(3)
    act(() => result.current.changePage(2))
    await waitFor(() => expect(usageApi.list).toHaveBeenLastCalledWith(expect.objectContaining({ page: 2 })))
    act(() => result.current.setFrom('2026-10-01'))
    await waitFor(() => expect(usageApi.list).toHaveBeenLastCalledWith(expect.objectContaining({ from: '2026-10-01', page: 1 })))
  })

  it('permits organization creation and membership changes only for a system admin', async () => {
    const create = vi.spyOn(organizationsApi, 'create').mockResolvedValue(orgA)
    const update = vi.spyOn(organizationsApi, 'setMembership').mockResolvedValue(accountA)
    const { result } = setup()
    await waitFor(() => expect(result.current.canManage).toBe(true))
    act(() => result.current.setNewOrganizationName('Alpha'))
    act(() => result.current.createOrganization())
    await waitFor(() => expect(create).toHaveBeenCalledExactlyOnceWith('Alpha'))
    act(() => result.current.editMembership(accountA))
    act(() => result.current.setMembershipRole('admin'))
    act(() => result.current.saveMembership())
    await waitFor(() => expect(update).toHaveBeenCalledExactlyOnceWith('account-A', {
      organization_id: 'org-A', organization_role: 'admin',
    }))
  })

  it('keeps an organization admin inside their current organization and denies write commands', async () => {
    const create = vi.spyOn(organizationsApi, 'create')
    const membership = vi.spyOn(organizationsApi, 'setMembership')
    const { result } = setup(orgAdmin)
    await waitFor(() => expect(result.current.usageRows).toHaveLength(1))
    expect(result.current.canManage).toBe(false)
    expect(result.current.organizations.map((org) => org.id)).toEqual(['org-A'])
    act(() => result.current.selectOrganization('org-B'))
    act(() => { result.current.setNewOrganizationName('Illegal'); result.current.createOrganization() })
    act(() => { result.current.editMembership(accountA); result.current.saveMembership() })
    expect(result.current.selectedOrganizationId).toBe('org-A')
    expect(create).not.toHaveBeenCalled()
    expect(membership).not.toHaveBeenCalled()
    expect(usageApi.list).toHaveBeenLastCalledWith(expect.objectContaining({ group_by: 'account', organization_id: 'org-A' }))
  })

  it('limits a member to their own account and does not issue keys for another account', async () => {
    const issue = vi.spyOn(organizationsApi, 'issueKey')
    const { result } = setup(member)
    await waitFor(() => expect(result.current.usageRows).toHaveLength(1))
    expect(result.current.accounts.map((account) => account.id)).toEqual(['account-A'])
    expect(usageApi.list).toHaveBeenLastCalledWith(expect.objectContaining({ account_id: 'account-A', organization_id: 'org-A' }))
    act(() => result.current.openKeyForAccount(accountB))
    act(() => result.current.issueKey())
    expect(issue).not.toHaveBeenCalled()
  })

  it('does not acquire private data for an unassigned viewer', async () => {
    const { result } = setup({ ...member, organization_id: null, organization_role: null, scope: 'unassigned' })
    await waitFor(() => expect(result.current.isAuthLoading).toBe(false))
    expect(result.current.canView).toBe(false)
    expect(organizationsApi.list).not.toHaveBeenCalled()
    expect(organizationsApi.accounts).not.toHaveBeenCalled()
    expect(usageApi.list).not.toHaveBeenCalled()
  })

  it('opens only an authorized account detail and resets its usage filter on organization selection', async () => {
    const { result } = setup()
    await waitFor(() => expect(result.current.accounts).toHaveLength(2))
    act(() => result.current.selectAccount('account-A'))
    await waitFor(() => expect(usageApi.list).toHaveBeenLastCalledWith(expect.objectContaining({
      group_by: 'account', account_id: 'account-A', page: 1,
    })))
    expect(result.current.selectedAccount?.username).toBe('alice')
    act(() => result.current.selectAccount('unknown-account'))
    expect(result.current.selectedAccount?.id).toBe('account-A')
    act(() => result.current.selectOrganization('org-B'))
    expect(result.current.selectedAccount).toBeNull()
    await waitFor(() => expect(usageApi.list).toHaveBeenLastCalledWith(expect.objectContaining({ organization_id: 'org-B' })))
    expect(vi.mocked(usageApi.list).mock.lastCall?.[0].account_id).toBeUndefined()
  })

  it('keeps plaintext outside caches and clears it when the issue dialog closes', async () => {
    vi.spyOn(organizationsApi, 'issueKey').mockResolvedValue({ id: 'key-A', key: 'secret-A' })
    const { client, result } = setup()
    await waitFor(() => expect(result.current.accounts).toHaveLength(2))
    act(() => result.current.openKeyForAccount(accountA))
    act(() => result.current.setKeyName('Alice inference'))
    act(() => result.current.issueKey())
    await waitFor(() => expect(result.current.createdKey).toBe('secret-A'))
    expect(organizationsApi.issueKey).toHaveBeenCalledExactlyOnceWith({
      name: 'Alice inference', usage_account_id: 'account-A', organization_id: 'org-A',
      permissions: ['openai.inference', 'openai.models.read'],
    })
    expect(JSON.stringify(client.getQueryCache().getAll().map((query) => query.state.data))).not.toContain('secret-A')
    expect(JSON.stringify(client.getMutationCache().getAll().map((mutation) => mutation.state.data))).not.toContain('secret-A')
    act(() => result.current.clearKey())
    expect(result.current.createdKey).toBeNull()
  })

  it('does not inject an earlier target plaintext after close and reopen for another account', async () => {
    const keyResponse = deferred<{ id: string; key: string }>()
    vi.spyOn(organizationsApi, 'issueKey').mockReturnValue(keyResponse.promise)
    const { result } = setup()
    await waitFor(() => expect(result.current.accounts).toHaveLength(2))
    act(() => result.current.openKeyForAccount(accountA))
    act(() => result.current.setKeyName('Alice'))
    act(() => result.current.issueKey())
    await waitFor(() => expect(organizationsApi.issueKey).toHaveBeenCalledOnce())
    act(() => { result.current.clearKey(); result.current.openKeyForAccount(accountB) })
    await act(async () => keyResponse.resolve({ id: 'key-A', key: 'secret-A' }))
    expect(result.current.keyTarget?.id).toBe('account-B')
    expect(result.current.createdKey).toBeNull()
  })

  it('does not send a command invalidated while the mutation admission callback waits', async () => {
    const admission = deferred<void>()
    const issue = vi.spyOn(organizationsApi, 'issueKey')
    const { result } = setup(undefined, new MutationCache({ onMutate: () => admission.promise }))
    await waitFor(() => expect(result.current.accounts).toHaveLength(2))
    act(() => result.current.openKeyForAccount(accountA))
    act(() => result.current.setKeyName('Alice'))
    act(() => result.current.issueKey())
    act(() => result.current.clearKey())
    await act(async () => admission.resolve())
    expect(issue).not.toHaveBeenCalled()
    expect(result.current.createdKey).toBeNull()
  })

  it('rejects a saved form command after reopening for the same account', async () => {
    const issue = vi.spyOn(organizationsApi, 'issueKey').mockResolvedValue({ id: 'key-A', key: 'secret-A' })
    const { result } = setup()
    await waitFor(() => expect(result.current.accounts).toHaveLength(2))
    act(() => result.current.openKeyForAccount(accountA))
    act(() => result.current.setKeyName('closed form'))
    const closedCommand = result.current.issueKey
    act(() => result.current.clearKey())
    act(() => result.current.openKeyForAccount(accountA))
    act(() => result.current.setKeyName('current form'))
    await act(async () => closedCommand())
    expect(issue).not.toHaveBeenCalled()
    expect(result.current.createdKey).toBeNull()
    act(() => result.current.issueKey())
    await waitFor(() => expect(issue).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ name: 'current form' })))
    await waitFor(() => expect(result.current.createdKey).toBe('secret-A'))
  })

  it.each(['same turn', 'after success'])('consumes a key form command once: %s', async (when) => {
    const issue = vi.spyOn(organizationsApi, 'issueKey').mockResolvedValue({ id: 'key-A', key: 'secret-A' })
    const { result } = setup()
    await waitFor(() => expect(result.current.accounts).toHaveLength(2))
    act(() => result.current.openKeyForAccount(accountA))
    act(() => result.current.setKeyName('one issuance'))
    const command = result.current.issueKey
    act(() => { command(); if (when === 'same turn') command() })
    await waitFor(() => expect(result.current.createdKey).toBe('secret-A'))
    if (when === 'after success') await act(async () => command())
    expect(issue).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ name: 'one issuance' }))
  })

  it('does not combine an old-render key target with a newly authenticated session', async () => {
    const issue = vi.spyOn(organizationsApi, 'issueKey')
    const { result } = setup()
    await waitFor(() => expect(result.current.accounts).toHaveLength(2))
    act(() => result.current.openKeyForAccount(accountA))
    act(() => result.current.setKeyName('Alice'))
    const oldCommand = result.current.issueKey
    setAuthSession('other-session', 'organization:org-B')
    await act(async () => oldCommand())
    expect(issue).not.toHaveBeenCalled()
  })

  it('does not publish a secret when the authenticated session changes before its success callback', async () => {
    const callback = deferred<void>()
    vi.spyOn(organizationsApi, 'issueKey').mockResolvedValue({ id: 'key-A', key: 'secret-A' })
    const { client, result } = setup(undefined, new MutationCache({ onSuccess: () => callback.promise }))
    await waitFor(() => expect(result.current.accounts).toHaveLength(2))
    act(() => result.current.openKeyForAccount(accountA))
    act(() => result.current.setKeyName('Alice'))
    act(() => result.current.issueKey())
    await waitFor(() => expect(organizationsApi.issueKey).toHaveBeenCalledOnce())
    setAuthSession('other-session', 'organization:org-B')
    client.clear()
    await act(async () => callback.resolve())
    expect(result.current.createdKey).toBeNull()
    expect(JSON.stringify(client.getQueryCache().getAll().map((query) => query.state.data))).not.toContain('key-A')
  })
})
