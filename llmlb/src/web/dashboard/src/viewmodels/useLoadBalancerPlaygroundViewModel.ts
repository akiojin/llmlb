import { queryKeys } from '@/lib/queryKeys'
import { useState, useEffect, useRef, useCallback, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import {
  chatApi,
  dashboardApi,
  ApiError,
  type ChatMessage,
  type OpenAIModel,
  type OpenAIModelsResponse,
  type RequestResponseRecord,
} from '@/lib/api'
import { isAbortError } from '@/lib/utils'
import { toast } from '@/hooks/use-toast'
import { useAuth } from '@/hooks/useAuth'
import { usePlayground } from '@/hooks/usePlayground'
import { splitAssistantMessage } from '@/lib/reasoning'
import {
  getErrorMessage,
  transformMessage,
  MAX_INPUT_CHARS,
  type Message,
} from '@/components/playground/types'
const DEFAULT_LOAD_TEST_SETTINGS = {
  totalRequests: 200,
  concurrency: 10,
  intervalMs: 0,
}

type PlaygroundMode = 'chat' | 'load_test'

interface DistributionRow {
  endpoint: string
  count: number
  success: number
  error: number
  averageDurationMs: number
}

interface LoadTestProgress {
  total: number
  completed: number
  success: number
  error: number
}

interface DistributionSummary {
  runId: string
  expectedCount: number
  matchedCount: number
  rows: DistributionRow[]
  updatedAt: string
}

function generateRunId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID()
  }
  return `${Date.now()}-${Math.random().toString(16).slice(2)}`
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

function toNumberValue(value: string, fallback: number): number {
  const parsed = Number.parseInt(value, 10)
  return Number.isFinite(parsed) ? parsed : fallback
}

function extractUserTag(requestBody: unknown): string | null {
  if (!requestBody || typeof requestBody !== 'object') {
    return null
  }
  const record = requestBody as Record<string, unknown>
  return typeof record.user === 'string' ? record.user : null
}

function buildDistributionRows(records: RequestResponseRecord[]): DistributionRow[] {
  const map = new Map<
    string,
    { count: number; success: number; error: number; totalDuration: number }
  >()

  records.forEach((record) => {
    const endpoint = record.endpoint_name || record.endpoint_id || 'unknown'
    const current = map.get(endpoint) ?? { count: 0, success: 0, error: 0, totalDuration: 0 }
    current.count += 1
    current.totalDuration += record.duration_ms ?? 0
    if (record.status.type === 'success') {
      current.success += 1
    } else {
      current.error += 1
    }
    map.set(endpoint, current)
  })

  return Array.from(map.entries())
    .map(([endpoint, data]) => ({
      endpoint,
      count: data.count,
      success: data.success,
      error: data.error,
      averageDurationMs: data.count > 0 ? Math.round(data.totalDuration / data.count) : 0,
    }))
    .sort((a, b) => b.count - a.count)
}

export interface LoadBalancerPlaygroundViewModel {
  pg: ReturnType<typeof usePlayground>
  isAdmin: boolean
  mode: PlaygroundMode
  setMode: (mode: PlaygroundMode) => void
  loadTestTotalRequests: string
  setLoadTestTotalRequests: (value: string) => void
  loadTestConcurrency: string
  setLoadTestConcurrency: (value: string) => void
  loadTestIntervalMs: string
  setLoadTestIntervalMs: (value: string) => void
  isLoadTesting: boolean
  isStoppingLoadTest: boolean
  loadTestProgress: LoadTestProgress | null
  loadTestProgressLabel: string
  loadTestOutcomeLabel: string
  distributionSummary: { label: string; rows: DistributionRow[] } | null
  distributionError: string | null
  isRefreshingDistribution: boolean
  refreshDistribution: () => Promise<void>
  modelOptions: OpenAIModel[]
  isLoadingModels: boolean
  refetchModels: () => Promise<void>
  selectedModelMaxTokens: OpenAIModel['max_tokens']
  canSendChat: boolean
  progressRate: number
  sendMessage: () => Promise<void>
  startLoadTest: () => Promise<void>
  stopLoadTest: () => void
  curlCommand: string
}

