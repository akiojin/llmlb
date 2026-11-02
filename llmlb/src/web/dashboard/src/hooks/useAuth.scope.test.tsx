import type { ReactNode } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { authApi } from '@/lib/api/auth'
import { fetchWithAuth } from '@/lib/api/client'
import { deferred } from '@/test/render'
import { AuthProvider, useAuth } from './useAuth'

type ScopedUser = Awaited<ReturnType<typeof authApi.me>> & {
  organization_id: string | null
  organization_role: 'admin' | 'member' | null
  disabled: boolean
  scope: string
}

const alice: ScopedUser = {
  user_id: 'alice', username: 'alice', role: 'viewer', must_change_password: false,
  organization_id: 'org-A', organization_role: 'member', disabled: false,
  scope: 'self:org-A:alice',
}
const movedAlice: ScopedUser = {
  ...alice, organization_id: 'org-B', scope: 'self:org-B:alice',
}
const bob: ScopedUser = {
  ...alice, user_id: 'bob', username: 'bob', organization_id: 'org-B',
  scope: 'self:org-B:bob',
}

function setup() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>
      <AuthProvider>{children}</AuthProvider>
    </QueryClientProvider>
  )
  return { client, ...renderHook(() => useAuth(), { wrapper }) }
}

afterEach(() => {
  document.cookie = 'llmlb_csrf=; Max-Age=0; path=/'
})

describe('authentication scope isolation', () => {
  it('publishes the current membership and scope returned by the server', async () => {
    vi.spyOn(authApi, 'me').mockResolvedValue(alice)
    const { result } = setup()
    await waitFor(() => expect(result.current.isLoggedIn).toBe(true))
    expect(result.current.user).toMatchObject({
      id: alice.user_id, organization_id: alice.organization_id,
      organization_role: 'member', disabled: false, scope: alice.scope,
    })
  })

  it('cancels old private queries and clears their data when the same account moves organizations', async () => {
    const me = vi.spyOn(authApi, 'me').mockResolvedValue(alice)
    const { client, result } = setup()
    await waitFor(() => expect(result.current.user?.id).toBe('alice'))
    client.setQueryData(['private-totals'], { organization: 'org-A', requests: 45 })
    let requestSignal: AbortSignal | undefined
    const response = deferred<{ secret: string }>()
    const pending = client.fetchQuery({
      queryKey: ['private-page'],
      queryFn: ({ signal }) => { requestSignal = signal; return response.promise },
    }).catch(() => undefined)
    await waitFor(() => expect(requestSignal).toBeDefined())
    me.mockResolvedValue(movedAlice)
    await act(async () => result.current.checkAuth())
    expect(requestSignal?.aborted).toBe(true)
    expect(client.getQueryData(['private-totals'])).toBeUndefined()
    response.resolve({ secret: 'org-A-only' })
    await pending
    expect(client.getQueryData(['private-page'])).toBeUndefined()
  })

  it('also clears private caches when an account loses its organization role', async () => {
    const me = vi.spyOn(authApi, 'me').mockResolvedValue({ ...alice, organization_role: 'admin', scope: 'organization:org-A' })
    const { client, result } = setup()
    await waitFor(() => expect(result.current.user?.id).toBe('alice'))
    client.setQueryData(['organization-members'], [{ id: 'other-account' }])
    me.mockResolvedValue(alice)
    await act(async () => result.current.checkAuth())
    expect(client.getQueryData(['organization-members'])).toBeUndefined()
  })

  it('keeps a later session when an older me request finishes after it', async () => {
    const oldMe = deferred<ScopedUser>()
    const me = vi.spyOn(authApi, 'me').mockResolvedValueOnce(alice).mockReturnValueOnce(oldMe.promise).mockResolvedValue(bob)
    const { result } = setup()
    await waitFor(() => expect(result.current.user?.id).toBe('alice'))
    let oldCheck!: Promise<void>
    act(() => { oldCheck = result.current.checkAuth() })
    expect(me).toHaveBeenCalledTimes(2)
    await act(async () => result.current.checkAuth())
    expect(result.current.user?.id).toBe('bob')
    await act(async () => { oldMe.resolve(alice); await oldCheck })
    expect(result.current.user?.id).toBe('bob')
  })

  it('drops secrets immediately on logout while the server response is pending', async () => {
    vi.spyOn(authApi, 'me').mockResolvedValue(alice)
    const logout = deferred<void>()
    vi.spyOn(authApi, 'logout').mockReturnValue(logout.promise)
    const { client, result } = setup()
    await waitFor(() => expect(result.current.user?.id).toBe('alice'))
    client.setQueryData(['api-keys'], [{ id: 'private-key' }])
    act(() => { void result.current.logout() })
    expect(result.current.user).toBeNull()
    expect(result.current.isLoggedIn).toBe(false)
    expect(client.getQueryData(['api-keys'])).toBeUndefined()
  })

  it('does not resurrect a logged-out account from an older pending me result', async () => {
    const oldMe = deferred<ScopedUser>()
    vi.spyOn(authApi, 'me').mockResolvedValueOnce(alice).mockReturnValueOnce(oldMe.promise)
    vi.spyOn(authApi, 'logout').mockReturnValue(new Promise<void>(() => {}))
    const { result } = setup()
    await waitFor(() => expect(result.current.user?.id).toBe('alice'))
    let oldCheck!: Promise<void>
    act(() => { oldCheck = result.current.checkAuth() })
    act(() => { void result.current.logout() })
    await act(async () => { oldMe.resolve(alice); await oldCheck })
    expect(result.current.user).toBeNull()
  })
})

