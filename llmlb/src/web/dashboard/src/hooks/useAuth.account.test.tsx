import { useState, type ReactNode } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { describe, expect, it, vi } from 'vitest'
import { authApi, type CurrentUser } from '@/lib/api/auth'
import { fetchWithAuth } from '@/lib/api/client'
import { deferred } from '@/test/render'
import { AuthProvider, useAuth } from './useAuth'

function identities(scope: 'global' | 'organization'): [CurrentUser, CurrentUser] {
  const shared = {
    role: scope === 'global' ? 'admin' : 'viewer', must_change_password: false, disabled: false,
    organization_id: scope === 'global' ? null : 'org-A',
    organization_role: scope === 'global' ? null : 'admin' as const,
    scope: scope === 'global' ? 'global' : 'organization:org-A',
  }
  return [
    { ...shared, user_id: 'account-A', username: 'actor-A' },
    { ...shared, user_id: 'account-B', username: 'actor-B' },
  ]
}

function setup(first: CurrentUser, next: CurrentUser = first) {
  const me = vi.spyOn(authApi, 'me').mockResolvedValueOnce(first).mockResolvedValue(next)
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}><AuthProvider>{children}</AuthProvider></QueryClientProvider>
  )
  const hook = renderHook(() => {
    const auth = useAuth()
    const [secret, setSecret] = useState<string | null>(null)
    return { auth, secret, setSecret }
  }, { wrapper })
  return { client, me, ...hook }
}

describe('server account identity within one authorization scope', () => {
  for (const scope of ['global', 'organization'] as const) {
    for (const status of [200, 403]) {
      it(`rejects another ${scope} actor at header receipt (status ${status}), clears secrets and rechecks me`, async () => {
        const [first, next] = identities(scope)
        const { client, me, result } = setup(first, next)
        await waitFor(() => expect(result.current.auth.user?.id).toBe(first.user_id))
        client.setQueryData(['private-accounts'], ['account-A-only'])
        act(() => result.current.setSecret('once-only-secret-A'))
        const response = new Response('{"private":"account-B-only"}', {
          status, headers: { 'X-LLMLB-Scope': first.scope!, 'X-LLMLB-Account': next.user_id, 'Cache-Control': 'no-store' },
        })
        const body = vi.spyOn(response, 'text')
        vi.stubGlobal('fetch', vi.fn<typeof fetch>().mockResolvedValue(response))
        await act(async () => {
          await expect(fetchWithAuth('/api/me/api-keys', { method: 'POST', body: '{}' })).rejects.toThrow(/scope|session/i)
        })
        expect(body).not.toHaveBeenCalled()
        expect(client.getQueryData(['private-accounts'])).toBeUndefined()
        expect(result.current.secret).toBeNull()
        await waitFor(() => expect(result.current.auth.user?.id).toBe(next.user_id))
        expect(me).toHaveBeenCalledTimes(2)
      })
    }

    it(`rejects a pending ${scope} response body when another response reveals a same-scope account switch`, async () => {
      const [first, next] = identities(scope)
      const { client, result } = setup(first, next)
      await waitFor(() => expect(result.current.auth.user?.id).toBe(first.user_id))
      client.setQueryData(['private-accounts'], ['account-A-only'])
      act(() => result.current.setSecret('once-only-secret-A'))
      const body = deferred<string>()
      const firstResponse = new Response(null, { headers: { 'X-LLMLB-Scope': first.scope!, 'X-LLMLB-Account': first.user_id } })
      const readBody = vi.spyOn(firstResponse, 'text').mockReturnValue(body.promise)
      const changedActorResponse = new Response('{"private":"account-B-only"}', {
        headers: { 'X-LLMLB-Scope': first.scope!, 'X-LLMLB-Account': next.user_id },
      })
      vi.stubGlobal('fetch', vi.fn<typeof fetch>().mockResolvedValueOnce(firstResponse).mockResolvedValueOnce(changedActorResponse))
      const pending = fetchWithAuth('/api/me/api-keys', { method: 'POST', body: '{}' })
        .then((value) => ({ value, error: null }), (error: unknown) => ({ value: null, error }))
      await waitFor(() => expect(readBody).toHaveBeenCalledOnce())
      await act(async () => {
        await expect(fetchWithAuth('/api/users')).rejects.toThrow(/scope|session/i)
      })
      await act(async () => body.resolve('{"key":"old-secret-A"}'))
      const settled = await pending
      expect(settled.value).toBeNull()
      expect(settled.error).toBeInstanceOf(Error)
      expect(client.getQueryData(['private-accounts'])).toBeUndefined()
      expect(result.current.secret).toBeNull()
      await waitFor(() => expect(result.current.auth.user?.id).toBe(next.user_id))
    })

    it(`retains the current ${scope} actor, cache and secret when the account header matches`, async () => {
      const [first] = identities(scope)
      const { client, me, result } = setup(first)
      await waitFor(() => expect(result.current.auth.user?.id).toBe(first.user_id))
      client.setQueryData(['private-accounts'], ['account-A-only'])
      act(() => result.current.setSecret('once-only-secret-A'))
      vi.stubGlobal('fetch', vi.fn<typeof fetch>().mockResolvedValue(new Response('{"id":"permitted"}', {
        headers: { 'X-LLMLB-Scope': first.scope!, 'X-LLMLB-Account': first.user_id },
      })))
      await expect(fetchWithAuth('/api/me/api-keys')).resolves.toEqual({ id: 'permitted' })
      expect(client.getQueryData(['private-accounts'])).toEqual(['account-A-only'])
      expect(result.current.secret).toBe('once-only-secret-A')
      expect(result.current.auth.user?.id).toBe(first.user_id)
      expect(me).toHaveBeenCalledOnce()
    })
  }

  it('lets a real me refresh establish the new same-scope actor instead of rejecting its account header', async () => {
    const [first, next] = identities('global')
    const realMe = authApi.me
    const { client, me, result } = setup(first)
    await waitFor(() => expect(result.current.auth.user?.id).toBe(first.user_id))
    client.setQueryData(['private-accounts'], ['account-A-only'])
    act(() => result.current.setSecret('once-only-secret-A'))
    me.mockImplementation(realMe)
    vi.stubGlobal('fetch', vi.fn<typeof fetch>().mockResolvedValue(new Response(JSON.stringify(next), {
      headers: { 'X-LLMLB-Scope': first.scope!, 'X-LLMLB-Account': next.user_id },
    })))
    await act(async () => result.current.auth.checkAuth())
    expect(result.current.auth.user?.id).toBe(next.user_id)
    expect(client.getQueryData(['private-accounts'])).toBeUndefined()
    expect(result.current.secret).toBeNull()
  })
})
