import { fireEvent, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { ApiError, authApi } from '@/lib/api'
import { renderWithProviders } from '@/test/render'
import RegisterPage from './Register'

describe('registration request', () => {
  it('accepts email and password without an invitation and explains administrator approval', async () => {
    const register = vi.spyOn(authApi, 'register').mockResolvedValue({ status: 'pending', message: 'Awaiting administrator approval.' })
    const login = vi.spyOn(authApi, 'login')
    const user = userEvent.setup()
    renderWithProviders(<RegisterPage />)

    expect(screen.queryByLabelText('Invitation Code')).not.toBeInTheDocument()
    expect(screen.getByLabelText('Email')).toHaveAttribute('type', 'email')
    await user.type(screen.getByLabelText('Email'), 'new@example.com')
    await user.type(screen.getByLabelText('Password', { exact: true }), 'StrongPass123')
    await user.type(screen.getByLabelText('Confirm Password'), 'StrongPass123')
    await user.click(screen.getByRole('button', { name: 'Request registration' }))

    expect(await screen.findByRole('heading', { name: 'Registration requested' })).toBeInTheDocument()
    expect(screen.getByText('Awaiting administrator approval.')).toBeInTheDocument()
    expect(register).toHaveBeenCalledExactlyOnceWith({ username: 'new@example.com', password: 'StrongPass123' })
    expect(login).not.toHaveBeenCalled()
    expect(screen.queryByText(/You can now sign in/)).not.toBeInTheDocument()
  })

  it('preserves the form when an existing account or pending request conflicts', async () => {
    vi.spyOn(authApi, 'register').mockRejectedValue(new ApiError(409, 'Conflict'))
    const user = userEvent.setup()
    renderWithProviders(<RegisterPage />)
    await user.type(screen.getByLabelText('Email'), 'pending@example.com')
    await user.type(screen.getByLabelText('Password', { exact: true }), 'StrongPass123')
    await user.type(screen.getByLabelText('Confirm Password'), 'StrongPass123')
    fireEvent.submit(screen.getByRole('button', { name: 'Request registration' }).closest('form')!)
    await waitFor(() => expect(screen.getByText('An account or pending registration already uses this email.')).toBeInTheDocument())
    expect(screen.getByLabelText('Email')).toHaveValue('pending@example.com')
  })
})


it.each(['light', 'dark'])('uses the saved %s theme on registration', (theme) => {
  localStorage.setItem('llmlb-theme', theme)
  document.documentElement.classList.toggle('dark', theme !== 'dark')
  renderWithProviders(<RegisterPage />)
  expect(document.documentElement.classList.contains('dark')).toBe(theme === 'dark')
})
