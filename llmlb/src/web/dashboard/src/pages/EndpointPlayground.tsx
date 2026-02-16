import { useState, useEffect, useRef, useCallback } from 'react'
import { useQuery } from '@tanstack/react-query'
import { endpointsApi, chatApi, ApiError, type DashboardEndpoint } from '@/lib/api'
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
  Cpu,
  MessageSquare,
  Trash2,
  Send,
  Settings,
  Copy,
  User,
  Bot,
  Loader2,
  RefreshCw,
  ExternalLink,
  Code,
  Check,
  CircleDot,
  RotateCcw,
  ArrowLeft,
  Image as ImageIcon,
  Mic,
  X,
  Volume2,
} from 'lucide-react'

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

interface EndpointPlaygroundProps {
  endpointId: string
  onBack: () => void
}

function getErrorMessage(status: number): string {
  switch (status) {
    case 401:
      return 'Invalid API key. Please check your settings.'
    case 403:
      return 'Access denied to this resource.'
    case 404:
      return 'API endpoint not found.'
    case 503:
      return 'No available endpoints. Please start an endpoint.'
    case 504:
      return 'Request timed out.'
    default:
      return `Server error occurred (HTTP ${status})`
  }
}

function getStatusBadgeVariant(
  status: DashboardEndpoint['status'] | undefined
): 'online' | 'pending' | 'offline' | 'destructive' | 'outline' {
  switch (status) {
    case 'online':
      return 'online'
    case 'pending':
      return 'pending'
    case 'offline':
      return 'offline'
    case 'error':
      return 'destructive'
    default:
      return 'outline'
  }
}

function getStatusIndicatorColor(status: DashboardEndpoint['status'] | undefined): string {
  switch (status) {
    case 'online':
      return 'text-success'
    case 'pending':
      return 'text-warning'
    case 'offline':
      // SPEC-66555000: offline should be a lighter red than error.
      return 'text-destructive/70'
    case 'error':
      return 'text-destructive'
    default:
      return 'text-muted-foreground'
  }
}

