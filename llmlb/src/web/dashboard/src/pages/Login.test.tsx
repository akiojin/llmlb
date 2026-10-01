import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { Toaster } from '@/components/ui/toaster'
import { ApiError, authApi } from '@/lib/api'
import { deferred } from '@/test/render'
import LoginPage from './Login'

// Same tree as the login.tsx entry point.
function renderLogin() {
  return render(
    <>
      <LoginPage />
      <Toaster />
    </>,
  )
}

async function submitCredentials(username: string, password: string) {
  const user = userEvent.setup()
  await user.type(screen.getByLabelText('Username'), username)
  await user.type(screen.getByLabelText('Password'), password)
  await user.click(screen.getByRole('button', { name: 'Sign in' }))
}

describe('LoginPage', () => {
  it('renders the sign-in form', () => {
    renderLogin()

    expect(screen.getByRole('heading', { name: 'LLM Load Balancer' })).toBeInTheDocument()
    expect(screen.getByLabelText('Username')).toBeRequired()
    expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
    expect(screen.getByRole('button', { name: 'Sign in' })).toBeEnabled()
    expect(screen.getByRole('link', { name: 'Register' })).toHaveAttribute(
      'href',
      '/dashboard/register.html',
    )
  })

  it('submits the typed credentials and shows a busy state while signing in', async () => {
    // Left pending: the redirect after a successful login is a full page
    // navigation, which the Playwright suite covers.
    const login = vi.spyOn(authApi, 'login').mockReturnValue(deferred<never>().promise)
    renderLogin()

    await submitCredentials('admin', 'correct horse')

    expect(login).toHaveBeenCalledExactlyOnceWith('admin', 'correct horse')
    expect(screen.getByRole('button', { name: 'Signing in...' })).toBeDisabled()
  })

  it('reports a failed login and lets the user retry', async () => {
    vi.spyOn(authApi, 'login').mockRejectedValue(new ApiError(401, 'Unauthorized'))
    renderLogin()

    await submitCredentials('admin', 'wrong')

    expect(await screen.findByText('Login failed')).toBeInTheDocument()
    expect(screen.getByText('Invalid username or password')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Sign in' })).toBeEnabled()
  })

  it('does not submit while a required field is empty', async () => {
    const login = vi.spyOn(authApi, 'login')
    renderLogin()

    await userEvent.setup().click(screen.getByRole('button', { name: 'Sign in' }))

    expect(login).not.toHaveBeenCalled()
  })
})
