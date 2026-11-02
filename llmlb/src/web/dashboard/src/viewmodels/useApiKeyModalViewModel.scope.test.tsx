import type { ReactNode } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { MutationCache, QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { describe, expect, it, vi } from 'vitest'
import { AuthProvider } from '@/hooks/useAuth'
import { authApi } from '@/lib/api/auth'
import { apiKeysApi, type ApiKey } from '@/lib/api/api-keys'
import { setAuthSession } from '@/lib/authSession'
import { queryKeys } from '@/lib/queryKeys'
import { adminUser, deferred, type TestUser } from '@/test/render'
import { useApiKeyModalViewModel } from './useApiKeyModalViewModel'

const actor: TestUser = { ...adminUser, user_id: 'issuer-A', organization_id: 'org-A', organization_role: 'member', scope: 'global' }
const ownKey: ApiKey = { id: 'key-A', name: 'own', created_by: 'issuer-A', created_at: '2026-11-01T00:00:00Z', permissions: ['openai.inference'] }
const otherKey: ApiKey = { ...ownKey, id: 'key-B', name: 'same organization, other issuer', created_by: 'issuer-B' }
const issued = { ...ownKey, key: 'secret-A', key_prefix: 'sk_prefix' }

function setup(user = actor, mutationCache?: MutationCache) {
  vi.spyOn(authApi, 'me').mockResolvedValue(user)
  vi.spyOn(apiKeysApi, 'list').mockResolvedValue([ownKey, otherKey])
  const client = new QueryClient({ mutationCache, defaultOptions: { queries: { retry: false, gcTime: Infinity } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}><AuthProvider>{children}</AuthProvider></QueryClientProvider>
  )
  return { client, ...renderHook(() => useApiKeyModalViewModel({ open: true, onOpenChange: vi.fn() }), { wrapper }) }
}

describe('API key scope and one-time secret boundary', () => {
  it('creates only a current-account attributed inference key even for a system administrator', async () => {
    const create = vi.spyOn(apiKeysApi, 'create').mockResolvedValue(issued)
    const { result } = setup()
    await waitFor(() => expect(result.current.apiKeys).toHaveLength(2))
    act(() => result.current.handleOpenCreateDialog())
    expect(result.current.permissionOptions).not.toContain('users.manage')
    act(() => result.current.setNewKeyName('self inference'))
    act(() => result.current.handleCreate())
    await waitFor(() => expect(create).toHaveBeenCalledExactlyOnceWith({
      name: 'self inference', expires_at: undefined, usage_account_id: 'issuer-A', organization_id: 'org-A',
      permissions: ['openai.inference', 'openai.models.read'],
    }))
  })

  it('omits permissions for an assigned viewer and accepts the server fixed inference permissions', async () => {
    const create = vi.spyOn(apiKeysApi, 'create').mockResolvedValue({
      ...issued, permissions: ['openai.inference', 'openai.models.read'],
    })
    const { result } = setup({ ...actor, role: 'viewer', organization_role: 'member', scope: 'self:org-A:issuer-A' })
    await waitFor(() => expect(result.current.apiKeys).toHaveLength(2))
    act(() => result.current.handleOpenCreateDialog())
    act(() => result.current.setNewKeyName('viewer self inference'))
    act(() => result.current.handleCreate())
    await waitFor(() => expect(create).toHaveBeenCalledOnce())
    expect(create).toHaveBeenCalledExactlyOnceWith({
      name: 'viewer self inference', expires_at: undefined, usage_account_id: 'issuer-A', organization_id: 'org-A',
    })
    expect(create.mock.lastCall?.[0]).not.toHaveProperty('permissions')
    await waitFor(() => expect(result.current.createdKey).toBe('secret-A'))
    expect(result.current.apiKeys?.find((key) => key.id === issued.id)?.permissions).toEqual(['openai.inference', 'openai.models.read'])
  })

  it('does not acquire or issue keys for an unassigned viewer even when opened directly', async () => {
    const create = vi.spyOn(apiKeysApi, 'create')
    const { result } = setup({ ...actor, role: 'viewer', organization_id: null, organization_role: null, scope: 'unassigned' })
    await act(async () => {})
    expect(apiKeysApi.list).not.toHaveBeenCalled()
    act(() => { result.current.setNewKeyName('invalid'); result.current.handleCreate() })
    expect(create).not.toHaveBeenCalled()
    expect(result.current.canIssue).toBe(false)
  })

  it('keeps another issuer key read-only and denies a direct delete command', async () => {
    const remove = vi.spyOn(apiKeysApi, 'delete')
    const { result } = setup({ ...actor, role: 'viewer', organization_role: 'admin', scope: 'organization:org-A' })
    await waitFor(() => expect(result.current.apiKeys).toHaveLength(2))
    expect(result.current.apiKeys?.find((key) => key.id === 'key-A')?.canDelete).toBe(true)
    expect(result.current.apiKeys?.find((key) => key.id === 'key-B')?.canDelete).toBe(false)
    act(() => result.current.setDeleteKey(otherKey))
    act(() => result.current.handleDelete())
    expect(remove).not.toHaveBeenCalled()
  })

  it('rejects a delayed create after the dialog closes without retaining plaintext in mutation cache', async () => {
    const response = deferred<typeof issued>()
    vi.spyOn(apiKeysApi, 'create').mockReturnValue(response.promise)
    const { client, result } = setup()
    await waitFor(() => expect(result.current.apiKeys).toHaveLength(2))
    act(() => result.current.handleOpenCreateDialog())
    act(() => result.current.setNewKeyName('old'))
    act(() => result.current.handleCreate())
    await waitFor(() => expect(apiKeysApi.create).toHaveBeenCalledOnce())
    act(() => result.current.handleMainOpenChange(false))
    await act(async () => response.resolve(issued))
    expect(result.current.createdKey).toBeNull()
    expect(JSON.stringify(client.getMutationCache().getAll().map((mutation) => mutation.state.data))).not.toContain('secret-A')
  })

  it('does not send a key command invalidated while onMutate is waiting', async () => {
    const admission = deferred<void>()
    const create = vi.spyOn(apiKeysApi, 'create')
    const { result } = setup(actor, new MutationCache({ onMutate: () => admission.promise }))
    await waitFor(() => expect(result.current.apiKeys).toHaveLength(2))
    act(() => result.current.handleOpenCreateDialog())
    act(() => result.current.setNewKeyName('old'))
    act(() => result.current.handleCreate())
    act(() => result.current.handleMainOpenChange(false))
    await act(async () => admission.resolve())
    expect(create).not.toHaveBeenCalled()
  })

  it('does not repopulate a new session cache from an old deferred success callback', async () => {
    const callback = deferred<void>()
    vi.spyOn(apiKeysApi, 'create').mockResolvedValue(issued)
    const { client, result } = setup(actor, new MutationCache({ onSuccess: () => callback.promise }))
    await waitFor(() => expect(result.current.apiKeys).toHaveLength(2))
    act(() => result.current.handleOpenCreateDialog())
    act(() => result.current.setNewKeyName('old'))
    act(() => result.current.handleCreate())
    await waitFor(() => expect(apiKeysApi.create).toHaveBeenCalledOnce())
    setAuthSession('other-session', 'organization:org-B')
    client.clear()
    await act(async () => callback.resolve())
    expect(client.getQueryData(queryKeys.apiKeys())).toBeUndefined()
    expect(result.current.createdKey).toBeNull()
  })
})
