import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { notificationsApi, registrationRequestsApi } from '@/lib/api'
import { adminUser, deferred, renderWithProviders, viewerUser } from '@/test/render'
import { Header } from './Header'

beforeEach(() => {
  vi.spyOn(registrationRequestsApi, 'list').mockResolvedValue([])
})

describe('Header user menu', () => {
  it('opens the notification settings from the admin menu', async () => {
    // Kept pending: this test is about the menu entry, not the dialog content.
    const get = vi.spyOn(notificationsApi, 'get').mockReturnValue(deferred<never>().promise)
    const user = userEvent.setup()
    renderWithProviders(<Header user={adminUser} />)

    await user.click(screen.getByRole('button', { name: 'User menu' }))
    await user.click(await screen.findByRole('menuitem', { name: 'Notifications' }))

    expect(await screen.findByRole('dialog', { name: 'Notifications' })).toBeInTheDocument()
    expect(get).toHaveBeenCalledOnce()
  })

  it('does not offer the notification settings to a viewer', async () => {
    const get = vi.spyOn(notificationsApi, 'get')
    const user = userEvent.setup()
    renderWithProviders(<Header user={viewerUser} />, { user: viewerUser })

    await user.click(screen.getByRole('button', { name: 'User menu' }))

    expect(await screen.findByRole('menuitem', { name: 'Sign out' })).toBeInTheDocument()
    expect(screen.queryByRole('menuitem', { name: 'Notifications' })).not.toBeInTheDocument()
    expect(get).not.toHaveBeenCalled()
  })
})


it('shows an administrator the pending count next to Manage Users', async () => {
  vi.mocked(registrationRequestsApi.list).mockResolvedValue([
    { id: 'request-one', username: 'one@example.com', created_at: '2026-10-01T00:00:00Z' },
  ])
  const user = userEvent.setup()
  renderWithProviders(<Header user={adminUser} />)
  await user.click(screen.getByRole('button', { name: 'User menu' }))
  expect(await screen.findByTestId('pending-registration-count')).toHaveTextContent('1')
  expect(screen.queryByRole('menuitem', { name: 'Invitation Codes' })).not.toBeInTheDocument()
})
