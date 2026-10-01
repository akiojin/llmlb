import { act, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { ApiError, endpointsApi, type DashboardEndpoint } from '@/lib/api'
import type { AssistantTextParts } from '@/lib/reasoning'
import { deferred, renderPage } from '@/test/render'
import EndpointPlayground from './EndpointPlayground'

type EndpointModels = Awaited<ReturnType<typeof endpointsApi.getModels>>

const ENDPOINT_ID = 'ep-42'

function endpoint(overrides: Partial<DashboardEndpoint> = {}): DashboardEndpoint {
  return {
    id: ENDPOINT_ID,
    name: 'gpu-node-1',
    base_url: 'http://gpu-node-1:8080',
    status: 'online',
    endpoint_type: 'xllm',
    health_check_interval_secs: 30,
    inference_timeout_secs: 600,
    error_count: 0,
    registered_at: '2026-10-01T00:00:00Z',
    model_count: 2,
    total_requests: 0,
    successful_requests: 0,
    failed_requests: 0,
    ...overrides,
  }
}

function modelList(...models: EndpointModels['models']): EndpointModels {
  return { endpoint_id: ENDPOINT_ID, models }
}

function chunk(content: string, reasoning = ''): AssistantTextParts {
  return { content, reasoning }
}

function stubEndpoint(overrides: Partial<DashboardEndpoint> = {}) {
  return vi.spyOn(endpointsApi, 'get').mockResolvedValue(endpoint(overrides))
}

function stubModels(...ids: string[]) {
  return vi
    .spyOn(endpointsApi, 'getModels')
    .mockResolvedValue(modelList(...ids.map((model_id) => ({ model_id }))))
}

function stubStreamedReply(...chunks: AssistantTextParts[]) {
  return vi
    .spyOn(endpointsApi, 'chatCompletions')
    .mockImplementation(async (_id, _request, onDelta) => {
      chunks.forEach((delta) => onDelta?.(delta))
      return null
    })
}

const modelSelect = () => screen.getByRole('combobox')
const promptInput = () => screen.getByPlaceholderText('Type a message or attach files...')
const sendButton = () => screen.getByRole('button', { name: 'Send' })

async function renderPlayground(onBack: () => void = vi.fn()) {
  renderPage(<EndpointPlayground endpointId={ENDPOINT_ID} onBack={onBack} />)
  await screen.findByText('Playground')
}

async function waitForSelectedModel(id: string) {
  await waitFor(() => expect(modelSelect()).toHaveTextContent(id))
}

async function sendPrompt(text: string) {
  const user = userEvent.setup()
  await user.type(promptInput(), text)
  await user.click(sendButton())
}

describe('EndpointPlayground', () => {
  describe('endpoint details', () => {
    it('switches from the loading state to the details of the requested endpoint', async () => {
      const response = deferred<DashboardEndpoint>()
      const get = vi.spyOn(endpointsApi, 'get').mockReturnValue(response.promise)
      const getModels = stubModels('model-a', 'model-b')
      renderPage(<EndpointPlayground endpointId={ENDPOINT_ID} onBack={vi.fn()} />)

      expect(await screen.findByText('Loading endpoint...')).toBeInTheDocument()
      expect(screen.queryByRole('combobox')).not.toBeInTheDocument()
      expect(get).toHaveBeenCalledExactlyOnceWith(ENDPOINT_ID)
      expect(getModels).toHaveBeenCalledExactlyOnceWith(ENDPOINT_ID)

      response.resolve(endpoint())

      expect(await screen.findByRole('heading', { name: 'gpu-node-1' })).toBeInTheDocument()
      expect(screen.queryByText('Loading endpoint...')).not.toBeInTheDocument()
      expect(screen.getByText('http://gpu-node-1:8080')).toBeInTheDocument()
      expect(screen.getByText('Start a conversation')).toBeInTheDocument()
      await waitFor(() =>
        expect(screen.getByText('Models:').parentElement).toHaveTextContent('Models: 2'),
      )
    })

    it.each([
      ['online', 'Online'],
      ['pending', 'Pending'],
      ['offline', 'Offline'],
      ['error', 'Error'],
    ] as const)('labels an endpoint with status %s as %s', async (status, label) => {
      stubEndpoint({ status })
      stubModels('model-a')

      await renderPlayground()

      expect(screen.getByText('Status:').parentElement).toHaveTextContent(`Status: ${label}`)
    })

    it('warns when the endpoint has no base URL', async () => {
      stubEndpoint({ base_url: '' })
      stubModels('model-a')
      await renderPlayground()

      expect(screen.getByText('URL:').parentElement).toHaveTextContent('URL: Not set')
      expect(
        screen.getByText('Base URL is not configured. Please check the endpoint settings.'),
      ).toBeInTheDocument()

      await userEvent.setup().click(screen.getByRole('button', { name: 'cURL' }))

      expect(await screen.findByRole('dialog', { name: 'cURL Command' })).toHaveTextContent(
        '# Error: endpoint base_url is not configured. Please set it in the dashboard.',
      )
    })

    it('shows the request as a cURL command against the endpoint', async () => {
      stubEndpoint({ base_url: 'http://gpu-node-1:8080/' })
      stubModels('model-a')
      await renderPlayground()
      await waitForSelectedModel('model-a')

      await userEvent.setup().click(screen.getByRole('button', { name: 'cURL' }))

      const dialog = await screen.findByRole('dialog', { name: 'cURL Command' })
      expect(dialog).toHaveTextContent("curl -X POST 'http://gpu-node-1:8080/v1/chat/completions'")
      expect(dialog).toHaveTextContent('"model": "model-a"')
    })

    it('returns to the dashboard from the sidebar', async () => {
      stubEndpoint()
      stubModels('model-a')
      const onBack = vi.fn()
      await renderPlayground(onBack)

      await userEvent.setup().click(screen.getByRole('button', { name: 'Back to Dashboard' }))

      expect(onBack).toHaveBeenCalledOnce()
    })
  })

  describe('model list', () => {
    it('switches from loading to the loaded models and selects the first one', async () => {
      stubEndpoint()
      const response = deferred<EndpointModels>()
      vi.spyOn(endpointsApi, 'getModels').mockReturnValue(response.promise)
      await renderPlayground()
      const user = userEvent.setup()

      expect(modelSelect()).toHaveTextContent('Select a model')
      await user.type(promptInput(), 'Hello')
      expect(sendButton()).toBeDisabled()

      await user.click(modelSelect())
      expect(await screen.findByRole('option', { name: 'Loading models...' })).toHaveAttribute(
        'aria-disabled',
        'true',
      )

      response.resolve(modelList({ model_id: 'model-a' }, { model_id: 'model-b' }))

      expect(await screen.findByRole('option', { name: 'model-b' })).toBeInTheDocument()
      expect(screen.queryByRole('option', { name: 'Loading models...' })).not.toBeInTheDocument()
      await waitFor(() =>
        expect(screen.getByRole('option', { name: 'model-a' })).toHaveAttribute(
          'aria-selected',
          'true',
        ),
      )
    })

    it('offers nothing to select and blocks sending when the endpoint has no models', async () => {
      stubEndpoint()
      stubModels()
      const chatCompletions = vi.spyOn(endpointsApi, 'chatCompletions')
      await renderPlayground()
      const user = userEvent.setup()

      await user.click(modelSelect())
      expect(await screen.findByRole('option', { name: 'No models available' })).toHaveAttribute(
        'aria-disabled',
        'true',
      )
      await user.keyboard('{Escape}')
      await user.type(promptInput(), 'Hello{Enter}')

      expect(sendButton()).toBeDisabled()
      expect(chatCompletions).not.toHaveBeenCalled()
      expect(screen.getByText('Models:').parentElement).toHaveTextContent('Models: 0')
    })

    it('reports a model list failure', async () => {
      stubEndpoint()
      vi.spyOn(endpointsApi, 'getModels').mockRejectedValue(new ApiError(404, 'Not Found'))
      await renderPlayground()

      expect(await screen.findByText('API endpoint not found.')).toBeInTheDocument()
      expect(modelSelect()).toHaveTextContent('Select a model')
    })

    it('sends to the model the user picks', async () => {
      stubEndpoint()
      stubModels('model-a', 'model-b')
      const chatCompletions = stubStreamedReply(chunk('ok'))
      await renderPlayground()
      const user = userEvent.setup()
      await waitForSelectedModel('model-a')

      await user.click(modelSelect())
      await user.click(await screen.findByRole('option', { name: 'model-b' }))
      expect(modelSelect()).toHaveTextContent('model-b')
      await sendPrompt('Hello')

      expect(chatCompletions).toHaveBeenCalledExactlyOnceWith(
        ENDPOINT_ID,
        expect.objectContaining({ model: 'model-b' }),
        expect.any(Function),
        expect.any(AbortSignal),
      )
      expect(await screen.findByText('ok')).toBeInTheDocument()
    })
  })

  describe('chat', () => {
    it('sends the prompt to the endpoint and renders the reply as it streams', async () => {
      stubEndpoint()
      stubModels('model-a')
      const reply = deferred<null>()
      const chatCompletions = vi
        .spyOn(endpointsApi, 'chatCompletions')
        .mockReturnValue(reply.promise)
      await renderPlayground()
      await waitForSelectedModel('model-a')

      expect(sendButton()).toBeDisabled()
      await sendPrompt('What is xLLM?')

      expect(chatCompletions).toHaveBeenCalledExactlyOnceWith(
        ENDPOINT_ID,
        expect.objectContaining({
          model: 'model-a',
          messages: [{ role: 'user', content: 'What is xLLM?' }],
          stream: true,
        }),
        expect.any(Function),
        expect.any(AbortSignal),
      )
      expect(screen.getByText('What is xLLM?')).toBeInTheDocument()
      expect(screen.queryByText('Start a conversation')).not.toBeInTheDocument()
      expect(screen.getByText('Generating response...')).toBeInTheDocument()
      expect(promptInput()).toHaveValue('')
      expect(screen.getByRole('button', { name: 'Stop' })).toBeInTheDocument()

      const onDelta = chatCompletions.mock.calls[0][2]!
      act(() => onDelta(chunk('An inference')))
      expect(screen.getByText('An inference')).toBeInTheDocument()
      expect(screen.queryByText('Generating response...')).not.toBeInTheDocument()

      act(() => onDelta(chunk(' engine.')))
      expect(screen.getByText('An inference engine.')).toBeInTheDocument()

      await act(async () => reply.resolve(null))

      expect(sendButton()).toBeDisabled()
      expect(screen.queryByRole('button', { name: 'Stop' })).not.toBeInTheDocument()
    })

    it('sends on Enter and keeps the conversation history in the next request', async () => {
      stubEndpoint()
      stubModels('model-a')
      const chatCompletions = stubStreamedReply(chunk('First answer'))
      await renderPlayground()
      const user = userEvent.setup()
      await waitForSelectedModel('model-a')

      await user.type(promptInput(), 'First question{Enter}')
      expect(await screen.findByText('First answer')).toBeInTheDocument()
      await user.type(promptInput(), 'Second question{Enter}')

      await waitFor(() => expect(chatCompletions).toHaveBeenCalledTimes(2))
      expect(chatCompletions.mock.calls[1][1].messages).toEqual([
        { role: 'user', content: 'First question' },
        { role: 'assistant', content: 'First answer' },
        { role: 'user', content: 'Second question' },
      ])
      await waitFor(() => expect(sendButton()).toBeInTheDocument())
    })

    it('shows the reasoning of a reply separately from the answer', async () => {
      stubEndpoint()
      stubModels('model-a')
      stubStreamedReply(chunk('', 'Thinking it through'), chunk('Final answer'))
      await renderPlayground()
      await waitForSelectedModel('model-a')

      await sendPrompt('Why?')

      expect(await screen.findByText('Final answer')).toBeInTheDocument()
      expect(screen.getByText('Show reasoning')).toBeInTheDocument()
      expect(screen.getByText('Thinking it through')).toBeInTheDocument()
    })

    it('reports a failed request and restores the prompt for a retry', async () => {
      stubEndpoint()
      stubModels('model-a')
      vi.spyOn(endpointsApi, 'chatCompletions').mockRejectedValue(
        new ApiError(504, 'Gateway Timeout'),
      )
      await renderPlayground()
      await waitForSelectedModel('model-a')

      await sendPrompt('Hello')

      expect(await screen.findByText('Failed to send message')).toBeInTheDocument()
      expect(screen.getByText('Request timed out.')).toBeInTheDocument()
      expect(screen.queryByText('Generating response...')).not.toBeInTheDocument()
      expect(promptInput()).toHaveValue('Hello')
      expect(sendButton()).toBeEnabled()
    })

    it('stops a pending reply without reporting an error', async () => {
      stubEndpoint()
      stubModels('model-a')
      const chatCompletions = vi.spyOn(endpointsApi, 'chatCompletions').mockImplementation(
        (_id, _request, _onDelta, signal) =>
          new Promise((_resolve, reject) => {
            signal?.addEventListener('abort', () =>
              reject(new DOMException('The operation was aborted.', 'AbortError')),
            )
          }),
      )
      await renderPlayground()
      await waitForSelectedModel('model-a')
      await sendPrompt('Hello')

      await userEvent.setup().click(screen.getByRole('button', { name: 'Stop' }))

      expect(chatCompletions.mock.calls[0][3]?.aborted).toBe(true)
      expect(await screen.findByRole('button', { name: 'Send' })).toBeInTheDocument()
      expect(screen.queryByText('Generating response...')).not.toBeInTheDocument()
      expect(screen.queryByText('Failed to send message')).not.toBeInTheDocument()
    })

    it('applies the system prompt and the non-streaming setting', async () => {
      stubEndpoint()
      stubModels('model-a')
      const chatCompletions = vi
        .spyOn(endpointsApi, 'chatCompletions')
        .mockResolvedValue({ choices: [{ message: { content: 'Whole answer' } }] })
      await renderPlayground()
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

      expect(chatCompletions).toHaveBeenCalledExactlyOnceWith(
        ENDPOINT_ID,
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
    })

    it('requests the model context size when the user opts into it', async () => {
      stubEndpoint()
      vi.spyOn(endpointsApi, 'getModels').mockResolvedValue(
        modelList({ model_id: 'model-a', max_tokens: 8192 }),
      )
      const chatCompletions = stubStreamedReply(chunk('ok'))
      await renderPlayground()
      const user = userEvent.setup()
      await waitForSelectedModel('model-a')

      await user.click(screen.getByRole('button', { name: 'Settings' }))
      const settings = within(await screen.findByRole('dialog', { name: 'Settings' }))
      await user.click(settings.getByRole('checkbox', { name: /Use model max context/ }))
      await user.click(settings.getByRole('button', { name: 'Done' }))
      await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
      await sendPrompt('Hello')

      expect(chatCompletions.mock.calls[0][1].max_tokens).toBe(8192)
      expect(await screen.findByText('ok')).toBeInTheDocument()
    })

    it('clears the conversation', async () => {
      stubEndpoint()
      stubModels('model-a')
      stubStreamedReply(chunk('An answer'))
      await renderPlayground()
      await waitForSelectedModel('model-a')
      await sendPrompt('A question')
      expect(await screen.findByText('An answer')).toBeInTheDocument()

      await userEvent.setup().click(screen.getByRole('button', { name: 'Clear Chat' }))

      expect(await screen.findByText('Chat cleared')).toBeInTheDocument()
      expect(screen.getByText('Start a conversation')).toBeInTheDocument()
      expect(screen.queryByText('A question')).not.toBeInTheDocument()
      expect(screen.queryByText('An answer')).not.toBeInTheDocument()
    })
  })
})
