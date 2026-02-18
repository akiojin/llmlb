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
import { copyToClipboard, cn } from '@/lib/utils'
import { toast } from '@/hooks/use-toast'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Separator } from '@/components/ui/separator'
import { Switch } from '@/components/ui/switch'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Network,
  MessageSquare,
  Trash2,
  Send,
  Settings,
  Copy,
  User,
  Bot,
  Loader2,
  Code,
  Check,
  ArrowLeft,
  Image as ImageIcon,
  Mic,
  X,
  Volume2,
  Gauge,
  Play,
  Square,
  KeyRound,
  AlertCircle,
  RefreshCw,
} from 'lucide-react'

const API_KEY_STORAGE_KEY = 'lb_playground_api_key'
const MAX_ATTACHMENT_BYTES = 4 * 1024 * 1024

const DEFAULT_LOAD_TEST_SETTINGS = {
  totalRequests: 200,
  concurrency: 10,
  intervalMs: 0,
}

type PlaygroundMode = 'chat' | 'load_test'

interface MessageAttachment {
  type: 'image' | 'audio'
  name: string
  data: string
  mimeType: string
}

interface Message {
  role: 'user' | 'assistant' | 'system'
  content: string
  attachments?: MessageAttachment[]
}

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

interface LoadBalancerPlaygroundProps {
  onBack: () => void
  initialModel?: string
}