function getStatusLabel(status: DashboardEndpoint['status'] | undefined): string {
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

export default function EndpointPlayground({ endpointId, onBack }: EndpointPlaygroundProps) {
  const [messages, setMessages] = useState<Message[]>([])
  const [input, setInput] = useState('')
  const [isStreaming, setIsStreaming] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [curlOpen, setCurlOpen] = useState(false)
  const [copied, setCopied] = useState(false)

  // Settings state
  const [selectedModel, setSelectedModel] = useState('')
  const [systemPrompt, setSystemPrompt] = useState('')
  const [streamEnabled, setStreamEnabled] = useState(true)
  const [temperature, setTemperature] = useState(0.7)
  const [maxTokens, setMaxTokens] = useState(16384)
  const [useMaxContext, setUseMaxContext] = useState(false)

  const messagesEndRef = useRef<HTMLDivElement>(null)
  const abortControllerRef = useRef<AbortController | null>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const imageInputRef = useRef<HTMLInputElement>(null)
  const audioInputRef = useRef<HTMLInputElement>(null)

  const [attachments, setAttachments] = useState<MessageAttachment[]>([])

  // Fetch endpoint details
  const { data: endpoint, isLoading: isLoadingEndpoint } = useQuery({
    queryKey: ['endpoint', endpointId],
    queryFn: () => endpointsApi.get(endpointId),
  })

  // Fetch endpoint models
  const { data: endpointModels, isLoading: isLoadingModels, error: modelsError } = useQuery({
    queryKey: ['endpoint-models', endpointId],
    queryFn: () => endpointsApi.getModels(endpointId),
    retry: false,
  })

  // Notify on model fetch error
  useEffect(() => {
    if (modelsError) {
      let description = 'Failed to fetch model list'
      if (modelsError instanceof ApiError) {
        description = modelsError.message
      }
      toast({
        title: 'Error',
        description,
        variant: 'destructive',
      })
    }
  }, [modelsError])

  // Set default model when models are loaded
  useEffect(() => {
    if (endpointModels?.models && !selectedModel && endpointModels.models.length > 0) {
      setSelectedModel(endpointModels.models[0].model_id)
    }
  }, [endpointModels, selectedModel])

  const selectedModelMaxTokens = endpointModels?.models?.find(m => m.model_id === selectedModel)?.max_tokens
  const effectiveMaxTokens = useMaxContext && selectedModelMaxTokens != null ? selectedModelMaxTokens : maxTokens

  // Scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  // Reset chat
  const resetChat = () => {
    setMessages([])
    setAttachments([])
    toast({ title: 'Chat cleared' })
  }

  // Handle file attachments
  const handleFileAttachment = async (file: File, type: 'image' | 'audio') => {
    if (!file) return

    if (file.size > 4 * 1024 * 1024) {
      toast({ title: 'File too large', description: 'Maximum size is 4MB', variant: 'destructive' })
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

  // Extract media from content
  const extractMediaFromContent = (content: string) => {
    const imageUrlRegex = /(data:image\/[^;]+;base64,[^\s"'<>]+|https?:\/\/[^\s"'<>]+\.(png|jpg|jpeg|gif|webp))/gi
    const audioUrlRegex = /(data:audio\/[^;]+;base64,[^\s"'<>]+|https?:\/\/[^\s"'<>]+\.(mp3|wav|ogg|m4a))/gi

    const imageMatches = content.match(imageUrlRegex) || []
    const audioMatches = content.match(audioUrlRegex) || []

    return { imageMatches, audioMatches }
  }

  // Send message
  const sendMessage = async () => {
    if ((!input.trim() && attachments.length === 0) || !selectedModel || isStreaming) return

    const userMessage: Message = {
      role: 'user',
      content: input.trim(),
      attachments: attachments.length > 0 ? attachments : undefined
    }
    const newMessages = [...messages, userMessage]
    setMessages(newMessages)
    setInput('')
    setAttachments([])

    setIsStreaming(true)
    abortControllerRef.current = new AbortController()

    try {
      const transformMessage = (msg: Message) => {
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
          } else if (att.type === 'audio') {
            const audioData = att.data.startsWith('data:') ? att.data.split(',')[1] : att.data
            content.push({ type: 'input_audio', input_audio: { data: audioData, format: 'wav' } })
          }
        })
        return { role: msg.role, content }
      }

      const requestMessages = systemPrompt
        ? [{ role: 'system' as const, content: systemPrompt }, ...newMessages.map(transformMessage)]
        : newMessages.map(transformMessage)

      if (streamEnabled) {
        // ストリーミングモード: エンドポイントプロキシAPIを使用（JWT認証）
        let assistantContent = ''
        setMessages((prev) => [...prev, { role: 'assistant', content: '' }])

        await endpointsApi.chatCompletions(
          endpointId,
          {
            model: selectedModel,
            messages: requestMessages,
            stream: true,
            temperature,
            max_tokens: effectiveMaxTokens,
          },
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
          }
        )
      } else {
        // 非ストリーミングモード: エンドポイントプロキシAPIを使用（JWT認証）
        const data = await endpointsApi.chatCompletions(
          endpointId,
          {
            model: selectedModel,
            messages: requestMessages,
            stream: false,
            temperature,
            max_tokens: effectiveMaxTokens,
          }
        )

        const assistantMessage: Message = {
          role: 'assistant',
          content: data?.choices?.[0]?.message?.content || '',
        }

        setMessages((prev) => [...prev, assistantMessage])
      }
    } catch (error) {
      if ((error as Error).name !== 'AbortError') {
        toast({
          title: 'Failed to send message',
          description: error instanceof Error ? error.message : 'Unknown error',
          variant: 'destructive',
        })
        setMessages(messages)
      }
    } finally {
      setIsStreaming(false)
      abortControllerRef.current = null
      inputRef.current?.focus()
    }
  }

  // Stop generation
  const stopGeneration = () => {
    abortControllerRef.current?.abort()
    setIsStreaming(false)
  }

  // Generate cURL command (エンドポイントに直接リクエスト)
  const generateCurl = () => {
    const requestMessages = systemPrompt
      ? [{ role: 'system', content: systemPrompt }, ...messages]
      : messages

    if (!hasBaseUrl) {
      return '# Error: endpoint base_url is not configured. Please set it in the dashboard.'
    }

    return `curl -X POST '${baseUrl}/v1/chat/completions' \\
  -H 'Content-Type: application/json' \\
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

  // Copy to clipboard
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

  const models = endpointModels?.models || []
  const baseUrl = endpoint?.base_url?.replace(/\/$/, '') || ''
  const hasBaseUrl = baseUrl.length > 0

  if (isLoadingEndpoint) {
    return (
      <div className="flex h-screen w-full items-center justify-center bg-background">
        <div className="flex flex-col items-center gap-4">
          <Loader2 className="h-8 w-8 animate-spin text-primary" />
          <p className="text-sm text-muted-foreground">Loading endpoint...</p>
        </div>
      </div>
    )
  }

  return (
    <div className="flex h-screen bg-background">
      {/* Sidebar */}
      <div className="w-64 border-r flex flex-col">
        {/* Header */}
        <div className="p-4 border-b">
          <div className="flex items-center gap-2">
            <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary/10">
              <Cpu className="h-4 w-4 text-primary" />
            </div>
            <div>
              <h1 className="font-semibold text-sm truncate" title={endpoint?.name}>
                {endpoint?.name || 'Endpoint'}
              </h1>
              <p className="text-xs text-muted-foreground">Playground</p>
            </div>
          </div>
        </div>

        {/* Endpoint Info */}
        <div className="p-3 space-y-2">
          <div className="text-xs text-muted-foreground">
            <span className="font-medium">URL:</span>{' '}
            <span className="truncate block" title={endpoint?.base_url}>
              {hasBaseUrl ? endpoint?.base_url : '未設定'}
            </span>
          </div>
          {!hasBaseUrl && (
            <div className="text-xs text-destructive">
              base_url が未設定です。エンドポイント設定を確認してください。
            </div>
          )}
          <div className="text-xs text-muted-foreground">
            <span className="font-medium">Status:</span>{' '}
            <Badge variant={getStatusBadgeVariant(endpoint?.status)} className="text-xs">
              {getStatusLabel(endpoint?.status)}
            </Badge>
          </div>
          <div className="text-xs text-muted-foreground">
            <span className="font-medium">Models:</span> {models.length}
          </div>
        </div>

        <Separator />

        {/* Actions */}
        <div className="p-3 space-y-2">
          <Button
            variant="outline"
            className="w-full justify-start"
            onClick={onBack}
          >
            <ArrowLeft className="mr-2 h-4 w-4" />
            Back to Dashboard
          </Button>
          <Button
            variant="outline"
            className="w-full justify-start"
            onClick={() => setSettingsOpen(true)}
          >
            <Settings className="mr-2 h-4 w-4" />
            Settings
          </Button>
        </div>

        <div className="flex-1" />

        {/* Footer */}
        <div className="p-3 border-t">
          <Button
            variant="outline"
            size="sm"
            className="w-full"
            onClick={resetChat}
          >
            <Trash2 className="mr-2 h-4 w-4" />
            Clear Chat
          </Button>
        </div>
      </div>

      {/* Main Content */}
      <div className="flex-1 flex flex-col">
        {/* Chat Header */}
        <div className="h-14 border-b flex items-center justify-between px-4">
          <div className="flex items-center gap-3">
            <Select value={selectedModel} onValueChange={setSelectedModel}>
              <SelectTrigger className="w-64">
                <SelectValue placeholder="Select a model" />
              </SelectTrigger>
              <SelectContent>
                {isLoadingModels ? (
                  <SelectItem value="__loading__" disabled>
                    Loading models...
                  </SelectItem>
                ) : models.length === 0 ? (
                  <SelectItem value="__no_models__" disabled>
                    No models available
                  </SelectItem>
                ) : (
                  models.map((model) => (
                    <SelectItem key={model.model_id} value={model.model_id}>
                      {model.model_id}
                    </SelectItem>
                  ))
                )}
              </SelectContent>
            </Select>

            <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
              <CircleDot className={cn("h-3 w-3", getStatusIndicatorColor(endpoint?.status))} />
              {getStatusLabel(endpoint?.status)}
            </span>

            {streamEnabled && (
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

        {/* Messages */}
        <ScrollArea className="flex-1 p-4">
          <div className="max-w-3xl mx-auto space-y-4">
            {messages.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-64 text-center">
                <MessageSquare className="h-12 w-12 text-muted-foreground/50 mb-4" />
                <h2 className="text-lg font-medium">Start a conversation</h2>
                <p className="text-sm text-muted-foreground mt-1">
                  Select a model and send a message to get started.
                </p>
              </div>
            ) : (
              messages.map((message, index) => (
                <div
                  key={index}
                  className={cn(
                    'flex gap-3',
                    message.role === 'user' ? 'justify-end' : ''
                  )}
                >
                  {message.role === 'assistant' && (
                    <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary/10">
                      <Bot className="h-4 w-4 text-primary" />
                    </div>
                  )}
                  <div
                    className={cn(
                      'rounded-lg px-4 py-3 max-w-[80%] space-y-2',
                      message.role === 'user'
                        ? 'bg-primary text-primary-foreground'
                        : 'bg-muted'
                    )}
                  >
                    {message.content && (
                      <p className="text-sm whitespace-pre-wrap">{message.content}</p>
                    )}

                    {/* User attachments */}
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
                                <audio
                                  src={attachment.data}
                                  controls
                                  className="w-full max-w-[120px]"
                                />
                              </div>
                            )}
                          </div>
                        ))}
                      </div>
                    )}

                    {/* Assistant media output */}
                    {message.role === 'assistant' && (() => {
                      const { imageMatches, audioMatches } = extractMediaFromContent(message.content)
                      return (
                        <>
                          {imageMatches.length > 0 && (
                            <div className="grid grid-cols-2 gap-2 mt-2">
                              {imageMatches.map((url, idx) => (
                                <div key={idx} className="rounded-md overflow-hidden bg-black/20 p-1">
                                  <img
                                    src={url}
                                    alt={`assistant-image-${idx}`}
                                    className="w-full h-32 object-cover rounded-sm"
                                  />
                                </div>
                              ))}
                            </div>
                          )}
                          {audioMatches.length > 0 && (
                            <div className="space-y-2 mt-2">
                              {audioMatches.map((url, idx) => (
                                <div key={idx} className="rounded-md overflow-hidden bg-black/20 p-2 flex flex-col items-center justify-center gap-2">
                                  <Volume2 className="h-4 w-4" />
                                  <audio
                                    src={url}
                                    controls
                                    className="w-full max-w-[200px]"
                                  />
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

        {/* Input */}
        <div className="border-t p-4 bg-gradient-to-b from-background via-background to-muted/5">
          <div className="max-w-3xl mx-auto space-y-3">
            {/* Attachment Preview */}
            {attachments.length > 0 && (
              <div className="flex flex-wrap gap-2 p-3 rounded-lg bg-muted/40 border border-muted-foreground/10">
                {attachments.map((attachment, idx) => (
                  <div
                    key={idx}
                    className="relative group inline-block rounded-md overflow-hidden bg-background border border-border"
                  >
                    {attachment.type === 'image' && (
                      <>
                        <img
                          src={attachment.data}
                          alt={attachment.name}
                          className="h-16 w-16 object-cover"
                        />
                        <div className="absolute inset-0 bg-black/0 group-hover:bg-black/40 transition-colors flex items-center justify-center opacity-0 group-hover:opacity-100">
                          <Button
                            variant="destructive"
                            size="icon"
                            onClick={() => removeAttachment(idx)}
                            className="rounded-full p-1 h-auto w-auto"
                          >
                            <X className="h-3 w-3" />
                          </Button>
                        </div>
                      </>
                    )}
                    {attachment.type === 'audio' && (
                      <div className="h-16 w-16 flex flex-col items-center justify-center bg-muted gap-1">
                        <Mic className="h-5 w-5 text-muted-foreground" />
                        <Button
                          variant="destructive"
                          size="icon"
                          onClick={() => removeAttachment(idx)}
                          className="absolute -top-2 -right-2 rounded-full p-0.5 h-auto w-auto"
                        >
                          <X className="h-3 w-3" />
                        </Button>
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}

            {/* Input with attachment buttons */}
            <div className="flex gap-2">
              <input
                ref={imageInputRef}
                type="file"
                accept="image/*"
                className="hidden"
                onChange={(e) => e.target.files?.[0] && handleFileAttachment(e.target.files[0], 'image')}
              />
              <input
                ref={audioInputRef}
                type="file"
                accept="audio/*"
                className="hidden"
                onChange={(e) => e.target.files?.[0] && handleFileAttachment(e.target.files[0], 'audio')}
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

              <Input
                ref={inputRef}
                placeholder="Type a message or attach files..."
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
                    e.preventDefault()
                    sendMessage()
                  }
                }}
                onPaste={(e) => {
                  const files = e.clipboardData?.files
                  if (files) {
                    for (const file of Array.from(files)) {
                      const type = file.type.startsWith('audio/') ? 'audio' : file.type.startsWith('image/') ? 'image' : null
                      if (type) {
                        handleFileAttachment(file, type)
                      }
                    }
                  }
                }}
                disabled={isStreaming}
                className="flex-1"
              />

              {isStreaming ? (
                <Button variant="destructive" onClick={stopGeneration} className="shrink-0">
                  <RefreshCw className="mr-2 h-4 w-4 animate-spin" />
                  Stop
                </Button>
              ) : (
                <Button
                  onClick={sendMessage}
                  disabled={(!input.trim() && attachments.length === 0) || !selectedModel}
                  className="shrink-0"
                >
                  <Send className="mr-2 h-4 w-4" />
                  Send
                </Button>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Settings Dialog */}
      <Dialog open={settingsOpen} onOpenChange={setSettingsOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Settings</DialogTitle>
            <DialogDescription>
              Configure your chat preferences.
            </DialogDescription>
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
                  Stream responses as they're generated
                </p>
              </div>
              <Switch checked={streamEnabled} onCheckedChange={setStreamEnabled} />
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
                onChange={(e) => setTemperature(parseFloat(e.target.value))}
                className="w-full"
              />
            </div>
            <div className="space-y-2">
              <Label>Max Tokens</Label>
              <div className="flex items-center space-x-2 mb-2">
                <Checkbox
                  id="use-max-context"
                  checked={useMaxContext}
                  onCheckedChange={(checked) => setUseMaxContext(checked === true)}
                  disabled={selectedModelMaxTokens == null && !useMaxContext}
                />
                <Label
                  htmlFor="use-max-context"
                  className={cn("text-sm font-normal", selectedModelMaxTokens == null && "text-muted-foreground")}
                >
                  Use model max context{selectedModelMaxTokens != null ? ` (${selectedModelMaxTokens.toLocaleString()})` : ' (unknown)'}
                </Label>
              </div>
              <Input
                type="number"
                value={useMaxContext && selectedModelMaxTokens != null ? selectedModelMaxTokens : maxTokens}
                onChange={(e) => setMaxTokens(parseInt(e.target.value) || 2048)}
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

      {/* cURL Dialog */}
      <Dialog open={curlOpen} onOpenChange={setCurlOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>cURL Command</DialogTitle>
            <DialogDescription>
              Copy this command to replicate the API call.
            </DialogDescription>
          </DialogHeader>
          <div className="relative">
            <Button
              variant="outline"
              size="sm"
              className="absolute right-2 top-2"
              onClick={() => void handleCopyCurl(generateCurl())}
              disabled={!hasBaseUrl}
            >
              {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
            </Button>
            <ScrollArea className="h-64 rounded-md border bg-muted">
              <pre className="p-4 text-xs font-mono">{generateCurl()}</pre>
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
