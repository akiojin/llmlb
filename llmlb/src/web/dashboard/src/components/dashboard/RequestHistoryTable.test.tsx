import { screen, waitFor, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { type RequestHistoryItem } from '@/lib/api'
import { renderWithProviders } from '@/test/render'
import { RequestHistoryTable } from './RequestHistoryTable'

function request(overrides: Partial<RequestHistoryItem> = {}): RequestHistoryItem {
  return {
    request_id: 'req-ok',
    timestamp: '2026-10-01T00:00:00Z',
    model: 'gpt-oss:20b',
    node_id: '11111111-aaaa-bbbb-cccc-000000000001',
    node_name: 'gpu-node-1',
    status: 'success',
    duration_ms: 1500,
    total_tokens: 321,
    client_ip: '192.0.2.10',
    ...overrides,
  }
}

const succeeded = request({
  request_body: { model: 'gpt-oss:20b', messages: [{ role: 'user', content: 'hello' }] },
})
const failed = request({
  request_id: 'req-failed',
  model: 'qwen3:8b',
  node_id: '22222222-aaaa-bbbb-cccc-000000000002',
  node_name: undefined,
  status: 'error',
  duration_ms: 250,
  total_tokens: undefined,
  client_ip: undefined,
  error: 'upstream timed out',
})

/** `count` requests whose model name (`model-01`, ...) identifies the row. */
function requests(count: number, overrides: Partial<RequestHistoryItem> = {}) {
  return Array.from({ length: count }, (_, i) => {
    const n = String(i + 1).padStart(2, '0')
    return request({ request_id: `req-${n}`, model: `model-${n}`, ...overrides })
  })
}

function renderTable(history: RequestHistoryItem[], isLoading = false) {
  return renderWithProviders(<RequestHistoryTable history={history} isLoading={isLoading} />)
}

function bodyRows() {
  return screen.getAllByRole('row').slice(1)
}

/** Model names in the order the body rows are displayed. */
function rowModels() {
  return bodyRows().map((row) => within(row).getAllByRole('cell')[1].textContent)
}

async function chooseStatus(user: UserEvent, option: string) {
  await user.click(screen.getByRole('combobox', { name: 'Filter by status' }))
  await user.click(await screen.findByRole('option', { name: option }))
}

// The pager buttons are icon-only and have no accessible name; they are the
// two buttons around the page indicator.
function pagerButtons(indicator: string) {
  const pager = screen.getByText(indicator).parentElement as HTMLElement
  const [previous, next] = within(pager).getAllByRole('button')
  return { previous, next }
}

describe('RequestHistoryTable', () => {
  it('shows a loading placeholder instead of the table', () => {
    renderTable([], true)

    expect(screen.getByRole('status', { name: 'Loading' })).toBeInTheDocument()
    expect(screen.queryByRole('table')).not.toBeInTheDocument()
    expect(screen.queryByText('No request history')).not.toBeInTheDocument()
  })

  it('renders a row per request', () => {
    renderTable([succeeded, failed])

    expect(screen.queryByRole('status', { name: 'Loading' })).not.toBeInTheDocument()
    const [first, second] = bodyRows().map((row) => within(row))
    expect(first.getByText('gpt-oss:20b')).toBeInTheDocument()
    expect(first.getByText('gpu-node-1')).toBeInTheDocument()
    expect(first.getByText('192.0.2.10')).toBeInTheDocument()
    expect(first.getByText('success')).toBeInTheDocument()
    expect(first.getByText('1.5s')).toBeInTheDocument()
    expect(first.getByText('321')).toBeInTheDocument()

    expect(second.getByText('qwen3:8b')).toBeInTheDocument()
    expect(second.getByText('error')).toBeInTheDocument()
    expect(second.getByText('250ms')).toBeInTheDocument()
    // An unnamed node falls back to the short node id; unknown values to a dash.
    const cells = second.getAllByRole('cell')
    expect(cells[2]).toHaveTextContent('22222222')
    expect(cells[3]).toHaveTextContent('—')
    expect(cells[6]).toHaveTextContent('—')
  })

  it('shows the empty state when there is no history', () => {
    renderTable([])

    expect(screen.getByRole('heading', { name: 'No request history' })).toBeInTheDocument()
    expect(
      screen.getByText('Requests will appear here once traffic is routed through the balancer.'),
    ).toBeInTheDocument()
  })

  it('filters by status', async () => {
    const user = userEvent.setup()
    renderTable([succeeded, failed])

    await chooseStatus(user, 'Error')
    expect(rowModels()).toEqual(['qwen3:8b'])

    await chooseStatus(user, 'Success')
    expect(rowModels()).toEqual(['gpt-oss:20b'])

    await chooseStatus(user, 'All status')
    expect(rowModels()).toEqual(['gpt-oss:20b', 'qwen3:8b'])
  })

  it('shows the filter empty state when no request matches the status', async () => {
    renderTable([succeeded])

    await chooseStatus(userEvent.setup(), 'Error')

    expect(screen.getByRole('heading', { name: 'No requests match the filter' })).toBeInTheDocument()
    expect(screen.queryByText('No request history')).not.toBeInTheDocument()
  })

  it('pages through the history 25 requests at a time', async () => {
    const user = userEvent.setup()
    const history = requests(30)
    const models = history.map((item) => item.model)
    renderTable(history)

    expect(rowModels()).toEqual(models.slice(0, 25))
    expect(screen.getByText('Showing 1 to 25 of 30')).toBeInTheDocument()
    const { previous, next } = pagerButtons('Page 1 of 2')
    expect(previous).toBeDisabled()

    await user.click(next)
    expect(rowModels()).toEqual(models.slice(25))
    expect(screen.getByText('Showing 26 to 30 of 30')).toBeInTheDocument()
    expect(screen.getByText('Page 2 of 2')).toBeInTheDocument()
    expect(next).toBeDisabled()

    await user.click(previous)
    expect(screen.getByText('Showing 1 to 25 of 30')).toBeInTheDocument()
    expect(screen.getByText('Page 1 of 2')).toBeInTheDocument()
  })

  it('shows more requests per page when the page size is raised', async () => {
    const user = userEvent.setup()
    renderTable(requests(30))

    // The page-size select has no accessible name; it is the one showing 25.
    const pageSize = screen.getAllByRole('combobox').find((el) => el.textContent === '25')
    if (!pageSize) throw new Error('No select is showing the page size')
    await user.click(pageSize)
    await user.click(await screen.findByRole('option', { name: '50' }))

    expect(bodyRows()).toHaveLength(30)
    expect(screen.queryByText(/^Page \d+ of \d+$/)).not.toBeInTheDocument()
  })

  it('returns to the first page when the status filter changes', async () => {
    const user = userEvent.setup()
    renderTable(requests(30, { status: 'error' }))

    await user.click(pagerButtons('Page 1 of 2').next)
    expect(screen.getByText('Page 2 of 2')).toBeInTheDocument()

    await chooseStatus(user, 'Error')

    expect(screen.getByText('Page 1 of 2')).toBeInTheDocument()
    expect(within(bodyRows()[0]).getByText('model-01')).toBeInTheDocument()
  })

  it('opens the details of the clicked request', async () => {
    const user = userEvent.setup()
    renderTable([succeeded, failed])

    await user.click(bodyRows()[1])

    const dialog = await screen.findByRole('dialog', { name: 'Request Details' })
    expect(within(dialog).getByText('req-failed')).toBeInTheDocument()
    const overview = within(within(dialog).getByRole('tabpanel', { name: 'Overview' }))
    expect(overview.getByText('qwen3:8b')).toBeInTheDocument()
    expect(overview.getByText('error')).toBeInTheDocument()
    expect(overview.getByText('22222222-aaaa-bbbb-cccc-000000000002')).toBeInTheDocument()
    expect(overview.getByText('250ms')).toBeInTheDocument()
    expect(overview.getByText('upstream timed out')).toBeInTheDocument()
  })

  it('shows another request after the dialog is closed', async () => {
    const user = userEvent.setup()
    renderTable([succeeded, failed])

    await user.click(bodyRows()[1])
    await user.click(
      within(await screen.findByRole('dialog')).getByRole('button', { name: 'Close' }),
    )
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())

    await user.click(bodyRows()[0])

    const dialog = await screen.findByRole('dialog', { name: 'Request Details' })
    expect(within(dialog).getByText('req-ok')).toBeInTheDocument()
    const overview = within(within(dialog).getByRole('tabpanel', { name: 'Overview' }))
    expect(overview.getByText('gpu-node-1')).toBeInTheDocument()
    expect(overview.getByText('192.0.2.10')).toBeInTheDocument()
    expect(overview.getByText('321')).toBeInTheDocument()
    // Only failed requests carry an error box.
    expect(overview.queryByText('Error')).not.toBeInTheDocument()
  })

  it('shows the request and response bodies in their tabs', async () => {
    const user = userEvent.setup()
    renderTable([succeeded])

    await user.click(bodyRows()[0])
    const dialog = within(await screen.findByRole('dialog', { name: 'Request Details' }))

    await user.click(dialog.getByRole('tab', { name: 'Request' }))
    const requestBody = dialog.getByRole('tabpanel', { name: 'Request' })
    expect(requestBody).toHaveTextContent('"model": "gpt-oss:20b"')
    expect(requestBody).toHaveTextContent('"content": "hello"')

    await user.click(dialog.getByRole('tab', { name: 'Response' }))
    expect(dialog.getByRole('tabpanel', { name: 'Response' })).toHaveTextContent('No response body')
  })

  it('copies the request body to the clipboard', async () => {
    // userEvent.setup() installs a clipboard, which the component only uses
    // in a secure context.
    const user = userEvent.setup()
    vi.stubGlobal('isSecureContext', true)
    renderTable([succeeded])

    await user.click(bodyRows()[0])
    const dialog = within(await screen.findByRole('dialog', { name: 'Request Details' }))
    await user.click(dialog.getByRole('tab', { name: 'Request' }))
    // The copy button is icon-only and the only button in the panel.
    await user.click(within(dialog.getByRole('tabpanel', { name: 'Request' })).getByRole('button'))

    expect(await screen.findByText('Copied to clipboard')).toBeInTheDocument()
    expect(JSON.parse(await navigator.clipboard.readText())).toEqual(succeeded.request_body)
  })
})
