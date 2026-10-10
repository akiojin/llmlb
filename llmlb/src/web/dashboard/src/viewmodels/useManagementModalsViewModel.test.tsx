import type { ReactNode } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { focusManager, QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { apiKeysApi, invitationsApi, usersApi, type ApiKey, type Invitation, type User } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import * as utils from '@/lib/utils'
import { invalidateDashboardSubscriptions } from '@/hooks/dashboardSubscriptions'
import { useApiKeyModalViewModel } from './useApiKeyModalViewModel'
import { useUserModalViewModel } from './useUserModalViewModel'
import { useInvitationModalViewModel } from './useInvitationModalViewModel'

const { toast, auth } = vi.hoisted(() => ({
  toast: vi.fn(),
  auth: { role: 'admin' },
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
const invitation: Invitation = {
  id: 'invitation-A', created_by: 'admin', created_at: '2026-10-01T00:00:00Z',
  expires_at: '2099-10-01T00:00:00Z', status: 'active',
}

function setup(polling = false) {
  const client = new QueryClient({ defaultOptions: { queries: {
    retry: false, gcTime: Infinity,
    ...(polling ? { refetchInterval: 5000, refetchIntervalInBackground: true } : {}),
  } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
  return { client, wrapper }
}

afterEach(() => {
  vi.useRealTimers()
  focusManager.setFocused(undefined)
  auth.role = 'admin'
})

describe('management modal ViewModels', () => {
  it('gates all acquisition by open without subscribing management caches to unrelated WS resources', async () => {
    const keys = vi.spyOn(apiKeysApi, 'list').mockResolvedValue([key])
    const users = vi.spyOn(usersApi, 'list').mockResolvedValue([user])
    const invitations = vi.spyOn(invitationsApi, 'list').mockResolvedValue([invitation])
    const { client, wrapper } = setup()
    const { result, rerender } = renderHook(({ open }) => ({
      keys: useApiKeyModalViewModel({ open, onOpenChange: vi.fn() }),
      users: useUserModalViewModel({ open, onOpenChange: vi.fn() }),
      invitations: useInvitationModalViewModel({ open }),
    }), { wrapper, initialProps: { open: false } })
    expect(keys).not.toHaveBeenCalled()
    expect(users).not.toHaveBeenCalled()
    expect(invitations).not.toHaveBeenCalled()
    rerender({ open: true })
    await waitFor(() => expect(result.current.invitations.invitations).toHaveLength(1))
    expect(keys).toHaveBeenCalledTimes(1)
    expect(users).toHaveBeenCalledTimes(1)
    expect(invitations).toHaveBeenCalledTimes(1)
    const invalidate = vi.spyOn(client, 'invalidateQueries')
    for (const changed of ['endpoints', 'metrics', 'tps', 'system'] as const) {
      await act(() => invalidateDashboardSubscriptions(client, { changed, id: 'A' }))
    }
    expect(invalidate).not.toHaveBeenCalled()
  })

  it('inherits provider polling only for users/invitations and stops when closed; API keys ignore focus', async () => {
    vi.useFakeTimers()
    const keys = vi.spyOn(apiKeysApi, 'list').mockResolvedValue([])
    const users = vi.spyOn(usersApi, 'list').mockResolvedValue([])
    const invitations = vi.spyOn(invitationsApi, 'list').mockResolvedValue([])
    const { wrapper } = setup(true)
    const { rerender, unmount } = renderHook(({ open }) => {
      useApiKeyModalViewModel({ open, onOpenChange: vi.fn() })
      useUserModalViewModel({ open, onOpenChange: vi.fn() })
      useInvitationModalViewModel({ open })
    }, { wrapper, initialProps: { open: true } })
    await act(() => vi.advanceTimersByTimeAsync(1))
    await act(() => vi.advanceTimersByTimeAsync(10000))
    expect(keys).toHaveBeenCalledTimes(1)
    expect(users).toHaveBeenCalledTimes(3)
    expect(invitations).toHaveBeenCalledTimes(3)
    await act(async () => {
      focusManager.setFocused(false)
      focusManager.setFocused(true)
    })
    expect(keys).toHaveBeenCalledTimes(1)
    rerender({ open: false })
    const counts = [users.mock.calls.length, invitations.mock.calls.length]
    await act(() => vi.advanceTimersByTimeAsync(10000))
    expect([users.mock.calls.length, invitations.mock.calls.length]).toEqual(counts)
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
      permissions: ['openai.inference', 'openai.models.read', 'users.manage'] })
    expect(list).toHaveBeenCalledTimes(1)
    expect(client.getQueryData(queryKeys.apiKeys())).toEqual([{ ...key, name: 'new', key_prefix: 'sk_new', expires_at: undefined }])
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
    act(() => result.current.setDeleteKey(key))
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

  it('derives invitation expiry and revoke availability without changing status semantics', async () => {
    vi.spyOn(invitationsApi, 'list').mockResolvedValue([
      invitation,
      { ...invitation, id: 'old', expires_at: '2000-01-01T00:00:00Z' },
      { ...invitation, id: 'used', status: 'used', used_by: 'registered-user' },
      { ...invitation, id: 'revoked', status: 'revoked' },
    ])
    const { wrapper } = setup()
    const { result } = renderHook(() => useInvitationModalViewModel({ open: true }), { wrapper })
    await waitFor(() => expect(result.current.invitations).toHaveLength(4))
    expect(result.current.invitations?.map(({ displayStatus, canRevoke }) => ({ displayStatus, canRevoke }))).toEqual([
      { displayStatus: 'active', canRevoke: true }, { displayStatus: 'expired', canRevoke: false },
      { displayStatus: 'used', canRevoke: false }, { displayStatus: 'revoked', canRevoke: false },
    ])
    expect(result.current.invitations?.[2].usedByLabel).toBe('register...')
  })

  it('creates with the selected lifetime, revokes and refreshes invitations without waiting; closing clears the code', async () => {
    const response = { id: invitation.id, code: 'invite-once', created_at: invitation.created_at, expires_at: invitation.expires_at }
    const create = vi.spyOn(invitationsApi, 'create').mockResolvedValue(response)
    const revoke = vi.spyOn(invitationsApi, 'revoke').mockResolvedValue(undefined)
    const { client, wrapper } = setup()
    const invalidate = vi.spyOn(client, 'invalidateQueries').mockReturnValue(new Promise<void>(() => {}))
    const { result } = renderHook(() => useInvitationModalViewModel({ open: false }), { wrapper })
    expect(result.current.expiresInHours).toBe(72)
    act(() => { result.current.setCreateOpen(true); result.current.setExpiresInHours(24) })
    act(() => result.current.handleCreate())
    await waitFor(() => expect(result.current.createdCode).toEqual(response))
    expect(create).toHaveBeenCalledExactlyOnceWith(24)
    expect(result.current.isCreating).toBe(false)
    act(() => result.current.handleCloseCreatedDialog())
    expect(result.current.createdCode).toBeNull()
    expect(result.current.createOpen).toBe(false)
    expect(result.current.expiresInHours).toBe(24)
    act(() => result.current.setRevokeInvitation(invitation))
    act(() => result.current.handleRevoke())
    await waitFor(() => expect(toast).toHaveBeenCalledWith({ title: 'Invitation code revoked' }))
    expect(revoke).toHaveBeenCalledExactlyOnceWith(invitation.id)
    expect(invalidate).toHaveBeenCalledTimes(2)
    expect(invalidate).toHaveBeenLastCalledWith({ queryKey: queryKeys.invitations() })
    expect(result.current.revokeInvitation).toBeNull()
    expect(result.current.isRevoking).toBe(false)
  })

  it('keeps manual-copy fallback and two-second feedback in the ViewModel', async () => {
    vi.useFakeTimers()
    vi.spyOn(invitationsApi, 'create').mockResolvedValue({ id: invitation.id,
      code: 'invite-once', created_at: invitation.created_at, expires_at: invitation.expires_at })
    const copy = vi.spyOn(utils, 'copyToClipboard').mockResolvedValue({ method: 'manual' })
    const select = vi.spyOn(utils, 'selectTextForManualCopy').mockReturnValue(true)
    const { wrapper } = setup()
    const { result, unmount } = renderHook(() => useInvitationModalViewModel({ open: false }), { wrapper })
    act(() => result.current.handleCreate())
    await act(() => vi.advanceTimersByTimeAsync(1))
    await act(() => result.current.handleCopy())
    expect(select).toHaveBeenCalledExactlyOnceWith('invite-once')
    expect(result.current.copied).toBe(false)
    copy.mockResolvedValue({ method: 'clipboard' })
    await act(() => result.current.handleCopy())
    expect(result.current.copied).toBe(true)
    await act(() => vi.advanceTimersByTimeAsync(2000))
    expect(result.current.copied).toBe(false)
    unmount()
  })
})
