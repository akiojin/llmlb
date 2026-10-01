import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { auditLogApi, type AuditLogEntry, type AuditLogListResponse } from '@/lib/api'
import { deferred, renderWithProviders, viewerUser } from '@/test/render'
import AuditLogPage from './AuditLog'

function entry(overrides: Partial<AuditLogEntry> = {}): AuditLogEntry {
  return {
    id: 1,
    timestamp: '2026-10-01T00:00:00Z',
    http_method: 'POST',
    request_path: '/api/endpoints',
    status_code: 201,
    actor_type: 'user',
    actor_id: 'user-admin',
    actor_username: 'admin',
    api_key_owner_id: null,
    client_ip: '192.0.2.10',
    duration_ms: 12,
    input_tokens: null,
    output_tokens: null,
    total_tokens: null,
    model_name: null,
    endpoint_id: null,
    detail: null,
    is_migrated: false,
    ...overrides,
  }
}

function page(items: AuditLogEntry[], total = items.length): AuditLogListResponse {
  return { items, total, page: 1, per_page: 50 }
}

describe('AuditLogPage', () => {
  it('switches from the loading state to the entry table', async () => {
    const response = deferred<AuditLogListResponse>()
    const list = vi.spyOn(auditLogApi, 'list').mockReturnValue(response.promise)
    renderWithProviders(<AuditLogPage onBack={vi.fn()} />)

    await screen.findByRole('heading', { name: 'Audit Log' })
    expect(list).toHaveBeenCalledWith({ page: 1, per_page: 50 })
    expect(screen.queryByRole('table')).not.toBeInTheDocument()
    expect(screen.queryByText('No audit log entries found')).not.toBeInTheDocument()

    response.resolve(
      page([
        entry(),
        entry({ id: 2, http_method: 'DELETE', request_path: '/api/users/7', client_ip: null }),
      ]),
    )

    const table = await screen.findByRole('table')
    expect(within(table).getByText('/api/endpoints')).toBeInTheDocument()
    expect(within(table).getByText('/api/users/7')).toBeInTheDocument()
    expect(within(table).getByRole('link', { name: '192.0.2.10' })).toHaveAttribute(
      'href',
      '?tab=clients&ip=192.0.2.10',
    )
  })

  it('shows the empty state when there are no entries', async () => {
    vi.spyOn(auditLogApi, 'list').mockResolvedValue(page([]))
    renderWithProviders(<AuditLogPage onBack={vi.fn()} />)

    expect(await screen.findByText('No audit log entries found')).toBeInTheDocument()
    expect(screen.queryByRole('table')).not.toBeInTheDocument()
  })

  it('denies access to non-admin users without requesting entries', async () => {
    const list = vi.spyOn(auditLogApi, 'list')
    const onBack = vi.fn()
    renderWithProviders(<AuditLogPage onBack={onBack} />, { user: viewerUser })

    expect(await screen.findByRole('heading', { name: 'Access Denied' })).toBeInTheDocument()
    await userEvent.setup().click(screen.getByRole('button', { name: 'Back to Dashboard' }))

    expect(onBack).toHaveBeenCalledOnce()
    expect(list).not.toHaveBeenCalled()
  })

  it('returns to the dashboard from the header', async () => {
    vi.spyOn(auditLogApi, 'list').mockResolvedValue(page([]))
    const onBack = vi.fn()
    renderWithProviders(<AuditLogPage onBack={onBack} />)

    await userEvent.setup().click(await screen.findByRole('button', { name: 'Dashboard' }))

    expect(onBack).toHaveBeenCalledOnce()
  })

  it('requests the next page', async () => {
    const list = vi.spyOn(auditLogApi, 'list').mockResolvedValue(page([entry()], 120))
    renderWithProviders(<AuditLogPage onBack={vi.fn()} />)

    expect(await screen.findByText('Page 1 / 3')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Previous page' })).toBeDisabled()
    await userEvent.setup().click(screen.getByRole('button', { name: 'Next page' }))

    await waitFor(() => expect(list).toHaveBeenLastCalledWith({ page: 2, per_page: 50 }))
    expect(await screen.findByText('Page 2 / 3')).toBeInTheDocument()
  })

  it('searches from the first page after the typing pause', async () => {
    const list = vi.spyOn(auditLogApi, 'list').mockResolvedValue(page([entry()]))
    renderWithProviders(<AuditLogPage onBack={vi.fn()} />)

    await userEvent.setup().type(await screen.findByPlaceholderText('Search...'), 'login')

    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({ page: 1, per_page: 50, search: 'login' }),
    )
  })

  it('verifies the hash chain and reports the result', async () => {
    vi.spyOn(auditLogApi, 'list').mockResolvedValue(page([]))
    const verify = vi
      .spyOn(auditLogApi, 'verify')
      .mockResolvedValue({ valid: true, batches_checked: 3, tampered_batch: null, message: null })
    renderWithProviders(<AuditLogPage onBack={vi.fn()} />)

    await userEvent.setup().click(await screen.findByRole('button', { name: 'Verify Hash Chain' }))

    expect(verify).toHaveBeenCalledOnce()
    expect(await screen.findByText('Verified (3 batches)')).toBeInTheDocument()
  })

  it('reports a tampered batch', async () => {
    vi.spyOn(auditLogApi, 'list').mockResolvedValue(page([]))
    vi.spyOn(auditLogApi, 'verify').mockResolvedValue({
      valid: false,
      batches_checked: 5,
      tampered_batch: 4,
      message: 'hash mismatch',
    })
    renderWithProviders(<AuditLogPage onBack={vi.fn()} />)

    await userEvent.setup().click(await screen.findByRole('button', { name: 'Verify Hash Chain' }))

    expect(await screen.findByText('Tampered: Batch 4')).toBeInTheDocument()
  })

  it('reports a failed hash chain verification', async () => {
    vi.spyOn(auditLogApi, 'list').mockResolvedValue(page([]))
    vi.spyOn(auditLogApi, 'verify').mockRejectedValue(new Error('verification service unavailable'))
    renderWithProviders(<AuditLogPage onBack={vi.fn()} />)

    await userEvent.setup().click(await screen.findByRole('button', { name: 'Verify Hash Chain' }))

    expect(await screen.findByText('verification service unavailable')).toBeInTheDocument()
  })
})
