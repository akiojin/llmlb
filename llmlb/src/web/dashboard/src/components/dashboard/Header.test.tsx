import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { notificationsApi } from '@/lib/api'
import { adminUser, deferred, renderWithProviders, viewerUser } from '@/test/render'
import { Header } from './Header'

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
