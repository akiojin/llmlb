import type { ReactNode } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { focusManager, MutationCache, QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { apiKeysApi, registrationRequestsApi, usersApi, type ApiKey, type RegistrationRequest, type User } from '@/lib/api'
import { organizationsApi, type OrganizationAccount } from '@/lib/api/organizations'
import { queryKeys } from '@/lib/queryKeys'
import { setExpectedUsageScope } from '@/lib/api/client'
import { invalidateDashboardSubscriptions } from '@/hooks/dashboardSubscriptions'
import { useApiKeyModalViewModel } from './useApiKeyModalViewModel'
import { useOrganizationsViewModel } from './useOrganizationsViewModel'
import { useUserModalViewModel } from './useUserModalViewModel'

const { toast, auth } = vi.hoisted(() => ({
  toast: vi.fn(),
  auth: { id: 'current-issuer', role: 'admin', organization_id: 'organization-A' as string | null, scope: 'global' },
}))
vi.mock('@/hooks/use-toast', () => ({ toast }))
vi.mock('@/hooks/useAuth', () => ({ useAuth: () => ({ user: auth }) }))

const key: ApiKey = {
  id: 'key-A', name: 'existing', created_at: '2026-10-01T00:00:00Z',
  key_prefix: 'sk_prefix', permissions: ['openai.inference', 'openai.models.read'],
}
const user: User = {
  id: 'user-A', username: 'alice', role: 'viewer', email: 'alice@example.com',
  created_at: '2026-10-01T00:00:00Z',
}
const registration: RegistrationRequest = {
  id: 'request-A', username: 'pending@example.com', created_at: '2026-10-01T00:00:00Z',
}

function setup(polling = false, mutationCache?: MutationCache) {
  const client = new QueryClient({ mutationCache, defaultOptions: { queries: {
    retry: false, gcTime: Infinity,
    ...(polling ? { refetchInterval: 5000, refetchIntervalInBackground: true } : {}),
  } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
  return { client, wrapper }
}

function mutationBarrier() {
  let release = () => {}
  const promise = new Promise<void>((resolve) => { release = resolve })
  return { promise, release }
}

function mockOrganizationKeyTargets() {
  const organizationA = { id: 'organization-A', name: 'Alpha', created_at: '2026-10-01T00:00:00Z', updated_at: '2026-10-01T00:00:00Z', archived_at: null }
  const organizationB = { ...organizationA, id: 'organization-B', name: 'Beta' }
  const accountA: OrganizationAccount = { id: 'account-A', username: 'alpha@example.com', role: 'viewer', organization_id: organizationA.id, organization_role: 'member' }
  const accountB: OrganizationAccount = { ...accountA, id: 'account-B', username: 'beta@example.com', organization_id: organizationB.id }
  vi.spyOn(organizationsApi, 'list').mockResolvedValue([organizationA, organizationB])
  vi.spyOn(organizationsApi, 'members').mockImplementation(async (id) => id === organizationA.id ? [accountA] : [accountB])
  vi.spyOn(usersApi, 'list').mockResolvedValue([])
  return { organizationA, organizationB, accountA, accountB }
}

beforeEach(() => {
  vi.spyOn(registrationRequestsApi, 'list').mockResolvedValue([])
})

afterEach(() => {
  vi.useRealTimers()
  focusManager.setFocused(undefined)
  setExpectedUsageScope(null, null)
  Object.assign(auth, { id: 'current-issuer', role: 'admin', organization_id: 'organization-A', scope: 'global' })
})

describe('management modal ViewModels', () => {
  it('discards an unfinished self key after the main dialog is closed and reopened', async () => {
    toast.mockClear()
    vi.spyOn(apiKeysApi, 'list').mockResolvedValue([])
    const entered = mutationBarrier()
    const response = mutationBarrier()
    const settled = mutationBarrier()
    vi.spyOn(apiKeysApi, 'create').mockImplementation(async () => {
      entered.release()
      await response.promise
      return { ...key, key_prefix: 'sk_old', key: 'sk_old_plaintext' }
    })
    const { client, wrapper } = setup(false, new MutationCache({ onSettled: () => { settled.release() } }))
    const { result, rerender } = renderHook(({ open }) => useApiKeyModalViewModel({ open, onOpenChange: vi.fn() }),
      { wrapper, initialProps: { open: true } })
    act(() => { result.current.handleOpenCreateDialog(); result.current.setNewKeyName('old key') })
    await act(async () => { result.current.handleCreate(); await entered.promise })
    act(() => result.current.handleMainOpenChange(false))
    rerender({ open: false })
    rerender({ open: true })
    act(() => { result.current.handleOpenCreateDialog(); result.current.setNewKeyName('new form') })
    await act(async () => { response.release(); await settled.promise })

    expect(result.current.createdKey).toBeNull()
    expect(result.current.newKeyName).toBe('new form')
    expect(result.current.createOpen).toBe(true)
    expect(client.getQueryData(queryKeys.apiKeys())).toEqual([])
    expect(toast).not.toHaveBeenCalled()
  })

  it.each(['cancel', 'close', 'refresh'] as const)('does not send a queued self key command after %s', async (action) => {
    toast.mockClear()
    vi.spyOn(apiKeysApi, 'list').mockResolvedValue([])
    const entered = mutationBarrier()
    const release = mutationBarrier()
    const settled = mutationBarrier()
    const create = vi.spyOn(apiKeysApi, 'create').mockResolvedValue({ ...key, key_prefix: 'sk_old', key: 'sk_old_plaintext' })
    const { wrapper } = setup(false, new MutationCache({
      onMutate: async () => { entered.release(); await release.promise },
      onSettled: () => { settled.release() },
    }))
    const { result } = renderHook(() => useApiKeyModalViewModel({ open: true, onOpenChange: vi.fn() }), { wrapper })
    act(() => { result.current.handleOpenCreateDialog(); result.current.setNewKeyName('cancelled key') })
    await act(async () => { result.current.handleCreate(); await entered.promise })
    act(() => {
      if (action === 'cancel') result.current.handleCreateOpenChange(false)
      else if (action === 'close') result.current.handleMainOpenChange(false)
      else result.current.handleRefresh()
    })
    await act(async () => { release.release(); await settled.promise })

    expect(create).not.toHaveBeenCalled()
    expect(result.current.createdKey).toBeNull()
    expect(toast).not.toHaveBeenCalled()
  })

  it('does not reveal an old organization key after close and reopen for a different target', async () => {
    const { organizationA, organizationB, accountA, accountB } = mockOrganizationKeyTargets()
    const entered = mutationBarrier()
    const response = mutationBarrier()
    const settled = mutationBarrier()
    const create = vi.spyOn(apiKeysApi, 'create').mockImplementation(async () => {
      entered.release()
      await response.promise
      return { ...key, key_prefix: 'sk_alpha', key: 'sk_alpha_plaintext' }
    })
    const { wrapper } = setup(false, new MutationCache({ onSettled: () => { settled.release() } }))
    const { result, rerender } = renderHook(({ open }) => useOrganizationsViewModel(open), { wrapper, initialProps: { open: true } })
    await waitFor(() => expect(result.current.members).toEqual([accountA]))
    act(() => { result.current.openIssueKey(accountA); result.current.setKeyName('alpha key') })
    await act(async () => { result.current.issueKey(); await entered.promise })
    expect(create).toHaveBeenCalledExactlyOnceWith({ name: 'alpha key', usage_account_id: accountA.id,
      organization_id: organizationA.id, permissions: ['openai.inference', 'openai.models.read'] })

    act(() => result.current.clearKey())
    rerender({ open: false })
    rerender({ open: true })
    act(() => result.current.select(organizationB.id))
    await waitFor(() => expect(result.current.members).toEqual([accountB]))
    act(() => { result.current.openIssueKey(accountB); result.current.setKeyName('beta key') })
    expect(result.current.issuedKey).toBeNull()
    await act(async () => { response.release(); await settled.promise })

    expect(result.current.selected?.id).toBe(organizationB.id)
    expect(result.current.keyAccount?.id).toBe(accountB.id)
    expect(result.current.issuedKey).toBeNull()
    expect(result.current.keyName).toBe('beta key')
  })

  it('does not send a queued organization key command after its issue form is cleared', async () => {
    const { organizationB, accountA, accountB } = mockOrganizationKeyTargets()
    const entered = mutationBarrier()
    const release = mutationBarrier()
    const settled = mutationBarrier()
    const mutationCache = new MutationCache({
      onMutate: async () => { entered.release(); await release.promise },
      onSettled: () => { settled.release() },
    })
    const create = vi.spyOn(apiKeysApi, 'create').mockResolvedValue({ ...key, key_prefix: 'sk_alpha', key: 'sk_alpha_plaintext' })
    const { wrapper } = setup(false, mutationCache)
    const { result } = renderHook(() => useOrganizationsViewModel(true), { wrapper })
    await waitFor(() => expect(result.current.members).toEqual([accountA]))
    act(() => { result.current.openIssueKey(accountA); result.current.setKeyName('alpha key') })
    await act(async () => { result.current.issueKey(); await entered.promise })
    expect(create).not.toHaveBeenCalled()

    act(() => result.current.clearKey())
    act(() => result.current.select(organizationB.id))
    await waitFor(() => expect(result.current.members).toEqual([accountB]))
    act(() => { result.current.openIssueKey(accountB); result.current.setKeyName('beta key') })
    await act(async () => { release.release(); await settled.promise })

    expect(create).not.toHaveBeenCalled()
    expect(result.current.issuedKey).toBeNull()
    expect(result.current.keyName).toBe('beta key')
  })

  it('rejects an old key callback after global MutationCache success waits across unmount and identity change', async () => {
    setExpectedUsageScope('global', 'previous-account')
    toast.mockClear()
    const entered = mutationBarrier()
    const release = mutationBarrier()
    const settled = mutationBarrier()
    const mutationCache = new MutationCache({
      onSuccess: async () => { entered.release(); await release.promise },
      onSettled: () => { settled.release() },
    })
    const created = { ...key, id: 'old-callback-key', name: 'previous account metadata', key_prefix: 'sk_old', key: 'sk_old_secret' }
    const create = vi.spyOn(apiKeysApi, 'create').mockResolvedValue(created)
    const { client, wrapper } = setup(false, mutationCache)
    const { result, unmount } = renderHook(() => useApiKeyModalViewModel({ open: false, onOpenChange: vi.fn() }), { wrapper })
    act(() => result.current.setNewKeyName(created.name))
    await act(async () => { result.current.handleCreate(); await entered.promise })
    expect(create).toHaveBeenCalledOnce()
    await act(async () => {
      unmount()
      client.clear()
      setExpectedUsageScope('global', 'next-account')
      release.release()
      await settled.promise
    })
    expect(client.getQueryData(queryKeys.apiKeys())).toBeUndefined()
    expect(toast).not.toHaveBeenCalled()
  })

  it('does not send a queued key command after global MutationCache mutate waits across identity change', async () => {
    setExpectedUsageScope('global', 'previous-account')
    toast.mockClear()
    const entered = mutationBarrier()
    const release = mutationBarrier()
    const settled = mutationBarrier()
    const mutationCache = new MutationCache({
      onMutate: async () => { entered.release(); await release.promise },
      onSettled: () => { settled.release() },
    })
    const create = vi.spyOn(apiKeysApi, 'create').mockResolvedValue({ ...key, key_prefix: 'sk_queued', key: 'sk_queued_secret' })
    const { wrapper } = setup(false, mutationCache)
    const { result } = renderHook(() => useApiKeyModalViewModel({ open: false, onOpenChange: vi.fn() }), { wrapper })
    act(() => result.current.setNewKeyName('previous account command'))
    await act(async () => { result.current.handleCreate(); await entered.promise })
    expect(create).not.toHaveBeenCalled()
    await act(async () => {
      setExpectedUsageScope('global', 'next-account')
      release.release()
      await settled.promise
    })
    expect(create).not.toHaveBeenCalled()
    expect(toast).not.toHaveBeenCalled()
  })

  it('marks other-issuer and unknown-issuer scoped keys read only and suppresses their delete calls', async () => {
    const owned = { ...key, id: 'owned', created_by: auth.id }
    const foreign = { ...key, id: 'foreign', created_by: 'another-issuer' }
    const unknown = { ...key, id: 'unknown' }
    vi.spyOn(apiKeysApi, 'list').mockResolvedValue([owned, foreign, unknown])
    const remove = vi.spyOn(apiKeysApi, 'delete').mockResolvedValue(undefined)
    const { wrapper } = setup()
    const { result } = renderHook(() => useApiKeyModalViewModel({ open: true, onOpenChange: vi.fn() }), { wrapper })
    await waitFor(() => expect(result.current.apiKeys).toHaveLength(3))
    expect(result.current.apiKeys).toEqual(expect.arrayContaining([
      expect.objectContaining({ id: 'owned', canDelete: true }),
      expect.objectContaining({ id: 'foreign', canDelete: false }),
      expect.objectContaining({ id: 'unknown', canDelete: false }),
    ]))
    for (const row of [foreign, unknown]) {
      act(() => result.current.setDeleteKey(row))
      act(() => result.current.handleDelete())
    }
    expect(remove).not.toHaveBeenCalled()
  })

  it('stores the authenticated issuer on a created public key without caching its plaintext', async () => {
    vi.spyOn(apiKeysApi, 'list').mockResolvedValue([])
    const created = { ...key, id: 'created-owned', key_prefix: 'sk_created', key: 'sk_created_secret' }
    vi.spyOn(apiKeysApi, 'create').mockResolvedValue(created)
    const { client, wrapper } = setup()
    const { result } = renderHook(() => useApiKeyModalViewModel({ open: true, onOpenChange: vi.fn() }), { wrapper })
    act(() => result.current.setNewKeyName('created-owned'))
    act(() => result.current.handleCreate())
    await waitFor(() => expect(result.current.createdKey).toBe(created.key))
    expect(client.getQueryData(queryKeys.apiKeys())).toEqual([expect.objectContaining({ id: created.id, created_by: auth.id })])
    expect(JSON.stringify(client.getQueryData(queryKeys.apiKeys()))).not.toContain(created.key)
    expect(result.current.apiKeys).toEqual([expect.objectContaining({ id: created.id, canDelete: true })])
  })

  it('blocks unassigned member key acquisition and issuance even through direct commands', async () => {
    Object.assign(auth, { role: 'viewer', organization_id: null, scope: 'unassigned' })
    const list = vi.spyOn(apiKeysApi, 'list').mockResolvedValue([])
    const create = vi.spyOn(apiKeysApi, 'create').mockResolvedValue({ ...key, key_prefix: 'sk_fixture', key: 'sk_should_not_issue' })
    const { wrapper } = setup()
    const { result } = renderHook(() => useApiKeyModalViewModel({ open: true, onOpenChange: vi.fn() }), { wrapper })
    await act(async () => {
      result.current.handleRefresh()
      result.current.handleOpenCreateDialog()
      result.current.setNewKeyName('unassigned')
    })
    expect(result.current.canCreate).toBe(false)
    expect(result.current.createOpen).toBe(false)
    act(() => result.current.handleCreate())
    expect(list).not.toHaveBeenCalled()
    expect(create).not.toHaveBeenCalled()
  })

  it('allows global key reads while blocking self issuance without membership', async () => {
    Object.assign(auth, { organization_id: null, scope: 'global' })
    const list = vi.spyOn(apiKeysApi, 'list').mockResolvedValue([key])
    const create = vi.spyOn(apiKeysApi, 'create').mockResolvedValue({ ...key, key_prefix: 'sk_fixture', key: 'sk_should_not_issue' })
    const { wrapper } = setup()
    const { result } = renderHook(() => useApiKeyModalViewModel({ open: true, onOpenChange: vi.fn() }), { wrapper })
    await waitFor(() => expect(result.current.apiKeys).toHaveLength(1))
    expect(list).toHaveBeenCalledOnce()
    act(() => result.current.setNewKeyName('global-unassigned-self'))
    expect(result.current.canCreate).toBe(false)
    act(() => result.current.handleCreate())
    expect(create).not.toHaveBeenCalled()
  })

  it('never offers global automation permissions on new organization keys', () => {
    const { wrapper } = setup()
    const { result } = renderHook(() => useApiKeyModalViewModel({ open: false, onOpenChange: vi.fn() }), { wrapper })
    expect(result.current.permissionOptions).toEqual(['openai.inference', 'openai.models.read'])
    act(() => result.current.handleTogglePermission('users.manage', true))
    expect(result.current.selectedPermissions).not.toContain('users.manage')
  })
  it('gates all acquisition by open without subscribing management caches to unrelated WS resources', async () => {
    const keys = vi.spyOn(apiKeysApi, 'list').mockResolvedValue([key])
    const users = vi.spyOn(usersApi, 'list').mockResolvedValue([user])
    const registrations = vi.spyOn(registrationRequestsApi, 'list').mockResolvedValue([registration])
    const { client, wrapper } = setup()
    const { result, rerender } = renderHook(({ open }) => ({
      keys: useApiKeyModalViewModel({ open, onOpenChange: vi.fn() }),
      users: useUserModalViewModel({ open, onOpenChange: vi.fn() }),
    }), { wrapper, initialProps: { open: false } })
    expect(keys).not.toHaveBeenCalled()
    expect(users).not.toHaveBeenCalled()
    expect(registrations).not.toHaveBeenCalled()
    rerender({ open: true })
    await waitFor(() => expect(result.current.users.registrations.requests).toHaveLength(1))
    expect(keys).toHaveBeenCalledTimes(1)
    expect(users).toHaveBeenCalledTimes(1)
    expect(registrations).toHaveBeenCalledTimes(1)
    const invalidate = vi.spyOn(client, 'invalidateQueries')
    for (const changed of ['endpoints', 'metrics', 'tps', 'system'] as const) {
      await act(() => invalidateDashboardSubscriptions(client, { changed, id: 'A' }))
    }
    expect(invalidate).not.toHaveBeenCalled()
  })

  it('inherits provider polling only for users/registrations and stops when closed; API keys ignore focus', async () => {
    vi.useFakeTimers()
    const keys = vi.spyOn(apiKeysApi, 'list').mockResolvedValue([])
    const users = vi.spyOn(usersApi, 'list').mockResolvedValue([])
    const registrations = vi.spyOn(registrationRequestsApi, 'list').mockResolvedValue([])
    const { wrapper } = setup(true)
    const { rerender, unmount } = renderHook(({ open }) => {
      useApiKeyModalViewModel({ open, onOpenChange: vi.fn() })
      useUserModalViewModel({ open, onOpenChange: vi.fn() })
    }, { wrapper, initialProps: { open: true } })
    await act(() => vi.advanceTimersByTimeAsync(1))
    await act(() => vi.advanceTimersByTimeAsync(10000))
    expect(keys).toHaveBeenCalledTimes(1)
    expect(users).toHaveBeenCalledTimes(3)
    expect(registrations).toHaveBeenCalledTimes(3)
    await act(async () => {
      focusManager.setFocused(false)
      focusManager.setFocused(true)
    })
    expect(keys).toHaveBeenCalledTimes(1)
    rerender({ open: false })
    const counts = [users.mock.calls.length, registrations.mock.calls.length]
    await act(() => vi.advanceTimersByTimeAsync(10000))
    expect([users.mock.calls.length, registrations.mock.calls.length]).toEqual(counts)
    unmount()
  })

  it('keeps an admin-created plaintext key out of cache, deduplicates the list and clears it on refresh/close', async () => {
    const list = vi.spyOn(apiKeysApi, 'list').mockResolvedValue([key])
    const response = { ...key, name: 'new', key_prefix: 'sk_new', key: 'sk_plaintext_once' }
    const create = vi.spyOn(apiKeysApi, 'create').mockResolvedValue(response)
    const onOpenChange = vi.fn()
    const { client, wrapper } = setup()
    const { result } = renderHook(() => useApiKeyModalViewModel({ open: true, onOpenChange }), { wrapper })
    await waitFor(() => expect(result.current.apiKeys).toHaveLength(1))
    act(() => {
      result.current.setNewKeyName('new')
      result.current.handleTogglePermission('users.manage', true)
    })
    act(() => result.current.handleCreate())
    await waitFor(() => expect(result.current.createdKey).toBe(response.key))
    expect(create).toHaveBeenCalledExactlyOnceWith({ name: 'new', expires_at: undefined,
      permissions: ['openai.inference', 'openai.models.read'] })
    expect(list).toHaveBeenCalledTimes(1)
    expect(client.getQueryData(queryKeys.apiKeys())).toEqual([{ ...key, name: 'new', created_by: auth.id, key_prefix: 'sk_new', expires_at: undefined }])
    expect(JSON.stringify(client.getQueryData(queryKeys.apiKeys()))).not.toContain(response.key)
    expect(result.current.newKeyName).toBe('')
    act(() => result.current.toggleCreatedKey())
    expect(result.current.showKey).toBe('created')
    act(() => result.current.handleRefresh())
    expect(result.current.createdKey).toBeNull()
    expect(result.current.showKey).toBeNull()
    await waitFor(() => expect(list).toHaveBeenCalledTimes(2))
    act(() => result.current.handleCreate())
    await waitFor(() => expect(result.current.createdKey).toBe(response.key))
    act(() => result.current.handleMainOpenChange(false))
    expect(result.current.createdKey).toBeNull()
    expect(onOpenChange).toHaveBeenCalledExactlyOnceWith(false)
  })

  it('preserves viewer fixed permissions and form reset while leaving errors visible', async () => {
    auth.role = 'viewer'
    auth.scope = 'account'
    const create = vi.spyOn(apiKeysApi, 'create').mockRejectedValue(new Error('permission denied'))
    const { wrapper } = setup()
    const { result } = renderHook(() => useApiKeyModalViewModel({ open: false, onOpenChange: vi.fn() }), { wrapper })
    act(() => {
      result.current.setNewKeyName('viewer key')
      result.current.handleTogglePermission('users.manage', true)
    })
    expect(result.current.selectedPermissions).toEqual(['openai.inference', 'openai.models.read'])
    act(() => result.current.handleCreate())
    await waitFor(() => expect(toast).toHaveBeenCalledWith({ title: 'Failed to create API key',
      description: 'permission denied', variant: 'destructive' }))
    expect(create).toHaveBeenCalledExactlyOnceWith({ name: 'viewer key', expires_at: undefined })
    expect(result.current.newKeyName).toBe('viewer key')
    act(() => result.current.handleCreateOpenChange(false))
    expect(result.current.newKeyName).toBe('')
  })

  it('deletes keys and starts the list refresh without extending mutation pending', async () => {
    const remove = vi.spyOn(apiKeysApi, 'delete').mockResolvedValue(undefined)
    const { client, wrapper } = setup()
    const invalidate = vi.spyOn(client, 'invalidateQueries').mockReturnValue(new Promise<void>(() => {}))
    const { result } = renderHook(() => useApiKeyModalViewModel({ open: false, onOpenChange: vi.fn() }), { wrapper })
    act(() => result.current.setDeleteKey({ ...key, created_by: auth.id }))
    act(() => result.current.handleDelete())
    await waitFor(() => expect(toast).toHaveBeenCalledWith({ title: 'API key deleted' }))
    expect(remove).toHaveBeenCalledExactlyOnceWith(key.id)
    expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: queryKeys.apiKeys() })
    expect(result.current.deleteKey).toBeNull()
    expect(result.current.isDeleting).toBe(false)
  })

  it('creates a user, refreshes both recipients and users immediately, and clears the generated password on dismissal', async () => {
    const create = vi.spyOn(usersApi, 'create').mockResolvedValue({ user, generated_password: 'one-time-password' })
    const { client, wrapper } = setup()
    const invalidate = vi.spyOn(client, 'invalidateQueries').mockReturnValue(new Promise<void>(() => {}))
    const { result } = renderHook(() => useUserModalViewModel({ open: false, onOpenChange: vi.fn() }), { wrapper })
    act(() => {
      result.current.handleOpenCreateDialog()
      result.current.setFormUsername('alice')
      result.current.setFormEmail('  alice@example.com  ')
    })
    act(() => result.current.handleCreate())
    await waitFor(() => expect(result.current.generatedPassword).toBe('one-time-password'))
    expect(create).toHaveBeenCalledExactlyOnceWith({ username: 'alice', role: 'viewer', email: 'alice@example.com' })
    expect(invalidate.mock.calls).toEqual([
      [{ queryKey: queryKeys.users() }], [{ queryKey: queryKeys.notificationSettings() }],
    ])
    expect(result.current.isCreating).toBe(false)
    expect(result.current.createOpen).toBe(false)
    expect(result.current.formEmail).toBe('')
    act(() => result.current.handleClosePassword())
    expect(result.current.generatedPassword).toBeNull()
    expect(result.current.copied).toBe(false)
  })

  it('retains user edits and reports the original update error without invalidating cached lists', async () => {
    vi.spyOn(usersApi, 'update').mockRejectedValue(new Error('last admin'))
    const { client, wrapper } = setup()
    const invalidate = vi.spyOn(client, 'invalidateQueries')
    const { result } = renderHook(() => useUserModalViewModel({ open: false, onOpenChange: vi.fn() }), { wrapper })
    act(() => result.current.handleOpenEditDialog(user))
    act(() => result.current.setFormEmail('changed@example.com'))
    act(() => result.current.handleUpdate())
    await waitFor(() => expect(toast).toHaveBeenCalledWith({ title: 'Failed to update user',
      description: 'last admin', variant: 'destructive' }))
    expect(result.current.editUser).toEqual(user)
    expect(result.current.formEmail).toBe('changed@example.com')
    expect(invalidate).not.toHaveBeenCalled()
  })

  it('does not acquire or mutate users for a viewer even when opened directly', async () => {
    auth.role = 'viewer'
    const list = vi.spyOn(usersApi, 'list').mockResolvedValue([user])
    const create = vi.spyOn(usersApi, 'create')
    const { wrapper } = setup()
    const { result } = renderHook(() => useUserModalViewModel({ open: true, onOpenChange: vi.fn() }), { wrapper })
    await act(async () => {})
    expect(list).not.toHaveBeenCalled()
    act(() => { result.current.setFormUsername('new@example.com') })
    act(() => result.current.handleCreate())
    expect(create).not.toHaveBeenCalled()
  })

})