describe('API response session boundary', () => {
  it('rejects an old-scope mutation response before a consumer receives it', async () => {
    const me = vi.spyOn(authApi, 'me').mockResolvedValue(alice)
    const { result } = setup()
    await waitFor(() => expect(result.current.user?.id).toBe('alice'))
    const response = deferred<Response>()
    vi.stubGlobal('fetch', vi.fn<typeof fetch>().mockReturnValue(response.promise))
    const outcome = fetchWithAuth('/api/me/api-keys', { method: 'POST', body: '{"name":"key-A"}' })
      .then((value) => ({ value, error: null }), (error: unknown) => ({ value: null, error }))
    me.mockResolvedValue(bob)
    await act(async () => result.current.checkAuth())
    response.resolve(new Response('{"key":"secret-A"}', { headers: { 'X-LLMLB-Scope': alice.scope } }))
    const settled = await outcome
    expect(settled.value).toBeNull()
    expect(settled.error).toBeInstanceOf(Error)
    expect((settled.error as Error).message).toMatch(/scope|session/i)
  })

  it('rechecks the session after an asynchronously read response body', async () => {
    const me = vi.spyOn(authApi, 'me').mockResolvedValue(alice)
    const { result } = setup()
    await waitFor(() => expect(result.current.user?.id).toBe('alice'))
    const body = deferred<string>()
    const response = new Response(null, { headers: { 'X-LLMLB-Scope': alice.scope } })
    const readBody = vi.spyOn(response, 'text').mockReturnValue(body.promise)
    vi.stubGlobal('fetch', vi.fn<typeof fetch>().mockResolvedValue(response))
    const outcome = fetchWithAuth('/api/me/api-keys', { method: 'POST', body: '{"name":"key-A"}' })
      .then((value) => ({ value, error: null }), (error: unknown) => ({ value: null, error }))
    await waitFor(() => expect(readBody).toHaveBeenCalledOnce())
    me.mockResolvedValue(bob)
    await act(async () => result.current.checkAuth())
    body.resolve('{"key":"secret-A"}')
    const settled = await outcome
    expect(settled.value).toBeNull()
    expect(settled.error).toBeInstanceOf(Error)
  })

  it('rejects a changed server scope and removes cached data before it can render', async () => {
    vi.spyOn(authApi, 'me').mockResolvedValue(alice)
    const { client, result } = setup()
    await waitFor(() => expect(result.current.user?.id).toBe('alice'))
    client.setQueryData(['organization-members'], [{ id: 'org-A-member' }])
    vi.stubGlobal('fetch', vi.fn<typeof fetch>().mockResolvedValue(new Response('{"records":["org-B-only"]}', {
      headers: { 'X-LLMLB-Scope': movedAlice.scope },
    })))
    await expect(fetchWithAuth('/api/dashboard/request-responses')).rejects.toThrow(/scope|session/i)
    expect(client.getQueryData(['organization-members'])).toBeUndefined()
  })

  it('keeps CSRF and authenticated no-store fetch behavior for the current scope', async () => {
    vi.spyOn(authApi, 'me').mockResolvedValue(alice)
    const { result } = setup()
    await waitFor(() => expect(result.current.user?.id).toBe('alice'))
    document.cookie = 'llmlb_csrf=csrf-value; path=/'
    const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(new Response('{"id":"key-A"}', {
      headers: { 'X-LLMLB-Scope': alice.scope },
    }))
    vi.stubGlobal('fetch', fetch)
    await expect(fetchWithAuth('/api/me/api-keys', { method: 'POST', body: '{"name":"key-A"}' }))
      .resolves.toEqual({ id: 'key-A' })
    expect(fetch).toHaveBeenCalledWith('/api/me/api-keys', expect.objectContaining({
      credentials: 'include', cache: 'no-store',
      headers: expect.objectContaining({ 'X-CSRF-Token': 'csrf-value' }),
    }))
  })

  it('discards old global caches on the first forbidden mutation after a role change', async () => {
    const me = vi.spyOn(authApi, 'me').mockResolvedValue({ ...alice, role: 'admin', scope: 'global' })
    const { client, result } = setup()
    await waitFor(() => expect(result.current.user?.role).toBe('admin'))
    client.setQueryData(['global-account-list'], ['other-organization-secret'])
    me.mockResolvedValue(alice)
    vi.stubGlobal('fetch', vi.fn<typeof fetch>().mockResolvedValue(new Response('Admin access required', {
      status: 403, headers: { 'X-LLMLB-Scope': alice.scope, 'Cache-Control': 'no-store' },
    })))
    await expect(fetchWithAuth('/api/users', { method: 'POST', body: '{}' })).rejects.toThrow(/scope|session/i)
    expect(client.getQueryData(['global-account-list'])).toBeUndefined()
    await waitFor(() => expect(result.current.user?.scope).toBe(alice.scope))
  })

  it('retains permitted cached data when a forbidden operation leaves the scope unchanged', async () => {
    vi.spyOn(authApi, 'me').mockResolvedValue(alice)
    const { client, result } = setup()
    await waitFor(() => expect(result.current.user?.scope).toBe(alice.scope))
    client.setQueryData(['self-usage'], { request_count: 12 })
    vi.stubGlobal('fetch', vi.fn<typeof fetch>().mockResolvedValue(new Response('Admin access required', {
      status: 403, headers: { 'X-LLMLB-Scope': alice.scope, 'Cache-Control': 'no-store' },
    })))
    await expect(fetchWithAuth('/api/users', { method: 'POST', body: '{}' })).rejects.toThrow('Admin access required')
    expect(client.getQueryData(['self-usage'])).toEqual({ request_count: 12 })
    expect(result.current.user?.scope).toBe(alice.scope)
  })
})
