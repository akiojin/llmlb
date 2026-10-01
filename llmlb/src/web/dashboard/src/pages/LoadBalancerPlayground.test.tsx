import { act, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import {
  ApiError,
  chatApi,
  dashboardApi,
  type OpenAIModel,
  type OpenAIModelsResponse,
  type RequestResponseRecord,
  type RequestResponsesPage,
} from '@/lib/api'
import type { AssistantTextParts } from '@/lib/reasoning'
import { deferred, renderPage, viewerUser } from '@/test/render'
import LoadBalancerPlayground from './LoadBalancerPlayground'

function model(id: string, overrides: Partial<OpenAIModel> = {}): OpenAIModel {
  return {
    id,
    object: 'model',
    created: 0,
    owned_by: 'router',
    lifecycle_status: 'registered',
    ready: true,
    ...overrides,
  }
}

function modelList(...models: OpenAIModel[]): OpenAIModelsResponse {
  return { object: 'list', data: models }
}

function record(overrides: Partial<RequestResponseRecord> = {}): RequestResponseRecord {
  return {
    id: 'record-1',
    timestamp: '2026-10-01T00:00:00Z',
    request_type: 'chat',
    model: 'model-a',
    endpoint_name: 'gpu-node-1',
    duration_ms: 120,
    status: { type: 'success' },
    ...overrides,
  }
}

function historyPage(records: RequestResponseRecord[]): RequestResponsesPage {
  return { records, total_count: records.length, page: 1, per_page: 100 }
}

function chunk(content: string, reasoning = ''): AssistantTextParts {
  return { content, reasoning }
}

function stubModels(...ids: string[]) {
  return vi.spyOn(chatApi, 'getModels').mockResolvedValue(modelList(...ids.map((id) => model(id))))
}

// Every completed chat looks up which endpoint served it.
function stubHistory(records: RequestResponseRecord[] = []) {
  return vi.spyOn(dashboardApi, 'getRequestResponses').mockResolvedValue(historyPage(records))
}

function stubStreamedReply(...chunks: string[]) {
  return vi.spyOn(chatApi, 'complete').mockImplementation(async (_request, onDelta) => {
    chunks.forEach((content) => onDelta?.(chunk(content)))
    return null
  })
}

const modelSelect = () => screen.getByRole('combobox')
const promptInput = () => screen.getByPlaceholderText('Type a message or attach files...')
const sendButton = () => screen.getByRole('button', { name: 'Send' })

async function waitForSelectedModel(id: string) {
  await waitFor(() => expect(modelSelect()).toHaveTextContent(id))
}

async function sendPrompt(text: string) {
  const user = userEvent.setup()
  await user.type(promptInput(), text)
  await user.click(sendButton())
}

describe('LoadBalancerPlayground', () => {
  describe('model list', () => {
    it('switches from loading to the loaded models and selects the first one', async () => {
      const response = deferred<OpenAIModelsResponse>()
      vi.spyOn(chatApi, 'getModels').mockReturnValue(response.promise)
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)

      expect(await screen.findByRole('heading', { name: 'Load Balancer' })).toBeInTheDocument()
      expect(screen.getByText('Start a load balancer conversation')).toBeInTheDocument()
      expect(modelSelect()).toHaveTextContent('Select a model')
      expect(screen.getByRole('button', { name: 'Refresh Models' })).toBeDisabled()

      await userEvent.setup().click(modelSelect())
      expect(await screen.findByRole('option', { name: 'Loading models...' })).toHaveAttribute(
        'aria-disabled',
        'true',
      )

      response.resolve(modelList(model('model-a'), model('model-b')))

      expect(await screen.findByRole('option', { name: 'model-b' })).toBeInTheDocument()
      expect(screen.queryByRole('option', { name: 'Loading models...' })).not.toBeInTheDocument()
      await waitFor(() =>
        expect(screen.getByRole('option', { name: 'model-a' })).toHaveAttribute(
          'aria-selected',
          'true',
        ),
      )
    })

    it('summarizes the loaded models in the sidebar', async () => {
      stubModels('model-a', 'model-b')
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)

      await waitForSelectedModel('model-a')

      expect(screen.getByText('Models:').parentElement).toHaveTextContent('Models: 2')
      expect(screen.getByText('Mode:').parentElement).toHaveTextContent('Mode: Interactive chat')
      expect(screen.getByRole('button', { name: 'Refresh Models' })).toBeEnabled()
    })

    it('offers nothing to select and blocks sending when there are no models', async () => {
      stubModels()
      const complete = vi.spyOn(chatApi, 'complete')
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)
      const user = userEvent.setup()

      await waitFor(() => expect(screen.getByRole('button', { name: 'Refresh Models' })).toBeEnabled())
      await user.type(promptInput(), 'Hello{Enter}')

      expect(sendButton()).toBeDisabled()
      expect(complete).not.toHaveBeenCalled()
      expect(screen.getByText('Models:').parentElement).toHaveTextContent('Models: 0')

      await user.click(modelSelect())
      expect(await screen.findByRole('option', { name: 'No models available' })).toHaveAttribute(
        'aria-disabled',
        'true',
      )
    })

    it('reports a model list failure', async () => {
      vi.spyOn(chatApi, 'getModels').mockRejectedValue(new ApiError(503, 'Service Unavailable'))
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)

      expect(
        await screen.findByText('No available endpoints. Please check endpoint status.'),
      ).toBeInTheDocument()
      expect(modelSelect()).toHaveTextContent('Select a model')
    })

    it('reloads the models on request', async () => {
      const getModels = vi
        .spyOn(chatApi, 'getModels')
        .mockResolvedValueOnce(modelList(model('model-a')))
        .mockResolvedValueOnce(modelList(model('model-a'), model('model-b'), model('model-c')))
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)
      await waitForSelectedModel('model-a')

      await userEvent.setup().click(screen.getByRole('button', { name: 'Refresh Models' }))

      await waitFor(() =>
        expect(screen.getByText('Models:').parentElement).toHaveTextContent('Models: 3'),
      )
      expect(getModels).toHaveBeenCalledTimes(2)
      expect(modelSelect()).toHaveTextContent('model-a')
    })

    it('preselects the initial model', async () => {
      stubModels('model-a', 'model-b')
      stubHistory()
      const complete = stubStreamedReply('ok')
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} initialModel="model-b" />)

      await waitForSelectedModel('model-b')
      await sendPrompt('Hello')

      expect(complete).toHaveBeenCalledExactlyOnceWith(
        expect.objectContaining({ model: 'model-b' }),
        expect.any(Function),
        expect.any(AbortSignal),
      )
      expect(await screen.findByRole('button', { name: 'Send' })).toBeInTheDocument()
    })

    it('falls back to the first model when the initial model is not served', async () => {
      stubModels('model-a', 'model-b')
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} initialModel="retired-model" />)

      await waitForSelectedModel('model-a')
    })

    it('sends to the model the user picks', async () => {
      stubModels('model-a', 'model-b')
      stubHistory()
      const complete = stubStreamedReply('ok')
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)
      const user = userEvent.setup()
      await waitForSelectedModel('model-a')

      await user.click(modelSelect())
      await user.click(await screen.findByRole('option', { name: 'model-b' }))
      expect(modelSelect()).toHaveTextContent('model-b')
      await sendPrompt('Hello')

      expect(complete).toHaveBeenCalledExactlyOnceWith(
        expect.objectContaining({ model: 'model-b' }),
        expect.any(Function),
        expect.any(AbortSignal),
      )
      expect(await screen.findByRole('button', { name: 'Send' })).toBeInTheDocument()
    })
  })

  describe('chat', () => {
    it('sends the prompt with the selected model and renders the reply as it streams', async () => {
      stubModels('model-a')
      stubHistory()
      const reply = deferred<null>()
      const complete = vi.spyOn(chatApi, 'complete').mockReturnValue(reply.promise)
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)
      await waitForSelectedModel('model-a')

      expect(sendButton()).toBeDisabled()
      await sendPrompt('What is llmlb?')

      expect(complete).toHaveBeenCalledExactlyOnceWith(
        expect.objectContaining({
          model: 'model-a',
          messages: [{ role: 'user', content: 'What is llmlb?' }],
          stream: true,
        }),
        expect.any(Function),
        expect.any(AbortSignal),
      )
      expect(screen.getByText('What is llmlb?')).toBeInTheDocument()
      expect(screen.queryByText('Start a load balancer conversation')).not.toBeInTheDocument()
      expect(screen.getByText('Generating response...')).toBeInTheDocument()
      expect(promptInput()).toHaveValue('')
      expect(promptInput()).toBeDisabled()
      expect(screen.getByRole('button', { name: 'Stop' })).toBeInTheDocument()

      const onDelta = complete.mock.calls[0][1]!
      act(() => onDelta(chunk('A load')))
      expect(screen.getByText('A load')).toBeInTheDocument()
      expect(screen.queryByText('Generating response...')).not.toBeInTheDocument()

      act(() => onDelta(chunk(' balancer.')))
      expect(screen.getByText('A load balancer.')).toBeInTheDocument()

      await act(async () => reply.resolve(null))

      expect(await screen.findByRole('button', { name: 'Send' })).toBeDisabled()
      expect(promptInput()).toBeEnabled()
    })

    it('sends on Enter and keeps the conversation history in the next request', async () => {
      stubModels('model-a')
      stubHistory()
      const complete = stubStreamedReply('First answer')
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)
      const user = userEvent.setup()
      await waitForSelectedModel('model-a')

      await user.type(promptInput(), 'First question{Enter}')
      expect(await screen.findByText('First answer')).toBeInTheDocument()
      await waitFor(() => expect(promptInput()).toBeEnabled())
      await user.type(promptInput(), 'Second question{Enter}')

      await waitFor(() => expect(complete).toHaveBeenCalledTimes(2))
      expect(complete.mock.calls[1][0].messages).toEqual([
        { role: 'user', content: 'First question' },
        { role: 'assistant', content: 'First answer' },
        { role: 'user', content: 'Second question' },
      ])
      await waitFor(() => expect(promptInput()).toBeEnabled())
    })

    it('shows the reasoning of a reply separately from the answer', async () => {
      stubModels('model-a')
      stubHistory()
      vi.spyOn(chatApi, 'complete').mockImplementation(async (_request, onDelta) => {
        onDelta?.(chunk('', 'Thinking it through'))
        onDelta?.(chunk('Final answer'))
        return null
      })
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)
      await waitForSelectedModel('model-a')

      await sendPrompt('Why?')

      expect(await screen.findByText('Final answer')).toBeInTheDocument()
      expect(screen.getByText('Show reasoning')).toBeInTheDocument()
      expect(screen.getByText('Thinking it through')).toBeInTheDocument()
      expect(await screen.findByRole('button', { name: 'Send' })).toBeInTheDocument()
    })

    it('reports a failed request and restores the prompt for a retry', async () => {
      stubModels('model-a')
      vi.spyOn(chatApi, 'complete').mockRejectedValue(new ApiError(429, 'Too Many Requests'))
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)
      await waitForSelectedModel('model-a')

      await sendPrompt('Hello')

      expect(await screen.findByText('Failed to send message')).toBeInTheDocument()
      expect(screen.getByText('Rate limited. Please reduce request volume.')).toBeInTheDocument()
      expect(screen.queryByText('Generating response...')).not.toBeInTheDocument()
      expect(promptInput()).toHaveValue('Hello')
      expect(sendButton()).toBeEnabled()
    })

    it('stops a pending reply without reporting an error', async () => {
      stubModels('model-a')
      const complete = vi.spyOn(chatApi, 'complete').mockImplementation(
        (_request, _onDelta, signal) =>
          new Promise((_resolve, reject) => {
            signal?.addEventListener('abort', () =>
              reject(new DOMException('The operation was aborted.', 'AbortError')),
            )
          }),
      )
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)
      await waitForSelectedModel('model-a')
      await sendPrompt('Hello')

      await userEvent.setup().click(screen.getByRole('button', { name: 'Stop' }))

      expect(complete.mock.calls[0][2]?.aborted).toBe(true)
      expect(await screen.findByRole('button', { name: 'Send' })).toBeInTheDocument()
      await waitFor(() => expect(promptInput()).toBeEnabled())
      expect(screen.queryByText('Generating response...')).not.toBeInTheDocument()
      expect(screen.queryByText('Failed to send message')).not.toBeInTheDocument()
    })

    it('applies the system prompt and the non-streaming setting', async () => {
      stubModels('model-a')
      stubHistory()
      const complete = vi
        .spyOn(chatApi, 'complete')
        .mockResolvedValue({ choices: [{ message: { content: 'Whole answer' } }] })
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)
      const user = userEvent.setup()
      await waitForSelectedModel('model-a')
      expect(screen.getByText('Streaming')).toBeInTheDocument()

      await user.click(screen.getByRole('button', { name: 'Settings' }))
      const settings = within(await screen.findByRole('dialog', { name: 'Settings' }))
      await user.type(settings.getByPlaceholderText('You are a helpful assistant...'), 'Be brief.')
      await user.click(settings.getByRole('switch'))
      await user.click(settings.getByRole('button', { name: 'Done' }))
      await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
      expect(screen.queryByText('Streaming')).not.toBeInTheDocument()

      await sendPrompt('Hello')

      expect(complete).toHaveBeenCalledExactlyOnceWith(
        expect.objectContaining({
          model: 'model-a',
          messages: [
            { role: 'system', content: 'Be brief.' },
            { role: 'user', content: 'Hello' },
          ],
          stream: false,
        }),
        undefined,
        expect.any(AbortSignal),
      )
      expect(await screen.findByText('Whole answer')).toBeInTheDocument()
      expect(await screen.findByRole('button', { name: 'Send' })).toBeInTheDocument()
    })

    it('requests the model context size when the user opts into it', async () => {
      vi.spyOn(chatApi, 'getModels').mockResolvedValue(
        modelList(model('model-a', { max_tokens: 8192 })),
      )
      stubHistory()
      const complete = stubStreamedReply('ok')
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)
      const user = userEvent.setup()
      await waitForSelectedModel('model-a')

      await user.click(screen.getByRole('button', { name: 'Settings' }))
      const settings = within(await screen.findByRole('dialog', { name: 'Settings' }))
      await user.click(settings.getByRole('checkbox', { name: /Use model max context/ }))
      await user.click(settings.getByRole('button', { name: 'Done' }))
      await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
      await sendPrompt('Hello')

      expect(complete.mock.calls[0][0].max_tokens).toBe(8192)
      expect(await screen.findByRole('button', { name: 'Send' })).toBeInTheDocument()
    })

    it('clears the conversation', async () => {
      stubModels('model-a')
      stubHistory()
      stubStreamedReply('An answer')
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)
      await waitForSelectedModel('model-a')
      await sendPrompt('A question')
      expect(await screen.findByText('Request Distribution')).toBeInTheDocument()
      await screen.findByRole('button', { name: 'Send' })

      await userEvent.setup().click(screen.getByRole('button', { name: 'Clear Chat' }))

      expect(await screen.findByText('Chat cleared')).toBeInTheDocument()
      expect(screen.getByText('Start a load balancer conversation')).toBeInTheDocument()
      expect(screen.queryByText('A question')).not.toBeInTheDocument()
      expect(screen.queryByText('An answer')).not.toBeInTheDocument()
      expect(screen.queryByText('Request Distribution')).not.toBeInTheDocument()
    })

    it('shows the request as a cURL command', async () => {
      stubModels('model-a')
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)
      await waitForSelectedModel('model-a')

      await userEvent.setup().click(screen.getByRole('button', { name: 'cURL' }))

      const dialog = await screen.findByRole('dialog', { name: 'cURL Command' })
      expect(dialog).toHaveTextContent("curl -X POST '/api/dashboard/playground/chat/completions'")
      expect(dialog).toHaveTextContent('"model": "model-a"')
    })

    it('returns to the dashboard from the sidebar', async () => {
      stubModels('model-a')
      const onBack = vi.fn()
      renderPage(<LoadBalancerPlayground onBack={onBack} />)
      await waitForSelectedModel('model-a')

      await userEvent.setup().click(screen.getByRole('button', { name: 'Back to Dashboard' }))

      expect(onBack).toHaveBeenCalledOnce()
    })
  })

  describe('request distribution', () => {
    it('shows which endpoint served the request', async () => {
      stubModels('model-a')
      const complete = stubStreamedReply('ok')
      // Only records tagged with this run belong to the request just sent.
      const history = vi.spyOn(dashboardApi, 'getRequestResponses').mockImplementation(async () =>
        historyPage([
          record({ id: 'r-1', request_body: { user: complete.mock.calls[0][0].user } }),
          record({ id: 'r-2', endpoint_name: 'other-node', request_body: { user: 'someone-else' } }),
        ]),
      )
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)
      await waitForSelectedModel('model-a')

      await sendPrompt('Hello')

      expect(await screen.findByText('gpu-node-1')).toBeInTheDocument()
      expect(screen.getByText(/Matched: 1\/1/)).toBeInTheDocument()
      expect(screen.queryByText('other-node')).not.toBeInTheDocument()
      expect(history).toHaveBeenCalledExactlyOnceWith({ limit: 100, offset: 0 })
    })

    it('says so when the request is not in the history yet', async () => {
      stubModels('model-a')
      stubHistory([record({ request_body: { user: 'someone-else' } })])
      stubStreamedReply('ok')
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)
      await waitForSelectedModel('model-a')

      await sendPrompt('Hello')

      expect(await screen.findByText('No records found for this run yet.')).toBeInTheDocument()
      expect(screen.getByText(/Matched: 0\/1/)).toBeInTheDocument()
    })

    it('reports a failed history lookup while keeping the reply', async () => {
      stubModels('model-a')
      stubStreamedReply('Still answered')
      vi.spyOn(dashboardApi, 'getRequestResponses').mockRejectedValue(
        new Error('history unavailable'),
      )
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)
      await waitForSelectedModel('model-a')

      await sendPrompt('Hello')

      expect(await screen.findByText('history unavailable')).toBeInTheDocument()
      expect(screen.getByText('Still answered')).toBeInTheDocument()
      expect(screen.queryByText('Failed to send message')).not.toBeInTheDocument()
    })
  })

  describe('load test', () => {
    it('offers the load test mode to admins', async () => {
      stubModels('model-a')
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)
      const user = userEvent.setup()
      await waitForSelectedModel('model-a')

      await user.click(await screen.findByRole('button', { name: 'Load Test' }))

      expect(screen.getByText('Load test settings')).toBeInTheDocument()
      expect(screen.getByLabelText('Requests')).toHaveValue(200)
      expect(screen.getByLabelText('Concurrency')).toHaveValue(10)
      expect(screen.getByLabelText('Interval (ms)')).toHaveValue(0)
      expect(screen.getByText('Mode:').parentElement).toHaveTextContent('Mode: Load test')
      expect(screen.getByRole('button', { name: 'Start Load Test' })).toBeEnabled()
      expect(screen.queryByRole('button', { name: 'Send' })).not.toBeInTheDocument()
      expect(screen.queryByRole('button', { name: 'Attach image' })).not.toBeInTheDocument()

      await user.click(screen.getByRole('button', { name: 'Chat' }))

      expect(screen.queryByText('Load test settings')).not.toBeInTheDocument()
      expect(sendButton()).toBeInTheDocument()
    })

    it('hides the load test mode from non-admin users', async () => {
      stubModels('model-a')
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />, { user: viewerUser })

      await waitForSelectedModel('model-a')

      expect(screen.getByRole('button', { name: 'Chat' })).toBeInTheDocument()
      expect(screen.queryByRole('button', { name: 'Load Test' })).not.toBeInTheDocument()
    })

    it('runs the configured number of requests and reports the outcome', async () => {
      stubModels('model-a')
      const history = stubHistory()
      const complete = vi.spyOn(chatApi, 'complete')
      const completeLoadTest = vi
        .spyOn(chatApi, 'completeLoadTest')
        .mockResolvedValueOnce({})
        .mockRejectedValueOnce(new ApiError(503, 'Service Unavailable'))
        .mockResolvedValueOnce({})
      renderPage(<LoadBalancerPlayground onBack={vi.fn()} />)
      const user = userEvent.setup()
      await waitForSelectedModel('model-a')

      await user.click(await screen.findByRole('button', { name: 'Load Test' }))
      await user.clear(screen.getByLabelText('Requests'))
      await user.type(screen.getByLabelText('Requests'), '3')
      await user.clear(screen.getByLabelText('Concurrency'))
      await user.type(screen.getByLabelText('Concurrency'), '1')
      await user.type(
        screen.getByPlaceholderText('Prompt used for each load test request...'),
        'ping',
      )
      await user.click(screen.getByRole('button', { name: 'Start Load Test' }))

      expect(
        await screen.findByText('Load test finished. requests=3, success=2, error=1'),
      ).toBeInTheDocument()
      expect(screen.getByText('3/3 completed')).toBeInTheDocument()
      expect(screen.getByText('success=2, error=1')).toBeInTheDocument()
      expect(completeLoadTest).toHaveBeenCalledTimes(3)
      expect(completeLoadTest).toHaveBeenLastCalledWith(
        expect.objectContaining({
          model: 'model-a',
          messages: [{ role: 'user', content: 'ping' }],
          stream: false,
        }),
        expect.any(AbortSignal),
      )
      expect(complete).not.toHaveBeenCalled()
      expect(history).toHaveBeenCalled()
      expect(await screen.findByRole('button', { name: 'Start Load Test' })).toBeEnabled()
    })
  })
})