/**
 * SPEC #821 T010: the load-balancer-specific ViewModel composes usePlayground.
 * usePlayground owns shared conversation/input/settings/attachments/dialog state,
 * DOM refs, clipboard/file handling, reset and generation cancellation. No shared
 * state is duplicated here. This hook owns model fetching/selection, auth/mode,
 * inference and load-test I/O, run lifecycle, distribution and display values.
 *
 * Keep the existing model key, retry/staleTime and provider's 5s polling. This
 * query had no resource invalidation; adding a useInvalidateOn subscription or a
 * WebSocket here would change US-004 update timing. Distribution is fetched only
 * after completion or explicit refresh, not on notifications/polling. Queries
 * that already subscribe elsewhere continue to use useInvalidateOn (FR-004).
 */
export function useLoadBalancerPlaygroundViewModel(initialModel?: string): LoadBalancerPlaygroundViewModel {
  const { user } = useAuth()
  // Load Test は実推論を大量発行するため admin ロール限定（Chat は全ユーザー可）。
  const isAdmin = user?.role === 'admin'
  const [mode, setMode] = useState<PlaygroundMode>('chat')

  // 非 admin が Load Test モードに留まらないよう Chat へ矯正する（レンダー中の state 調整）。
  if (!isAdmin && mode === 'load_test') {
    setMode('chat')
  }

  const [loadTestTotalRequests, setLoadTestTotalRequests] = useState(
    String(DEFAULT_LOAD_TEST_SETTINGS.totalRequests)
  )
  const [loadTestConcurrency, setLoadTestConcurrency] = useState(
    String(DEFAULT_LOAD_TEST_SETTINGS.concurrency)
  )
  const [loadTestIntervalMs, setLoadTestIntervalMs] = useState(
    String(DEFAULT_LOAD_TEST_SETTINGS.intervalMs)
  )
  const [isLoadTesting, setIsLoadTesting] = useState(false)
  const [isStoppingLoadTest, setIsStoppingLoadTest] = useState(false)
  const [loadTestProgress, setLoadTestProgress] = useState<LoadTestProgress | null>(null)
  const [distributionSummary, setDistributionSummary] = useState<DistributionSummary | null>(null)
  const [distributionError, setDistributionError] = useState<string | null>(null)
  const [isRefreshingDistribution, setIsRefreshingDistribution] = useState(false)

  const isMountedRef = useRef(true)
  const loadTestStopRef = useRef(false)
  const loadTestAbortControllersRef = useRef<Set<AbortController>>(new Set())
  const appliedInitialModelRef = useRef<string | null>(null)

  const pg = usePlayground({
    onResetExtra: () => {
      setDistributionSummary(null)
      setDistributionError(null)
    },
  })
  const { abortControllerRef, selectedModel, setSelectedModel } = pg

  const {
    data: modelsData,
    isLoading: isLoadingModels,
    error: modelsError,
    refetch: refetchModelsQuery,
  } = useQuery<OpenAIModelsResponse>({
    queryKey: queryKeys.loadBalancerPlaygroundModels(),
    queryFn: () => chatApi.getModels(),
    retry: false,
    staleTime: 5000,
  })

  useEffect(() => {
    if (!modelsError) return
    let description = 'Failed to fetch model list'
    if (modelsError instanceof ApiError) {
      description = getErrorMessage(modelsError)
    } else if (modelsError instanceof Error) {
      description = modelsError.message
    }
    toast({ title: 'Error', description, variant: 'destructive' })
  }, [modelsError])

  useEffect(() => {
    if (!initialModel) {
      appliedInitialModelRef.current = null
      return
    }
    if (!modelsData?.data) return
    const exists = modelsData.data.some((m) => m.id === initialModel)
    if (!exists) return
    if (appliedInitialModelRef.current === initialModel) return
    setSelectedModel(initialModel)
    appliedInitialModelRef.current = initialModel
  }, [initialModel, modelsData?.data, setSelectedModel])

  useEffect(() => {
    const models = Array.isArray(modelsData?.data) ? modelsData.data : []
    const hasInitialModel = initialModel ? models.some((model) => model.id === initialModel) : false
    if (hasInitialModel) return
    if (models.length === 0) {
      if (selectedModel) setSelectedModel('')
      return
    }
    const hasSelectedModel = models.some((model) => model.id === selectedModel)
    if (!selectedModel || !hasSelectedModel) {
      setSelectedModel(models[0].id)
    }
  }, [modelsData, selectedModel, initialModel, setSelectedModel])

  const selectedModelMaxTokens = modelsData?.data?.find(m => m.id === pg.selectedModel)?.max_tokens
  const effectiveMaxTokens = pg.useMaxContext && selectedModelMaxTokens != null ? selectedModelMaxTokens : pg.maxTokens

  useEffect(() => {
    isMountedRef.current = true
    const controllers = loadTestAbortControllersRef.current
    return () => {
      isMountedRef.current = false
      abortControllerRef.current?.abort()
      loadTestStopRef.current = true
      controllers.forEach((controller) => controller.abort())
      controllers.clear()
    }
  }, [abortControllerRef])

  const fetchDistributionForRun = useCallback(async (runId: string, expectedCount: number) => {
    setIsRefreshingDistribution(true)
    setDistributionError(null)

    try {
      const matched: RequestResponseRecord[] = []
      const pageSize = 100
      const maxPages = 30

      for (let page = 0; page < maxPages; page += 1) {
        const response = await dashboardApi.getRequestResponses({
          limit: pageSize,
          offset: page * pageSize,
        })

        if (response.records.length === 0) break

        for (const record of response.records) {
          const tag = extractUserTag(record.request_body)
          if (tag?.startsWith(`lbpg:${runId}:`)) {
            matched.push(record)
          }
        }

        if (matched.length >= expectedCount || (page + 1) * pageSize >= response.total_count) break
      }

      setDistributionSummary({
        runId,
        expectedCount,
        matchedCount: matched.length,
        rows: buildDistributionRows(matched),
        updatedAt: new Date().toISOString(),
      })
    } catch (error) {
      setDistributionError(error instanceof Error ? error.message : 'Failed to load distribution')
    } finally {
      setIsRefreshingDistribution(false)
    }
  }, [])

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

    const runId = generateRunId()
    const runTag = `lbpg:${runId}:1`

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
        ? ([{ role: 'system', content: pg.systemPrompt } as ChatMessage].concat(
            newMessages.map(transformMessage)
          ))
        : newMessages.map(transformMessage)

      if (pg.streamEnabled) {
        let assistantContent = ''
        let assistantReasoning = ''
        pg.setMessages((prev) => [...prev, { role: 'assistant', content: '' }])

        await chatApi.complete(
          {
            model: pg.selectedModel,
            messages: requestMessages,
            stream: true,
            temperature: pg.temperature,
            max_tokens: effectiveMaxTokens,
            user: runTag,
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
        const response = await chatApi.complete(
          {
            model: pg.selectedModel,
            messages: requestMessages,
            stream: false,
            temperature: pg.temperature,
            max_tokens: effectiveMaxTokens,
            user: runTag,
          },
          undefined,
          abortControllerRef.current.signal
        )

        const { content, reasoning } = splitAssistantMessage(response)
        pg.setMessages((prev) => [...prev, {
          role: 'assistant',
          content,
          reasoning: reasoning || undefined,
        }])
      }

      await fetchDistributionForRun(runId, 1)
    } catch (error) {
      if (!isAbortError(error)) {
        const description =
          error instanceof ApiError
            ? getErrorMessage(error)
            : error instanceof Error
              ? error.message
              : 'Unknown error'

        toast({ title: 'Failed to send message', description, variant: 'destructive' })
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

  const startLoadTest = async () => {
    // Load Test は admin ロール限定（バックエンドの専用エンドポイントでも強制）。
    if (!isAdmin || !pg.selectedModel || isLoadTesting) return

    const totalRequests = Math.max(1, toNumberValue(loadTestTotalRequests, DEFAULT_LOAD_TEST_SETTINGS.totalRequests))
    const concurrency = Math.max(1, toNumberValue(loadTestConcurrency, DEFAULT_LOAD_TEST_SETTINGS.concurrency))
    const intervalMs = Math.max(0, toNumberValue(loadTestIntervalMs, DEFAULT_LOAD_TEST_SETTINGS.intervalMs))

    const prompt = pg.input.trim() || 'Load balancing validation request'
    const runId = generateRunId()

    setIsLoadTesting(true)
    setIsStoppingLoadTest(false)
    setLoadTestProgress({ total: totalRequests, completed: 0, success: 0, error: 0 })
    setDistributionSummary(null)
    setDistributionError(null)

    loadTestStopRef.current = false

    let nextIndex = 0
    let completed = 0
    let success = 0
    let error = 0

    const worker = async () => {
      while (true) {
        if (loadTestStopRef.current) return

        const index = nextIndex
        if (index >= totalRequests) return
        nextIndex += 1

        const runTag = `lbpg:${runId}:${index + 1}`
        const requestMessages: ChatMessage[] = pg.systemPrompt
          ? [
              { role: 'system', content: pg.systemPrompt },
              { role: 'user', content: prompt },
            ]
          : [{ role: 'user', content: prompt }]
        const requestAbortController = new AbortController()
        loadTestAbortControllersRef.current.add(requestAbortController)

        try {
          await chatApi.completeLoadTest(
            {
              model: pg.selectedModel,
              messages: requestMessages,
              stream: false,
              temperature: pg.temperature,
              max_tokens: effectiveMaxTokens,
              user: runTag,
            },
            requestAbortController.signal
          )
          success += 1
        } catch (requestError) {
          if (!isAbortError(requestError)) {
            error += 1
          }
        } finally {
          loadTestAbortControllersRef.current.delete(requestAbortController)
          completed += 1
          if (isMountedRef.current) {
            setLoadTestProgress({ total: totalRequests, completed, success, error })
          }
        }

        if (intervalMs > 0) {
          await delay(intervalMs)
        }
      }
    }

    try {
      const workers = Array.from({ length: concurrency }, () => worker())
      await Promise.all(workers)

      if (!isMountedRef.current) return

      const finalCount = completed
      await fetchDistributionForRun(runId, finalCount)

      pg.setMessages((prev) => [
        ...prev,
        {
          role: 'assistant',
          content: `Load test finished. requests=${finalCount}, success=${success}, error=${error}`,
        },
      ])
    } finally {
      loadTestAbortControllersRef.current.clear()
      if (isMountedRef.current) {
        setIsLoadTesting(false)
        setIsStoppingLoadTest(false)
      }
      loadTestStopRef.current = false
    }
  }

  const stopLoadTest = () => {
    if (!isLoadTesting) return
    loadTestStopRef.current = true
    loadTestAbortControllersRef.current.forEach((controller) => controller.abort())
    loadTestAbortControllersRef.current.clear()
    setIsStoppingLoadTest(true)
  }

  const generateCurl = () => {
    const requestMessages = pg.systemPrompt
      ? ([{ role: 'system', content: pg.systemPrompt } as ChatMessage].concat(pg.messages.map(transformMessage)))
      : pg.messages.map(transformMessage)

    return `curl -X POST '/api/dashboard/playground/chat/completions' \\
  -H 'Content-Type: application/json' \\
  -H 'X-CSRF-Token: <llmlb_csrf cookie value>' \\
  -b 'llmlb_jwt=<dashboard cookie>; llmlb_csrf=<csrf cookie>' \\
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

  const modelOptions = useMemo<OpenAIModel[]>(
    () => (Array.isArray(modelsData?.data) ? modelsData.data : []),
    [modelsData]
  )

  const canSendChat = (pg.input.trim().length > 0 || pg.attachments.length > 0) && !!pg.selectedModel
  const progressRate =
    loadTestProgress && loadTestProgress.total > 0
      ? Math.round((loadTestProgress.completed / loadTestProgress.total) * 100)
      : 0

  return {
    pg, isAdmin, mode, setMode,
    loadTestTotalRequests, setLoadTestTotalRequests,
    loadTestConcurrency, setLoadTestConcurrency,
    loadTestIntervalMs, setLoadTestIntervalMs,
    isLoadTesting, isStoppingLoadTest, loadTestProgress,
    loadTestProgressLabel: loadTestProgress
      ? `${loadTestProgress.completed}/${loadTestProgress.total} completed` : '',
    loadTestOutcomeLabel: loadTestProgress
      ? `success=${loadTestProgress.success}, error=${loadTestProgress.error}` : '',
    distributionSummary: distributionSummary ? {
      label: `Run: ${distributionSummary.runId.slice(0, 8)} | Matched: ${distributionSummary.matchedCount}/${distributionSummary.expectedCount} | Updated: ${new Date(distributionSummary.updatedAt).toLocaleTimeString()}`,
      rows: distributionSummary.rows,
    } : null,
    distributionError, isRefreshingDistribution,
    refreshDistribution: async () => {
      if (distributionSummary) {
        await fetchDistributionForRun(distributionSummary.runId, distributionSummary.expectedCount)
      }
    },
    modelOptions, isLoadingModels,
    refetchModels: async () => { await refetchModelsQuery() },
    selectedModelMaxTokens, canSendChat, progressRate,
    sendMessage, startLoadTest, stopLoadTest,
    curlCommand: generateCurl(),
  }
}
