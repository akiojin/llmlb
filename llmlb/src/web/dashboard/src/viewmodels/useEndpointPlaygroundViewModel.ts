import { useEffect, useRef } from 'react'
import { useQuery } from '@tanstack/react-query'
import { endpointsApi, ApiError, type DashboardEndpoint } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { isAbortError } from '@/lib/utils'
import { toast } from '@/hooks/use-toast'
import { usePlayground } from '@/hooks/usePlayground'
import { useEndpointViewModel } from '@/viewmodels/useEndpointViewModel'
import { splitAssistantMessage } from '@/lib/reasoning'
import { getErrorMessage, transformMessage, MAX_INPUT_CHARS, type Message } from '@/components/playground/types'

type EndpointModel = Awaited<ReturnType<typeof endpointsApi.getModels>>['models'][number]

export interface EndpointPlaygroundViewModel {
  pg: ReturnType<typeof usePlayground>
  isLoadingEndpoint: boolean
  endpointName: string | undefined
  endpointBaseUrl: string | undefined
  endpointStatus: DashboardEndpoint['status'] | undefined
  statusLabel: string
  models: EndpointModel[]
  isLoadingModels: boolean
  selectedModelMaxTokens: EndpointModel['max_tokens']
  hasBaseUrl: boolean
  canSend: boolean
  sendMessage: () => Promise<void>
  curlCommand: string
}

function getStatusLabel(
  status: DashboardEndpoint['status'] | undefined
): string {
  switch (status) {
    case 'online':
      return 'Online'
    case 'pending':
      return 'Pending'
    case 'offline':
      return 'Offline'
    case 'error':
      return 'Error'
    default:
      return 'Unknown'
  }
}

/**
 * SPEC #821 T011: compose shared conversation/settings/attachment/DOM commands
 * from usePlayground and scoped detail fetching from useEndpointViewModel.
 * This hook owns endpoint model selection, inference, lifecycle and display values.
 * The LB ViewModel owns its separate gateway/load-test I/O; neither duplicates
 * shared usePlayground state. Keep model polling/retry and detail subscriptions:
 * models had no resource invalidation, so adding one would change US-004 timing.
 */