function getErrorMessage(status: number): string {
  switch (status) {
    case 401:
      return 'Invalid API key. Please check your settings.'
    case 403:
      return 'Access denied to this resource.'
    case 404:
      return 'API endpoint not found.'
    case 429:
      return 'Rate limited. Please reduce request volume.'
    case 503:
      return 'No available endpoints. Please check endpoint status.'
    case 504:
      return 'Request timed out.'
    default:
      return `Server error occurred (HTTP ${status})`
  }
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

function isAbortError(error: unknown): boolean {
  if (error instanceof DOMException) {
    return error.name === 'AbortError'
  }
  if (!error || typeof error !== 'object') {
    return false
  }
  const withName = error as { name?: unknown }
  return withName.name === 'AbortError'
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
    {
      count: number
      success: number
      error: number
      totalDuration: number
    }
  >()

  records.forEach((record) => {
    const endpoint = record.node_machine_name || record.node_id || 'unknown'
    const current = map.get(endpoint) ?? {
      count: 0,
      success: 0,
      error: 0,
      totalDuration: 0,
    }
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

function transformMessage(msg: Message): ChatMessage {
  if (!msg.attachments || msg.attachments.length === 0) {
    return { role: msg.role, content: msg.content }
  }

  const content: Array<unknown> = []
  if (msg.content.trim()) {
    content.push({ type: 'text', text: msg.content })
  }

  msg.attachments.forEach((att) => {
    if (att.type === 'image') {
      content.push({ type: 'image_url', image_url: { url: att.data } })
      return
    }

    const audioData = att.data.startsWith('data:') ? att.data.split(',')[1] : att.data
    content.push({ type: 'input_audio', input_audio: { data: audioData, format: 'wav' } })
  })

  return { role: msg.role, content }
}

export default function LoadBalancerPlayground({ onBack, initialModel }: LoadBalancerPlaygroundProps) {
  const [messages, setMessages] = useState<Message[]>([])
  const [input, setInput] = useState('')
  const [isStreaming, setIsStreaming] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [curlOpen, setCurlOpen] = useState(false)
  const [copied, setCopied] = useState(false)

  const [mode, setMode] = useState<PlaygroundMode>('chat')
  const [apiKey, setApiKey] = useState(() => {
    try {
      return localStorage.getItem(API_KEY_STORAGE_KEY) ?? ''
    } catch {
      return ''
    }
  })

  // Settings state
  const [selectedModel, setSelectedModel] = useState('')
  const [systemPrompt, setSystemPrompt] = useState('')
  const [streamEnabled, setStreamEnabled] = useState(true)
  const [temperature, setTemperature] = useState(0.7)
  const [maxTokens, setMaxTokens] = useState(16384)
  const [useMaxContext, setUseMaxContext] = useState(false)

  // Load test state
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

  const messagesEndRef = useRef<HTMLDivElement>(null)
  const isMountedRef = useRef(true)
  const abortControllerRef = useRef<AbortController | null>(null)
  const loadTestStopRef = useRef(false)
  const loadTestAbortControllersRef = useRef<Set<AbortController>>(new Set())
  const appliedInitialModelRef = useRef<string | null>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const imageInputRef = useRef<HTMLInputElement>(null)
  const audioInputRef = useRef<HTMLInputElement>(null)

  const [attachments, setAttachments] = useState<MessageAttachment[]>([])

  const hasApiKey = apiKey.trim().length > 0

  // Fetch load balancer models
  const {
    data: modelsData,
    isLoading: isLoadingModels,
    error: modelsError,
    refetch: refetchModels,
  } = useQuery<OpenAIModelsResponse>({
    queryKey: ['lb-playground-models', apiKey.trim()],
    queryFn: () => chatApi.getModels(apiKey.trim()),
    enabled: hasApiKey,
    retry: false,
    staleTime: 5000,
  })

  useEffect(() => {
    try {
      localStorage.setItem(API_KEY_STORAGE_KEY, apiKey)
    } catch {
      // Ignore storage failures
    }
  }, [apiKey])

  // Notify on model fetch error
  useEffect(() => {
    if (!modelsError) {
      return
    }

    let description = 'Failed to fetch model list'
    if (modelsError instanceof ApiError) {
      description = getErrorMessage(modelsError.status)
    } else if (modelsError instanceof Error) {
      description = modelsError.message
    }

    toast({
      title: 'Error',
      description,
      variant: 'destructive',
    })
  }, [modelsError])

  // SPEC-8795f98f: Apply initialModel from URL parameter
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
  }, [initialModel, modelsData?.data])

  // Keep selected model valid when model list changes
  useEffect(() => {
    const models = Array.isArray(modelsData?.data) ? modelsData.data : []
    const hasInitialModel = initialModel
      ? models.some((model) => model.id === initialModel)
      : false

    if (hasInitialModel) {
      return
    }

    if (models.length === 0) {
      if (selectedModel) {
        setSelectedModel('')
      }
      return
    }

    const hasSelectedModel = models.some((model) => model.id === selectedModel)
    if (!selectedModel || !hasSelectedModel) {
      setSelectedModel(models[0].id)
    }
  }, [modelsData, selectedModel, initialModel])

  const selectedModelMaxTokens = modelsData?.data?.find(m => m.id === selectedModel)?.max_tokens
  const effectiveMaxTokens = useMaxContext && selectedModelMaxTokens != null ? selectedModelMaxTokens : maxTokens

  // Scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  // Cleanup on unmount: abort any in-flight requests (chat + load-test) so we don't
  // keep hammering the backend after leaving the page.
  useEffect(() => {
    return () => {
      isMountedRef.current = false

      abortControllerRef.current?.abort()

      loadTestStopRef.current = true
      loadTestAbortControllersRef.current.forEach((controller) => {
        controller.abort()
      })
      loadTestAbortControllersRef.current.clear()
    }
  }, [])

  const clearApiKey = useCallback(() => {
    setApiKey('')
    setSelectedModel('')
    try {
      localStorage.removeItem(API_KEY_STORAGE_KEY)
    } catch {
      // Ignore storage failures
    }
    toast({ title: 'API key removed' })
  }, [])

  const resetChat = () => {
    setMessages([])
    setAttachments([])
    setDistributionSummary(null)
    setDistributionError(null)
    toast({ title: 'Chat cleared' })
  }

  const extractMediaFromContent = (content: string) => {
    const imageUrlRegex = /(data:image\/[^;]+;base64,[^\s"'<>]+|https?:\/\/[^\s"'<>]+\.(png|jpg|jpeg|gif|webp))/gi
    const audioUrlRegex = /(data:audio\/[^;]+;base64,[^\s"'<>]+|https?:\/\/[^\s"'<>]+\.(mp3|wav|ogg|m4a))/gi

    const imageMatches = content.match(imageUrlRegex) || []
    const audioMatches = content.match(audioUrlRegex) || []

    return { imageMatches, audioMatches }
  }

  const handleFileAttachment = async (file: File, type: 'image' | 'audio') => {
    if (!file) return

    if (file.size > MAX_ATTACHMENT_BYTES) {
      toast({
        title: 'File too large',
        description: 'Maximum size is 4MB',
        variant: 'destructive',
      })
      return
    }

    try {
      const reader = new FileReader()
      reader.onload = (e) => {
        const data = e.target?.result as string
        const newAttachment: MessageAttachment = {
          type,
          name: file.name,
          data,
          mimeType: file.type,
        }
        setAttachments((prev) => [...prev, newAttachment])
        toast({ title: `${type === 'image' ? 'Image' : 'Audio'} attached` })
      }
      reader.readAsDataURL(file)
    } catch {
      toast({ title: 'Failed to read file', variant: 'destructive' })
    }
  }

  const removeAttachment = (index: number) => {
    setAttachments((prev) => prev.filter((_, i) => i !== index))
  }

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

        if (response.records.length === 0) {
          break
        }

        for (const record of response.records) {
          const tag = extractUserTag(record.request_body)
          if (tag?.startsWith(`lbpg:${runId}:`)) {
            matched.push(record)
          }
        }

        if (matched.length >= expectedCount || (page + 1) * pageSize >= response.total_count) {
          break
        }
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
    if (!hasApiKey) {
      toast({
        title: 'API key required',
        description: 'Enter an API key to send requests through the load balancer.',
        variant: 'destructive',
      })
      return
    }

    if ((!input.trim() && attachments.length === 0) || !selectedModel || isStreaming) {
      return
    }

    const runId = generateRunId()
    const runTag = `lbpg:${runId}:1`

    const userMessage: Message = {
      role: 'user',
      content: input.trim(),
      attachments: attachments.length > 0 ? attachments : undefined,
    }
    const newMessages = [...messages, userMessage]

    setMessages(newMessages)
    setInput('')
    setAttachments([])
    setIsStreaming(true)

    abortControllerRef.current = new AbortController()

    try {
      const requestMessages = systemPrompt
        ? ([{ role: 'system', content: systemPrompt } as ChatMessage].concat(
            newMessages.map(transformMessage)
          ))
        : newMessages.map(transformMessage)

      if (streamEnabled) {
        let assistantContent = ''
        setMessages((prev) => [...prev, { role: 'assistant', content: '' }])

        await chatApi.complete(
          {
            model: selectedModel,
            messages: requestMessages,
            stream: true,
            temperature,
            max_tokens: effectiveMaxTokens,
            user: runTag,
          },
          apiKey.trim(),
          (chunk) => {
            assistantContent += chunk
            setMessages((prev) => {
              const updated = [...prev]
              updated[updated.length - 1] = {
                role: 'assistant',
                content: assistantContent,
              }
              return updated
            })
          },
          abortControllerRef.current.signal
        )
      } else {
        const response = await chatApi.complete(
          {
            model: selectedModel,
            messages: requestMessages,
            stream: false,
            temperature,
            max_tokens: effectiveMaxTokens,
            user: runTag,
          },
          apiKey.trim(),
          undefined,
          abortControllerRef.current.signal
        )

        const assistantMessage: Message = {
          role: 'assistant',
          content: response?.choices?.[0]?.message?.content || '',
        }

        setMessages((prev) => [...prev, assistantMessage])
      }

      await fetchDistributionForRun(runId, 1)
    } catch (error) {
      if ((error as Error).name !== 'AbortError') {
        const description =
          error instanceof ApiError
            ? getErrorMessage(error.status)
            : error instanceof Error
              ? error.message
              : 'Unknown error'

        toast({
          title: 'Failed to send message',
          description,
          variant: 'destructive',
        })
        setMessages(messages)
      }
    } finally {
      if (isMountedRef.current) {
        setIsStreaming(false)
      }
      abortControllerRef.current = null
      if (isMountedRef.current) {
        inputRef.current?.focus()
      }
    }
  }

  const startLoadTest = async () => {
    if (!hasApiKey) {
      toast({
        title: 'API key required',
        description: 'Enter an API key before running load tests.',
        variant: 'destructive',
      })
      return
    }

    if (!selectedModel || isLoadTesting) {
      return
    }

    const totalRequests = Math.max(1, toNumberValue(loadTestTotalRequests, DEFAULT_LOAD_TEST_SETTINGS.totalRequests))
    const concurrency = Math.max(1, toNumberValue(loadTestConcurrency, DEFAULT_LOAD_TEST_SETTINGS.concurrency))
    const intervalMs = Math.max(0, toNumberValue(loadTestIntervalMs, DEFAULT_LOAD_TEST_SETTINGS.intervalMs))

    const prompt = input.trim() || 'Load balancing validation request'
    const runId = generateRunId()

    setIsLoadTesting(true)
    setIsStoppingLoadTest(false)
    setLoadTestProgress({
      total: totalRequests,
      completed: 0,
      success: 0,
      error: 0,
    })
    setDistributionSummary(null)
    setDistributionError(null)

    loadTestStopRef.current = false

    let nextIndex = 0
    let completed = 0
    let success = 0
    let error = 0

    const worker = async () => {
      while (true) {
        if (loadTestStopRef.current) {
          return
        }

        const index = nextIndex
        if (index >= totalRequests) {
          return
        }
        nextIndex += 1

        const runTag = `lbpg:${runId}:${index + 1}`
        const requestMessages: ChatMessage[] = systemPrompt
          ? [
              { role: 'system', content: systemPrompt },
              { role: 'user', content: prompt },
            ]
          : [{ role: 'user', content: prompt }]
        const requestAbortController = new AbortController()
        loadTestAbortControllersRef.current.add(requestAbortController)

        try {
          await chatApi.complete(
            {
              model: selectedModel,
              messages: requestMessages,
              stream: false,
              temperature,
              max_tokens: effectiveMaxTokens,
              user: runTag,
            },
            apiKey.trim(),
            undefined,
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
            setLoadTestProgress({
              total: totalRequests,
              completed,
              success,
              error,
            })
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

      if (!isMountedRef.current) {
        return
      }

      const finalCount = completed
      await fetchDistributionForRun(runId, finalCount)

      setMessages((prev) => [
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

  const stopGeneration = () => {
    abortControllerRef.current?.abort()
    setIsStreaming(false)
  }

  const stopLoadTest = () => {
    if (!isLoadTesting) {
      return
    }
    loadTestStopRef.current = true
    loadTestAbortControllersRef.current.forEach((controller) => {
      controller.abort()
    })
    loadTestAbortControllersRef.current.clear()
    setIsStoppingLoadTest(true)
  }

  const generateCurl = () => {
    const requestMessages = systemPrompt
      ? ([{ role: 'system', content: systemPrompt } as ChatMessage].concat(messages.map(transformMessage)))
      : messages.map(transformMessage)

    if (!hasApiKey) {
      return '# Error: API key is required. Set API key and retry.'
    }

    return `curl -X POST '/v1/chat/completions' \\
  -H 'Content-Type: application/json' \\
  -H 'Authorization: Bearer ${apiKey.trim()}' \\
  -d '${JSON.stringify(
    {
      model: selectedModel,
      messages: requestMessages,
      stream: streamEnabled,
      temperature,
      max_tokens: effectiveMaxTokens,
    },
    null,
    2
  )}'`
  }

  const handleCopyCurl = async (text: string) => {
    try {
      await copyToClipboard(text)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
      toast({ title: 'Copied to clipboard' })
    } catch {
      toast({ title: 'Failed to copy', variant: 'destructive' })
    }
  }

  const modelOptions = useMemo<OpenAIModel[]>(
    () => (Array.isArray(modelsData?.data) ? modelsData.data : []),
    [modelsData]
  )

  const canSendChat = hasApiKey && (input.trim().length > 0 || attachments.length > 0) && !!selectedModel
  const progressRate =
    loadTestProgress && loadTestProgress.total > 0
      ? Math.round((loadTestProgress.completed / loadTestProgress.total) * 100)
      : 0

  return (
    <div className="flex h-screen bg-background">
      <div className="w-72 border-r flex flex-col" id="lb-playground-sidebar">
        <div className="p-4 border-b">
          <div className="flex items-center gap-2">
            <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary/10">
              <Network className="h-4 w-4 text-primary" />
            </div>
            <div>
              <h1 className="font-semibold text-sm">Load Balancer</h1>
              <p className="text-xs text-muted-foreground">Playground</p>
            </div>
          </div>
        </div>

        <div className="p-3 space-y-2">
          <div className="text-xs text-muted-foreground flex items-center gap-2">
            <KeyRound className="h-3 w-3" />
            <span className="font-medium">API Key:</span>
            <Badge variant={hasApiKey ? 'default' : 'secondary'}>
              {hasApiKey ? 'Configured' : 'Missing'}
            </Badge>
          </div>
          <div className="text-xs text-muted-foreground">
            <span className="font-medium">Models:</span> {modelOptions.length}
          </div>
          <div className="text-xs text-muted-foreground">
            <span className="font-medium">Mode:</span>{' '}
            {mode === 'chat' ? 'Interactive chat' : 'Load test'}
          </div>
        </div>

        <Separator />

        <div className="p-3 space-y-2">
          <Button variant="outline" className="w-full justify-start" onClick={onBack}>
            <ArrowLeft className="mr-2 h-4 w-4" />
            Back to Dashboard
          </Button>
          <Button variant="outline" className="w-full justify-start" onClick={() => setSettingsOpen(true)}>
            <Settings className="mr-2 h-4 w-4" />
            Settings
          </Button>
          <Button
            variant="outline"
            className="w-full justify-start"
            onClick={() => refetchModels()}
            disabled={!hasApiKey || isLoadingModels}
          >
            <RefreshCw className={cn('mr-2 h-4 w-4', isLoadingModels && 'animate-spin')} />
            Refresh Models
          </Button>
        </div>

        <div className="px-3 pb-3">
          <Label htmlFor="lb-api-key" className="text-xs">API Key (stored in localStorage)</Label>
          <div className="mt-2 flex gap-2">
            <Input
              id="lb-api-key"
              type="password"
              placeholder="sk-..."
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              className="h-8"
              autoComplete="off"
            />
            <Button size="sm" variant="outline" onClick={clearApiKey} disabled={!hasApiKey}>
              Clear
            </Button>
          </div>
          <p className="mt-2 text-[11px] leading-4 text-muted-foreground">
            The API key is saved as plain text in your browser storage.
          </p>
        </div>

        <div className="flex-1" />

        <div className="p-3 border-t">
          <Button variant="outline" size="sm" className="w-full" onClick={resetChat}>
            <Trash2 className="mr-2 h-4 w-4" />
            Clear Chat
          </Button>
        </div>
      </div>

      <div className="flex-1 flex flex-col">
        <div className="h-14 border-b flex items-center justify-between px-4">
          <div className="flex items-center gap-3">
            <div className="inline-flex rounded-md border border-border p-1">
              <Button
                size="sm"
                variant={mode === 'chat' ? 'default' : 'outline'}
                onClick={() => setMode('chat')}
                className="h-7"
                id="lb-mode-chat"
              >
                <MessageSquare className="mr-1.5 h-3.5 w-3.5" />
                Chat
              </Button>
              <Button
                size="sm"
                variant={mode === 'load_test' ? 'default' : 'outline'}
                onClick={() => setMode('load_test')}
                className="h-7"
                id="lb-mode-load-test"
              >
                <Gauge className="mr-1.5 h-3.5 w-3.5" />
                Load Test
              </Button>
            </div>

            <Select value={selectedModel} onValueChange={setSelectedModel}>
              <SelectTrigger className="w-80" id="lb-model-select">
                <SelectValue placeholder={hasApiKey ? 'Select a model' : 'Set API key first'} />
              </SelectTrigger>
              <SelectContent>
                {!hasApiKey ? (
                  <SelectItem value="__missing_key__" disabled>
                    API key required
                  </SelectItem>
                ) : isLoadingModels ? (
                  <SelectItem value="__loading__" disabled>
                    Loading models...
                  </SelectItem>
                ) : modelOptions.length === 0 ? (
                  <SelectItem value="__empty__" disabled>
                    No models available
                  </SelectItem>
                ) : (
                  modelOptions.map((model) => (
                    <SelectItem key={model.id} value={model.id}>
                      {model.id}
                    </SelectItem>
                  ))
                )}
              </SelectContent>
            </Select>

            {mode === 'chat' && streamEnabled && (
              <Badge variant="secondary" className="text-xs">
                Streaming
              </Badge>
            )}
          </div>

          <Button variant="outline" size="sm" onClick={() => setCurlOpen(true)}>
            <Code className="mr-2 h-4 w-4" />
            cURL
          </Button>
        </div>

        {(distributionSummary || distributionError || isRefreshingDistribution) && (
          <div className="border-b bg-muted/20 px-4 py-3" id="lb-distribution-panel">
            <div className="mb-2 flex items-center justify-between">
              <div className="flex items-center gap-2 text-sm font-medium">
                <Network className="h-4 w-4 text-primary" />
                Request Distribution
              </div>
              {distributionSummary && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() =>
                    fetchDistributionForRun(
                      distributionSummary.runId,
                      distributionSummary.expectedCount
                    )
                  }
                  disabled={isRefreshingDistribution}
                >
                  <RefreshCw className={cn('mr-2 h-3.5 w-3.5', isRefreshingDistribution && 'animate-spin')} />
                  Refresh
                </Button>
              )}
            </div>

            {distributionError && (
              <div className="flex items-center gap-2 text-sm text-destructive">
                <AlertCircle className="h-4 w-4" />
                {distributionError}
              </div>
            )}

            {distributionSummary && (
              <div className="space-y-2">
                <p className="text-xs text-muted-foreground" id="lb-distribution-summary">
                  Run: {distributionSummary.runId.slice(0, 8)} | Matched:{' '}
                  {distributionSummary.matchedCount}/{distributionSummary.expectedCount} | Updated:{' '}
                  {new Date(distributionSummary.updatedAt).toLocaleTimeString()}
                </p>
                <div className="rounded-md border bg-background">
                  <div className="grid grid-cols-[1fr_80px_80px_80px_120px] border-b px-3 py-2 text-xs font-medium text-muted-foreground">
                    <span>Endpoint</span>
                    <span className="text-right">Count</span>
                    <span className="text-right">Success</span>
                    <span className="text-right">Error</span>
                    <span className="text-right">Avg ms</span>
                  </div>
                  {distributionSummary.rows.length === 0 ? (
                    <p className="px-3 py-3 text-xs text-muted-foreground">
                      No records found for this run yet.
                    </p>
                  ) : (
                    distributionSummary.rows.map((row) => (
                      <div
                        key={row.endpoint}
                        className="grid grid-cols-[1fr_80px_80px_80px_120px] px-3 py-2 text-xs"
                        data-testid="lb-distribution-row"
                      >
                        <span className="truncate" title={row.endpoint}>
                          {row.endpoint}
                        </span>
                        <span className="text-right">{row.count}</span>
                        <span className="text-right text-green-600">{row.success}</span>
                        <span className="text-right text-destructive">{row.error}</span>
                        <span className="text-right">{row.averageDurationMs}</span>
                      </div>
                    ))
                  )}
                </div>
              </div>
            )}
          </div>
        )}

        <ScrollArea className="flex-1 p-4">
          <div className="max-w-4xl mx-auto space-y-4">
            {messages.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-64 text-center">
                <MessageSquare className="h-12 w-12 text-muted-foreground/50 mb-4" />
                <h2 className="text-lg font-medium">Start a load balancer conversation</h2>
                <p className="text-sm text-muted-foreground mt-1">
                  Configure an API key, choose a model, and send requests through /v1/chat/completions.
                </p>
              </div>
            ) : (
              messages.map((message, index) => (
                <div key={index} className={cn('flex gap-3', message.role === 'user' ? 'justify-end' : '')}>
                  {message.role === 'assistant' && (
                    <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary/10">
                      <Bot className="h-4 w-4 text-primary" />
                    </div>
                  )}
                  <div
                    className={cn(
                      'rounded-lg px-4 py-3 max-w-[80%] space-y-2',
                      message.role === 'user' ? 'bg-primary text-primary-foreground' : 'bg-muted'
                    )}
                  >
                    {message.content && <p className="text-sm whitespace-pre-wrap">{message.content}</p>}

                    {message.attachments && message.attachments.length > 0 && (
                      <div className="grid grid-cols-2 gap-2 mt-2">
                        {message.attachments.map((attachment, aIdx) => (
                          <div key={aIdx} className="rounded-md overflow-hidden bg-black/20 p-1">
                            {attachment.type === 'image' && (
                              <img
                                src={attachment.data}
                                alt={attachment.name}
                                className="w-full h-32 object-cover rounded-sm"
                              />
                            )}
                            {attachment.type === 'audio' && (
                              <div className="flex flex-col items-center justify-center h-32 gap-2">
                                <Volume2 className="h-6 w-6" />
                                <audio src={attachment.data} controls className="w-full max-w-[120px]" />
                              </div>
                            )}
                          </div>
                        ))}
                      </div>
                    )}

                    {message.role === 'assistant' && (() => {
                      const { imageMatches, audioMatches } = extractMediaFromContent(message.content)
                      return (
                        <>
                          {imageMatches.length > 0 && (
                            <div className="grid grid-cols-2 gap-2 mt-2">
                              {imageMatches.map((url, i) => (
                                <div key={`${url}-${i}`} className="rounded-md overflow-hidden bg-black/20 p-1">
                                  <img src={url} alt={`assistant-image-${i}`} className="w-full h-32 object-cover rounded-sm" />
                                </div>
                              ))}
                            </div>
                          )}
                          {audioMatches.length > 0 && (
                            <div className="space-y-2 mt-2">
                              {audioMatches.map((url, i) => (
                                <div key={`${url}-${i}`} className="rounded-md bg-black/20 p-2">
                                  <audio src={url} controls className="w-full" />
                                </div>
                              ))}
                            </div>
                          )}
                        </>
                      )
                    })()}
                  </div>
                  {message.role === 'user' && (
                    <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-muted">
                      <User className="h-4 w-4" />
                    </div>
                  )}
                </div>
              ))
            )}
            <div ref={messagesEndRef} />
          </div>
        </ScrollArea>

        <div className="border-t p-4 bg-gradient-to-b from-background via-background to-muted/5">
          <div className="max-w-4xl mx-auto space-y-3">
            {mode === 'load_test' && (
              <div className="rounded-lg border bg-muted/20 p-3 space-y-3" id="lb-load-test-settings">
                <div className="flex items-center justify-between">
                  <p className="text-sm font-medium">Load test settings</p>
                  <Badge variant="secondary">Default: High</Badge>
                </div>
                <div className="grid grid-cols-3 gap-3">
                  <div className="space-y-1">
                    <Label htmlFor="lb-total-requests" className="text-xs">Requests</Label>
                    <Input
                      id="lb-total-requests"
                      type="number"
                      min={1}
                      value={loadTestTotalRequests}
                      onChange={(e) => setLoadTestTotalRequests(e.target.value)}
                      disabled={isLoadTesting}
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="lb-concurrency" className="text-xs">Concurrency</Label>
                    <Input
                      id="lb-concurrency"
                      type="number"
                      min={1}
                      value={loadTestConcurrency}
                      onChange={(e) => setLoadTestConcurrency(e.target.value)}
                      disabled={isLoadTesting}
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="lb-interval-ms" className="text-xs">Interval (ms)</Label>
                    <Input
                      id="lb-interval-ms"
                      type="number"
                      min={0}
                      value={loadTestIntervalMs}
                      onChange={(e) => setLoadTestIntervalMs(e.target.value)}
                      disabled={isLoadTesting}
                    />
                  </div>
                </div>

                {loadTestProgress && (
                  <div className="space-y-2" id="lb-load-test-progress">
                    <div className="flex items-center justify-between text-xs text-muted-foreground">
                      <span>
                        {loadTestProgress.completed}/{loadTestProgress.total} completed
                      </span>
                      <span>
                        success={loadTestProgress.success}, error={loadTestProgress.error}
                      </span>
                    </div>
                    <div className="h-2 rounded-full bg-muted">
                      <div
                        className="h-2 rounded-full bg-primary transition-all"
                        style={{ width: `${progressRate}%` }}
                      />
                    </div>
                  </div>
                )}
              </div>
            )}

            {mode === 'chat' && attachments.length > 0 && (
              <div className="flex flex-wrap gap-2 p-3 rounded-lg bg-muted/40 border border-muted-foreground/10">
                {attachments.map((att, i) => (
                  <div key={`${att.name}-${i}`} className="relative group inline-block rounded-md overflow-hidden bg-background border border-border">
                    {att.type === 'image' ? (
                      <>
                        <img src={att.data} alt={att.name} className="h-16 w-16 object-cover" />
                        <div className="absolute inset-0 bg-black/0 group-hover:bg-black/40 transition-colors flex items-center justify-center opacity-0 group-hover:opacity-100">
                          <Button
                            variant="destructive"
                            size="icon"
                            onClick={() => removeAttachment(i)}
                            className="rounded-full p-1 h-auto w-auto"
                          >
                            <X className="h-3 w-3" />
                          </Button>
                        </div>
                      </>
                    ) : (
                      <div className="h-16 w-16 flex items-center justify-center bg-muted">
                        <Volume2 className="h-4 w-4" />
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}

            <div className="flex gap-2">
              {mode === 'chat' && (
                <>
                  <input
                    ref={imageInputRef}
                    type="file"
                    accept="image/*"
                    className="hidden"
                    onChange={(e) => {
                      const file = e.target.files?.[0]
                      if (file) {
                        handleFileAttachment(file, 'image')
                      }
                      e.currentTarget.value = ''
                    }}
                  />
                  <input
                    ref={audioInputRef}
                    type="file"
                    accept="audio/*"
                    className="hidden"
                    onChange={(e) => {
                      const file = e.target.files?.[0]
                      if (file) {
                        handleFileAttachment(file, 'audio')
                      }
                      e.currentTarget.value = ''
                    }}
                  />

                  <Button
                    variant="outline"
                    size="icon"
                    onClick={() => imageInputRef.current?.click()}
                    title="Attach image"
                    className="shrink-0"
                  >
                    <ImageIcon className="h-4 w-4" />
                  </Button>
                  <Button
                    variant="outline"
                    size="icon"
                    onClick={() => audioInputRef.current?.click()}
                    title="Attach audio"
                    className="shrink-0"
                  >
                    <Mic className="h-4 w-4" />
                  </Button>
                </>
              )}

              <Input
                ref={inputRef}
                placeholder={
                  mode === 'chat'
                    ? 'Type a message or attach files...'
                    : 'Prompt used for each load test request...'
                }
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
                    e.preventDefault()
                    if (mode === 'chat') {
                      void sendMessage()
                    }
                  }
                }}
                onPaste={(e) => {
                  if (mode !== 'chat') {
                    return
                  }
                  const files = e.clipboardData?.files
                  if (!files) {
                    return
                  }
                  for (const file of Array.from(files)) {
                    const type =
                      file.type.startsWith('audio/')
                        ? 'audio'
                        : file.type.startsWith('image/')
                          ? 'image'
                          : null
                    if (type) {
                      void handleFileAttachment(file, type)
                    }
                  }
                }}
                disabled={isStreaming || isLoadTesting}
                id="lb-chat-input"
              />

              {mode === 'chat' ? (
                isStreaming ? (
                  <Button variant="destructive" onClick={stopGeneration} className="shrink-0" id="lb-stop-chat">
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    Stop
                  </Button>
                ) : (
                  <Button
                    onClick={() => void sendMessage()}
                    disabled={!canSendChat}
                    className="shrink-0"
                    id="lb-send-chat"
                  >
                    <Send className="mr-2 h-4 w-4" />
                    Send
                  </Button>
                )
              ) : isLoadTesting ? (
                <Button
                  variant="destructive"
                  onClick={stopLoadTest}
                  className="shrink-0"
                  id="lb-stop-load-test"
                >
                  <Square className="mr-2 h-4 w-4" />
                  {isStoppingLoadTest ? 'Stopping...' : 'Stop'}
                </Button>
              ) : (
                <Button
                  onClick={() => void startLoadTest()}
                  disabled={!hasApiKey || !selectedModel}
                  className="shrink-0"
                  id="lb-start-load-test"
                >
                  <Play className="mr-2 h-4 w-4" />
                  Start Load Test
                </Button>
              )}
            </div>
          </div>
        </div>
      </div>

      <Dialog open={settingsOpen} onOpenChange={setSettingsOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Settings</DialogTitle>
            <DialogDescription>Configure chat behavior and generation parameters.</DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label>System Prompt</Label>
              <Textarea
                placeholder="You are a helpful assistant..."
                value={systemPrompt}
                onChange={(e) => setSystemPrompt(e.target.value)}
                rows={3}
              />
            </div>

            <Separator />

            <div className="flex items-center justify-between">
              <div>
                <Label>Streaming</Label>
                <p className="text-xs text-muted-foreground">
                  Stream responses while tokens are generated
                </p>
              </div>
              <Switch
                checked={streamEnabled}
                onCheckedChange={setStreamEnabled}
                disabled={mode === 'load_test'}
              />
            </div>

            <Separator />

            <div className="space-y-2">
              <Label>Temperature: {temperature}</Label>
              <input
                type="range"
                min="0"
                max="2"
                step="0.1"
                value={temperature}
                onChange={(e) => setTemperature(Number.parseFloat(e.target.value))}
                className="w-full"
              />
            </div>

            <div className="space-y-2">
              <Label>Max Tokens</Label>
              <div className="flex items-center space-x-2 mb-2">
                <Checkbox
                  id="lb-use-max-context"
                  checked={useMaxContext}
                  onCheckedChange={(checked) => setUseMaxContext(checked === true)}
                  disabled={selectedModelMaxTokens == null && !useMaxContext}
                />
                <Label
                  htmlFor="lb-use-max-context"
                  className={cn("text-sm font-normal", selectedModelMaxTokens == null && "text-muted-foreground")}
                >
                  Use model max context{selectedModelMaxTokens != null ? ` (${selectedModelMaxTokens.toLocaleString()})` : ' (unknown)'}
                </Label>
              </div>
              <Input
                type="number"
                value={useMaxContext && selectedModelMaxTokens != null ? selectedModelMaxTokens : maxTokens}
                onChange={(e) => setMaxTokens(Number.parseInt(e.target.value, 10) || 2048)}
                min={1}
                max={131072}
                disabled={useMaxContext && selectedModelMaxTokens != null}
              />
            </div>
          </div>

          <DialogFooter>
            <Button onClick={() => setSettingsOpen(false)}>Done</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={curlOpen} onOpenChange={setCurlOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>cURL Command</DialogTitle>
            <DialogDescription>
              Copy this command to replay the current request through the load balancer.
            </DialogDescription>
          </DialogHeader>

          <div className="relative">
            <Button
              variant="outline"
              size="sm"
              className="absolute right-2 top-2"
              onClick={() => void handleCopyCurl(generateCurl())}
            >
              {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
            </Button>
            <ScrollArea className="h-64 rounded-md border bg-muted">
              <pre className="p-4 text-xs font-mono whitespace-pre-wrap">{generateCurl()}</pre>
            </ScrollArea>
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => setCurlOpen(false)}>
              Close
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
