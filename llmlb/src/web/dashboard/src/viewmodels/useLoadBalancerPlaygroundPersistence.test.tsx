import { type ReactNode } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { chatApi, dashboardApi } from '@/lib/api'
import { deferred } from '@/test/render'
import { useLoadBalancerPlaygroundViewModel } from './useLoadBalancerPlaygroundViewModel'

const auth = vi.hoisted(() => ({ user: { id: 'alice', role: 'admin' } as { id: string; role: string } | null, isLoading: false }))
const storage = vi.hoisted(() => ({ load: vi.fn(), save: vi.fn(), clear: vi.fn() }))
vi.mock('@/hooks/useAuth', () => ({ useAuth: () => auth }))
vi.mock('@/lib/lbConversationStorage', () => ({ lbConversationStorage: storage }))

const saved = {
  version: 1,
  messages: [
    { role: 'user', content: 'First question', attachments: [{ type: 'image', name: 'test.png', data: 'data:image/png;base64,AAAA', mimeType: 'image/png' }] },
    { role: 'assistant', content: 'First answer', reasoning: 'Thought' },
  ],
  settings: { selectedModel: 'model-b', systemPrompt: 'Be brief', streamEnabled: false, temperature: 0.4, maxTokens: 512, useMaxContext: false },
}
function setup(initialModel?: string) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return renderHook(() => useLoadBalancerPlaygroundViewModel(initialModel), {
    wrapper: ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>,
  })
}

beforeEach(() => {
  auth.user = { id: 'alice', role: 'admin' }
  auth.isLoading = false
  storage.load.mockResolvedValue(null)
  storage.save.mockResolvedValue(undefined)
  storage.clear.mockResolvedValue(undefined)
  vi.spyOn(chatApi, 'getModels').mockResolvedValue({ object: 'list', data: ['model-a', 'model-b'].map(id => ({ id, object: 'model', created: 0, owned_by: 'router' })) })
  vi.spyOn(dashboardApi, 'getRequestResponses').mockResolvedValue({ records: [], total_count: 0, page: 1, per_page: 100 })
})

describe('LB conversation persistence', () => {
  it('restores settings, reasoning and attachments before continuing the saved context', async () => {
    storage.load.mockResolvedValue(saved)
    const complete = vi.spyOn(chatApi, 'complete').mockResolvedValue({ choices: [{ message: { content: 'Continued' } }] })
    const { result } = setup()
    await waitFor(() => expect(result.current.pg.messages).toEqual(saved.messages))
    expect(result.current.pg.selectedModel).toBe('model-b')
    act(() => result.current.pg.setInput('Next'))
    await act(() => result.current.sendMessage())
    expect(complete.mock.calls[0][0]).toMatchObject({ model: 'model-b', stream: false, temperature: 0.4, max_tokens: 512, messages: [
      { role: 'system', content: 'Be brief' },
      { role: 'user', content: [{ type: 'text', text: 'First question' }, { type: 'image_url', image_url: { url: 'data:image/png;base64,AAAA' } }] },
      { role: 'assistant', content: 'First answer' }, { role: 'user', content: 'Next' },
    ] })
    await waitFor(() => expect(storage.save).toHaveBeenCalledWith('alice', expect.objectContaining({ messages: expect.arrayContaining([{ role: 'assistant', content: 'Continued', reasoning: undefined }]) })))
  })

  it('waits for auth and restore, then preserves a valid route model over saved selection', async () => {
    auth.isLoading = true
    const restore = deferred<typeof saved>()
    storage.load.mockReturnValue(restore.promise)
    const { result, rerender } = setup('model-a')
    act(() => result.current.pg.setInput('waiting'))
    expect(result.current.canSendChat).toBe(false)
    expect(storage.load).not.toHaveBeenCalled()
    auth.isLoading = false
    rerender()
    await waitFor(() => expect(storage.load).toHaveBeenCalledWith('alice'))
    expect(storage.save).not.toHaveBeenCalled()
    await act(async () => restore.resolve(saved))
    await waitFor(() => expect(result.current.pg.selectedModel).toBe('model-a'))
  })

  it('falls back from an expired saved model without losing the conversation', async () => {
    storage.load.mockResolvedValue({ ...saved, settings: { ...saved.settings, selectedModel: 'removed' } })
    const { result } = setup()
    await waitFor(() => expect(result.current.pg.messages).toHaveLength(2))
    await waitFor(() => expect(result.current.pg.selectedModel).toBe('model-a'))
  })

  it('hides previous user state immediately and ignores a late restore from another user', async () => {
    const restore = deferred<typeof saved>()
    storage.load.mockReturnValueOnce(restore.promise).mockResolvedValueOnce(null)
    const { result, rerender } = setup()
    await waitFor(() => expect(storage.load).toHaveBeenCalledWith('alice'))
    auth.user = { id: 'bob', role: 'viewer' }
    rerender()
    expect(result.current.pg.messages).toEqual([])
    await act(async () => restore.resolve(saved))
    await waitFor(() => expect(storage.load).toHaveBeenCalledWith('bob'))
    expect(result.current.pg.messages).toEqual([])
    expect(storage.save).not.toHaveBeenCalledWith('bob', expect.objectContaining({ messages: saved.messages }))
  })

  it('clear aborts a stream and rejects late deltas rather than resurrecting the conversation', async () => {
    const response = deferred<null>()
    let delta: ((value: { content: string; reasoning: string }) => void) | undefined
    vi.spyOn(chatApi, 'complete').mockImplementation((_req, onDelta) => { delta = onDelta; return response.promise })
    const { result } = setup()
    await waitFor(() => expect(result.current.pg.selectedModel).toBe('model-a'))
    act(() => result.current.pg.setInput('Hello'))
    let send!: Promise<void>
    act(() => { send = result.current.sendMessage() })
    act(() => delta?.({ content: 'Partial', reasoning: 'Thinking' }))
    await waitFor(() => expect(storage.save).toHaveBeenCalledWith('alice', expect.objectContaining({ messages: expect.arrayContaining([expect.objectContaining({ content: 'Partial', reasoning: 'Thinking' })]) })))
    const controller = result.current.pg.abortControllerRef.current
    act(() => result.current.pg.resetChat())
    expect(controller?.signal.aborted).toBe(true)
    act(() => delta?.({ content: 'Late', reasoning: '' }))
    await act(async () => { response.resolve(null); await send })
    expect(result.current.pg.messages).toEqual([])
    expect(result.current.pg.isStreaming).toBe(false)
    await waitFor(() => expect(storage.clear).toHaveBeenCalledWith('alice'))
  })

  it('keeps a failed restore usable and reports save failures without failing inference', async () => {
    storage.load.mockRejectedValue(new Error('corrupt'))
    storage.save.mockRejectedValue(new DOMException('Quota exceeded', 'QuotaExceededError'))
    const complete = vi.spyOn(chatApi, 'complete').mockResolvedValue(null)
    const { result } = setup()
    await waitFor(() => expect(result.current.pg.selectedModel).toBe('model-a'))
    act(() => result.current.pg.setInput('Hello'))
    await act(() => result.current.sendMessage())
    expect(complete).toHaveBeenCalledOnce()
    expect(result.current.pg.isStreaming).toBe(false)
  })
})