export function useEndpointPlaygroundViewModel(endpointId: string): EndpointPlaygroundViewModel {
  const pg = usePlayground()
  const { abortControllerRef, selectedModel, setSelectedModel } = pg
  const isMountedRef = useRef(true)

  useEffect(() => {
    isMountedRef.current = true
    return () => {
      isMountedRef.current = false
      abortControllerRef.current?.abort()
    }
  }, [abortControllerRef])

  const { endpoint, isLoadingEndpoint } = useEndpointViewModel(endpointId)

  const { data: endpointModels, isLoading: isLoadingModels, error: modelsError } = useQuery({
    queryKey: queryKeys.endpointModels(endpointId),
    queryFn: () => endpointsApi.getModels(endpointId),
    retry: false,
  })

  useEffect(() => {
    if (modelsError) {
      let description = 'Failed to fetch model list'
      if (modelsError instanceof ApiError) {
        description = getErrorMessage(modelsError)
      }
      toast({ title: 'Error', description, variant: 'destructive' })
    }
  }, [modelsError])

  useEffect(() => {
    if (endpointModels?.models && !selectedModel && endpointModels.models.length > 0) {
      setSelectedModel(endpointModels.models[0].model_id)
    }
  }, [endpointModels, selectedModel, setSelectedModel])

  const models = endpointModels?.models || []
  const selectedModelMaxTokens = models.find(m => m.model_id === pg.selectedModel)?.max_tokens
  const effectiveMaxTokens = pg.useMaxContext && selectedModelMaxTokens != null ? selectedModelMaxTokens : pg.maxTokens
  const baseUrl = endpoint?.base_url?.replace(/\/$/, '') || ''
  const hasBaseUrl = baseUrl.length > 0

  const sendMessage = async () => {
    if ((!pg.input.trim() && pg.attachments.length === 0) || !pg.selectedModel || pg.isStreaming) return

    if (pg.input.length > MAX_INPUT_CHARS) {
      toast({
        title: 'Message too long',
        description: `Keep your message under ${MAX_INPUT_CHARS.toLocaleString()} characters.`,
        variant: 'destructive',
      })
      return
    }

    const userMessage: Message = {
      role: 'user',
      content: pg.input.trim(),
      attachments: pg.attachments.length > 0 ? pg.attachments : undefined,
    }
    const newMessages = [...pg.messages, userMessage]
    pg.setMessages(newMessages)
    pg.setInput('')
    pg.setAttachments([])
    pg.setIsStreaming(true)
    abortControllerRef.current = new AbortController()

    try {
      const requestMessages = pg.systemPrompt
        ? [{ role: 'system' as const, content: pg.systemPrompt }, ...newMessages.map(transformMessage)]
        : newMessages.map(transformMessage)

      if (pg.streamEnabled) {
        let assistantContent = ''
        let assistantReasoning = ''
        pg.setMessages((prev) => [...prev, { role: 'assistant', content: '' }])

        await endpointsApi.chatCompletions(
          endpointId,
          {
            model: pg.selectedModel,
            messages: requestMessages,
            stream: true,
            temperature: pg.temperature,
            max_tokens: effectiveMaxTokens,
          },
          (delta) => {
            assistantContent += delta.content
            assistantReasoning += delta.reasoning
            if (!isMountedRef.current) return
            pg.setMessages((prev) => {
              const updated = [...prev]
              updated[updated.length - 1] = {
                role: 'assistant',
                content: assistantContent,
                reasoning: assistantReasoning || undefined,
              }
              return updated
            })
          },
          abortControllerRef.current.signal
        )
      } else {
        const data = await endpointsApi.chatCompletions(
          endpointId,
          {
            model: pg.selectedModel,
            messages: requestMessages,
            stream: false,
            temperature: pg.temperature,
            max_tokens: effectiveMaxTokens,
          },
          undefined,
          abortControllerRef.current.signal
        )

        const { content, reasoning } = splitAssistantMessage(data)
        pg.setMessages((prev) => [...prev, {
          role: 'assistant',
          content,
          reasoning: reasoning || undefined,
        }])
      }
    } catch (error) {
      if (!isAbortError(error)) {
        toast({
          title: 'Failed to send message',
          description:
            error instanceof ApiError
              ? getErrorMessage(error)
              : error instanceof Error
                ? error.message
                : 'Unknown error',
          variant: 'destructive',
        })
        // Drop only the optimistic empty assistant placeholder (streaming),
        // keep the user's message, and restore the input so it can be resent.
        pg.setMessages((prev) => {
          const last = prev[prev.length - 1]
          if (last && last.role === 'assistant' && last.content === '') {
            return prev.slice(0, -1)
          }
          return prev
        })
        pg.setInput(userMessage.content)
      }
    } finally {
      if (isMountedRef.current) {
        pg.setIsStreaming(false)
      }
      abortControllerRef.current = null
      if (isMountedRef.current) {
        pg.inputRef.current?.focus()
      }
    }
  }

  const generateCurl = () => {
    const requestMessages = pg.systemPrompt
      ? [{ role: 'system', content: pg.systemPrompt }, ...pg.messages]
      : pg.messages

    if (!hasBaseUrl) {
      return '# Error: endpoint base_url is not configured. Please set it in the dashboard.'
    }

    return `curl -X POST '${baseUrl}/v1/chat/completions' \\
  -H 'Content-Type: application/json' \\
  -d '${JSON.stringify(
    {
      model: pg.selectedModel,
      messages: requestMessages,
      stream: pg.streamEnabled,
      temperature: pg.temperature,
      max_tokens: effectiveMaxTokens,
    },
    null,
    2
  )}'`
  }

  return {
    pg,
    isLoadingEndpoint,
    endpointName: endpoint?.name,
    endpointBaseUrl: endpoint?.base_url,
    endpointStatus: endpoint?.status,
    statusLabel: getStatusLabel(endpoint?.status),
    models,
    isLoadingModels,
    selectedModelMaxTokens,
    hasBaseUrl,
    canSend: !!(pg.input.trim() || pg.attachments.length > 0) && !!pg.selectedModel,
    sendMessage,
    curlCommand: generateCurl(),
  }
}
