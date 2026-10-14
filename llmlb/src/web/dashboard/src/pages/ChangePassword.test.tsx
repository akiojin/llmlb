import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { Toaster } from '@/components/ui/toaster'
import { TooltipProvider } from '@/components/ui/tooltip'
import { ApiError, authApi } from '@/lib/api'
import { adminUser, deferred, type TestUser } from '@/test/render'
import ChangePasswordPage from './ChangePassword'

const CURRENT_PASSWORD = 'Curr3ntPass'
const VALID_PASSWORD = 'Str0ngPass'

// Same tree as the change-password.tsx entry point. The page checks the
// session itself, so there is no AuthProvider.
function renderChangePassword(session: Promise<TestUser> = Promise.resolve(adminUser)) {
  vi.spyOn(authApi, 'me').mockReturnValue(session)
  return render(
    <TooltipProvider>
      <ChangePasswordPage />
      <Toaster />
    </TooltipProvider>,
  )
}

async function submitPasswords(newPassword: string, confirmPassword: string) {
  const user = userEvent.setup()
  await user.type(await screen.findByLabelText('Current Password'), CURRENT_PASSWORD)
  await user.type(screen.getByLabelText('New Password'), newPassword)
  await user.type(screen.getByLabelText('Confirm Password'), confirmPassword)
  await user.click(screen.getByRole('button', { name: 'Change Password' }))
}

describe('ChangePasswordPage', () => {
  it('switches from the loading state to the form once the session is confirmed', async () => {
    const session = deferred<TestUser>()
    renderChangePassword(session.promise)

    expect(screen.getByText('Loading...')).toBeInTheDocument()
    expect(screen.queryByLabelText('New Password')).not.toBeInTheDocument()

    session.resolve({ ...adminUser, must_change_password: true })

    expect(await screen.findByLabelText('Current Password')).toHaveAttribute('type', 'password')
    expect(screen.getByLabelText('New Password')).toHaveAttribute('type', 'password')
    expect(screen.getByLabelText('Confirm Password')).toHaveAttribute('type', 'password')
    expect(screen.getByText('You must change your password before continuing.')).toBeInTheDocument()
    expect(screen.queryByText('Loading...')).not.toBeInTheDocument()
  })

  it('keeps the submit button disabled until every field is filled', async () => {
    renderChangePassword()
    const user = userEvent.setup()

    const submit = await screen.findByRole('button', { name: 'Change Password' })
    expect(submit).toBeDisabled()

    await user.type(screen.getByLabelText('Current Password'), CURRENT_PASSWORD)
    expect(submit).toBeDisabled()

    await user.type(screen.getByLabelText('New Password'), VALID_PASSWORD)
    expect(submit).toBeDisabled()

    await user.type(screen.getByLabelText('Confirm Password'), VALID_PASSWORD)
    expect(submit).toBeEnabled()
  })

  it('rates the password against the policy while the user types', async () => {
    renderChangePassword()
    const user = userEvent.setup()
    const newPassword = await screen.findByLabelText('New Password')

    expect(screen.queryByRole('list', { name: 'Password requirements' })).not.toBeInTheDocument()

    await user.type(newPassword, 'abc')

    const requirements = within(screen.getByRole('list', { name: 'Password requirements' }))
    expect(requirements.getByText('At least 8 characters')).toBeInTheDocument()
    expect(requirements.getByText('Contains an uppercase letter')).toBeInTheDocument()
    expect(requirements.getByText('Contains a number')).toBeInTheDocument()
    expect(screen.getByText('Weak')).toBeInTheDocument()

    await user.clear(newPassword)
    await user.type(newPassword, 'Str0ng-Passw0rd')

    expect(screen.getByText('Very strong')).toBeInTheDocument()
    expect(screen.queryByText('Weak')).not.toBeInTheDocument()
  })

  it.each([
    ['is shorter than 8 characters', 'Sh0rt'],
    ['has no uppercase letter', 'lowercase1'],
    ['has no number', 'NoNumberHere'],
  ])('rejects a password that %s without calling the API', async (_reason, password) => {
    const changePassword = vi.spyOn(authApi, 'changePassword')
    renderChangePassword()

    await submitPasswords(password, password)

    expect(await screen.findByText('Password too weak')).toBeInTheDocument()
    expect(
      screen.getByText('Use at least 8 characters including an uppercase letter and a number.'),
    ).toBeInTheDocument()
    expect(changePassword).not.toHaveBeenCalled()
  })

  it('rejects a confirmation that does not match without calling the API', async () => {
    const changePassword = vi.spyOn(authApi, 'changePassword')
    renderChangePassword()

    await submitPasswords(VALID_PASSWORD, `${VALID_PASSWORD}x`)

    expect(await screen.findByText('Passwords do not match')).toBeInTheDocument()
    expect(changePassword).not.toHaveBeenCalled()
  })

  it('submits the current and new passwords and shows a busy state while saving', async () => {
    // Left pending: a successful change schedules a full page navigation to
    // the login page, which the Playwright suite covers.
    const changePassword = vi
      .spyOn(authApi, 'changePassword')
      .mockReturnValue(deferred<never>().promise)
    renderChangePassword()

    await submitPasswords(VALID_PASSWORD, VALID_PASSWORD)

    expect(changePassword).toHaveBeenCalledExactlyOnceWith(CURRENT_PASSWORD, VALID_PASSWORD)
    expect(screen.getByRole('button', { name: 'Changing password...' })).toBeDisabled()
  })

  it('reports a failed change and lets the user retry', async () => {
    const changePassword = vi
      .spyOn(authApi, 'changePassword')
      .mockRejectedValue(new ApiError(500, 'Internal Server Error'))
    renderChangePassword()

    await submitPasswords(VALID_PASSWORD, VALID_PASSWORD)

    expect(await screen.findByText('Failed to change password')).toBeInTheDocument()
    expect(screen.getByText('Please try again')).toBeInTheDocument()
    expect(changePassword).toHaveBeenCalledOnce()
    expect(screen.getByRole('button', { name: 'Change Password' })).toBeEnabled()
    expect(screen.getByLabelText('New Password')).toHaveValue(VALID_PASSWORD)
  })
})
