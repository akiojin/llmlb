import { screen, waitFor, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { usersApi, type User } from '@/lib/api'
import { renderWithProviders } from '@/test/render'
import { UserModal } from './UserModal'

const EMAIL_LABEL = 'Notification email (optional)'

const alice: User = {
  id: 'user-alice',
  username: 'alice',
  role: 'admin',
  email: 'alice@example.com',
  created_at: '2026-09-30T00:00:00Z',
}

const bob: User = {
  id: 'user-bob',
  username: 'bob',
  role: 'viewer',
  email: null,
  created_at: '2026-09-30T00:00:00Z',
}

function rowFor(username: string) {
  return screen.getByRole('cell', { name: username }).closest('tr') as HTMLElement
}

async function renderModal() {
  const view = renderWithProviders(<UserModal open onOpenChange={() => {}} />)
  await screen.findByRole('cell', { name: 'alice' })
  return view
}

async function openEditDialog(user: UserEvent, username: string) {
  await user.click(within(rowFor(username)).getByRole('button', { name: `Edit ${username}` }))
  return screen.findByRole('dialog', { name: 'Edit User' })
}

async function fill(user: UserEvent, field: HTMLElement, value: string) {
  await user.clear(field)
  if (value) {
    await user.click(field)
    await user.paste(value)
  }
}

beforeEach(() => {
  vi.spyOn(usersApi, 'list').mockResolvedValue([alice, bob])
})

describe('UserModal notification email', () => {
  it('lists the notification email of each user and marks users without one', async () => {
    await renderModal()

    expect(screen.getByRole('columnheader', { name: 'Notification email' })).toBeInTheDocument()
    expect(within(rowFor('alice')).getByText('alice@example.com')).toBeInTheDocument()
    expect(within(rowFor('bob')).getByText('Not set')).toBeInTheDocument()
  })

  it('states in the create dialog that the email is a notification destination, not a login identifier', async () => {
    const user = userEvent.setup()
    await renderModal()

    await user.click(screen.getByRole('button', { name: 'Add User' }))
    const dialog = await screen.findByRole('dialog', { name: 'Create User' })

    expect(within(dialog).getByLabelText(EMAIL_LABEL)).toHaveAccessibleDescription(
      'Destination for operational notifications, which are sent to administrators only. It is not a login identifier: users sign in with their username.',
    )
  })

  it('creates a user with the entered notification email', async () => {
    const create = vi.spyOn(usersApi, 'create').mockResolvedValue({
      user: { ...bob, id: 'user-carol', username: 'carol-login@example.com', email: 'carol@example.com' },
      generated_password: 'generated-password',
    })
    const user = userEvent.setup()
    await renderModal()

    await user.click(screen.getByRole('button', { name: 'Add User' }))
    const dialog = within(await screen.findByRole('dialog', { name: 'Create User' }))
    await fill(user, dialog.getByLabelText('Email'), 'carol-login@example.com')
    await fill(user, dialog.getByLabelText(EMAIL_LABEL), 'carol@example.com')
    await user.click(dialog.getByRole('button', { name: 'Create' }))

    await waitFor(() =>
      expect(create).toHaveBeenCalledExactlyOnceWith({
        username: 'carol-login@example.com',
        role: 'viewer',
        email: 'carol@example.com',
      }),
    )
  })

  it('creates a user without an email field when the email is left blank', async () => {
    const create = vi.spyOn(usersApi, 'create').mockResolvedValue({
      user: { ...bob, id: 'user-carol', username: 'carol-login@example.com' },
      generated_password: 'generated-password',
    })
    const user = userEvent.setup()
    await renderModal()

    await user.click(screen.getByRole('button', { name: 'Add User' }))
    const dialog = within(await screen.findByRole('dialog', { name: 'Create User' }))
    await fill(user, dialog.getByLabelText('Email'), 'carol-login@example.com')
    await user.click(dialog.getByRole('button', { name: 'Create' }))

    await waitFor(() =>
      expect(create).toHaveBeenCalledExactlyOnceWith({ username: 'carol-login@example.com', role: 'viewer' }),
    )
  })

  it('shows the current email in the edit dialog with the same explanation', async () => {
    const user = userEvent.setup()
    await renderModal()

    const dialog = within(await openEditDialog(user, 'alice'))

    const email = dialog.getByLabelText(EMAIL_LABEL)
    expect(email).toHaveValue('alice@example.com')
    expect(email).toHaveAccessibleDescription(/It is not a login identifier/)
  })

  it('sends only the email when only the email was changed', async () => {
    const update = vi
      .spyOn(usersApi, 'update')
      .mockResolvedValue({ ...bob, email: 'bob@example.com' })
    const user = userEvent.setup()
    await renderModal()

    const dialog = within(await openEditDialog(user, 'bob'))
    expect(dialog.getByLabelText(EMAIL_LABEL)).toHaveValue('')
    await fill(user, dialog.getByLabelText(EMAIL_LABEL), 'bob@example.com')
    await user.click(dialog.getByRole('button', { name: 'Update' }))

    await waitFor(() =>
      expect(update).toHaveBeenCalledExactlyOnceWith('user-bob', { email: 'bob@example.com' }),
    )
  })

  it('clears the email by sending an empty string', async () => {
    const update = vi.spyOn(usersApi, 'update').mockResolvedValue({ ...alice, email: null })
    const user = userEvent.setup()
    await renderModal()

    const dialog = within(await openEditDialog(user, 'alice'))
    await fill(user, dialog.getByLabelText(EMAIL_LABEL), '')
    await user.click(dialog.getByRole('button', { name: 'Update' }))

    await waitFor(() =>
      expect(update).toHaveBeenCalledExactlyOnceWith('user-alice', { email: '' }),
    )
  })

  it('leaves the email out of the update when it was not changed', async () => {
    const update = vi.spyOn(usersApi, 'update').mockResolvedValue({ ...alice, username: 'alicia@example.com' })
    const user = userEvent.setup()
    await renderModal()

    const dialog = within(await openEditDialog(user, 'alice'))
    await fill(user, dialog.getByLabelText('Email'), 'alicia@example.com')
    await user.click(dialog.getByRole('button', { name: 'Update' }))

    await waitFor(() =>
      expect(update).toHaveBeenCalledExactlyOnceWith('user-alice', { username: 'alicia@example.com' }),
    )
  })

  it('refreshes the notification recipients after a user was updated', async () => {
    vi.spyOn(usersApi, 'update').mockResolvedValue({ ...bob, email: 'bob@example.com' })
    const user = userEvent.setup()
    const { queryClient } = await renderModal()
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')

    const dialog = within(await openEditDialog(user, 'bob'))
    await fill(user, dialog.getByLabelText(EMAIL_LABEL), 'bob@example.com')
    await user.click(dialog.getByRole('button', { name: 'Update' }))

    expect(await screen.findByText('User updated')).toBeInTheDocument()
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['notification-settings'] })
  })

  it('refreshes the notification recipients after a user was deleted', async () => {
    const remove = vi.spyOn(usersApi, 'delete').mockResolvedValue(undefined)
    const user = userEvent.setup()
    const { queryClient } = await renderModal()
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')

    await user.click(within(rowFor('alice')).getByRole('button', { name: 'Delete alice' }))
    const dialog = within(await screen.findByRole('alertdialog', { name: 'Delete User' }))
    await user.click(dialog.getByRole('button', { name: 'Delete' }))

    expect(await screen.findByText('User deleted')).toBeInTheDocument()
    expect(remove).toHaveBeenCalledExactlyOnceWith('user-alice')
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['notification-settings'] })
  })
})
