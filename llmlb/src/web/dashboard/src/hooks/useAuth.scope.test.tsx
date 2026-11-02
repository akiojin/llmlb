import { act, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { useAuth } from './useAuth'
import { authApi } from '@/lib/api/auth'
import { fetchWithAuth, setExpectedUsageScope } from '@/lib/api/client'
import { adminUser, deferred, renderPage, viewerUser } from '@/test/render'

function Session() {
  const { user, checkAuth, logout } = useAuth()
  return <><span>{user?.username}</span><span>{user?.organization_id}</span><button onClick={() => void checkAuth()}>Refresh session</button>
    <button onClick={() => void logout()}>Sign out</button></>
}

describe('authentication cache boundary', () => {
  it('rejects a response from a previous account even when both accounts have global scope', async () => {
    const response = deferred<Response>()
    vi.stubGlobal('fetch', vi.fn(() => response.promise))
    setExpectedUsageScope('global', 'previous-administrator')
    try {
      const pending = fetchWithAuth('/api/me/api-keys', { method: 'POST' })
      setExpectedUsageScope('global', 'current-administrator')
      response.resolve(new Response(JSON.stringify({ id: 'previous-account-key' })))
      await expect(pending).rejects.toMatchObject({ status: 409 })
    } finally {
      setExpectedUsageScope(null)
    }
  })
  it('rejects an old-scope body that finishes after its response headers were accepted', async () => {
    let controller!: ReadableStreamDefaultController<Uint8Array>
    const response = new Response(new ReadableStream<Uint8Array>({
      start(next) { controller = next },
    }))
    vi.stubGlobal('fetch', vi.fn(async () => response))
    setExpectedUsageScope('account:organization-a:old-account')
    try {
      const pending = fetchWithAuth('/api/me/api-keys', { method: 'POST' })
      await waitFor(() => expect(response.bodyUsed).toBe(true))
      setExpectedUsageScope('account:organization-b:current-account')
      controller.enqueue(new TextEncoder().encode(JSON.stringify({ id: 'old-key', organization_id: 'organization-a' })))
      controller.close()
      await expect(pending).rejects.toMatchObject({ status: 409 })
    } finally {
      setExpectedUsageScope(null)
    }
  })
  it('rejects a mutation that finishes after the session scope changes even without a response scope header', async () => {
    const response = deferred<Response>()
    vi.stubGlobal('fetch', vi.fn(() => response.promise))
    setExpectedUsageScope('account:organization-a:old-account')
    try {
      const pending = fetchWithAuth('/api/me/api-keys', {
        method: 'POST', body: JSON.stringify({ name: 'delayed key' }),
      })
      setExpectedUsageScope('account:organization-b:current-account')
      response.resolve(new Response(JSON.stringify({ id: 'old-key', organization_id: 'organization-a' })))
      await expect(pending).rejects.toMatchObject({ status: 409 })
    } finally {
      setExpectedUsageScope(null)
    }
  })
  it('rejects a data response from a different server scope before it enters a cache', async () => {
    const oldUser = { ...viewerUser, organization_id: 'organization-a', organization_role: 'admin' as const, scope: 'organization' as const }
    const { queryClient } = renderPage(<Session />, { user: oldUser })
    await screen.findByText('organization-a')
    queryClient.setQueryData(['private-usage'], { private: 'previous organization' })
    vi.mocked(authApi.me).mockResolvedValue({ ...oldUser, organization_id: 'organization-b' })
    vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({ private: 'new organization' }), {
      headers: { 'x-llmlb-usage-scope': 'organization:organization-b' },
    })))
    await act(async () => {
      await expect(fetchWithAuth('/api/dashboard/usage')).rejects.toMatchObject({ status: 409 })
    })
    expect(queryClient.getQueryData(['private-usage'])).toBeUndefined()
    expect(await screen.findByText('organization-b')).toBeVisible()
  })
  it('hides and clears previous-organization data while rechecking a changed server scope', async () => {
    const oldUser = { ...viewerUser, organization_id: 'organization-a', organization_role: 'admin' as const, scope: 'organization' as const }
    const { queryClient } = renderPage(<Session />, { user: oldUser })
    await screen.findByText('organization-a')
    queryClient.setQueryData(['private-usage'], { organization: 'organization-a', private: 'previous data' })
    const response = deferred<typeof oldUser>()
    vi.mocked(authApi.me).mockReturnValueOnce(response.promise)
    await act(async () => { window.dispatchEvent(new Event('llmlb:auth-scope-invalidated')) })
    expect(queryClient.getQueryData(['private-usage'])).toBeUndefined()
    expect(screen.queryByText('organization-a')).not.toBeInTheDocument()
    await act(async () => response.resolve({ ...oldUser, organization_id: 'organization-b' }))
    expect(await screen.findByText('organization-b')).toBeVisible()
  })
  it('removes previous-account data before publishing the next session', async () => {
    const { queryClient } = renderPage(<Session />)
    await screen.findByText('admin')
    queryClient.setQueryData(['request-history'], { secret: 'organization A record' })
    vi.mocked(authApi.me).mockResolvedValue(viewerUser)
    await act(async () => screen.getByRole('button', { name: 'Refresh session' }).click())
    await screen.findByText('viewer')
    expect(queryClient.getQueryData(['request-history'])).toBeUndefined()
  })

  it('clears cached private data when logout finishes', async () => {
    vi.spyOn(authApi, 'logout').mockResolvedValue(undefined)
    const { queryClient } = renderPage(<Session />, { user: adminUser })
    await screen.findByText('admin')
    queryClient.setQueryData(['usage'], { account: 'previous user' })
    await act(async () => screen.getByRole('button', { name: 'Sign out' }).click())
    await waitFor(() => expect(authApi.logout).toHaveBeenCalled())
    expect(queryClient.getQueryData(['usage'])).toBeUndefined()
  })

  it('does not restore a session from a refresh that finishes after logout', async () => {
    vi.spyOn(authApi, 'logout').mockResolvedValue(undefined)
    renderPage(<Session />)
    await screen.findByText('admin')
    const oldSession = deferred<typeof adminUser>()
    vi.mocked(authApi.me).mockReturnValueOnce(oldSession.promise)
    await act(async () => screen.getByRole('button', { name: 'Refresh session' }).click())
    await act(async () => screen.getByRole('button', { name: 'Sign out' }).click())
    await act(async () => oldSession.resolve(adminUser))
    expect(screen.queryByText('admin')).not.toBeInTheDocument()
  })
})
