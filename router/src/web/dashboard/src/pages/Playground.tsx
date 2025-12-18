import { useState, useEffect, useRef, useCallback } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { chatApi, modelsApi, type ChatSession, type ChatMessage, type RegisteredModelView } from '@/lib/api'
import { cn } from '@/lib/utils'
import { toast } from '@/hooks/use-toast'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Separator } from '@/components/ui/separator'
import { Switch } from '@/components/ui/switch'
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
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  Cpu,
  MessageSquare,
  Plus,
  Trash2,
  Send,
  Settings,
  Copy,
  Moon,
  Sun,
  User,
  Bot,
  MoreVertical,
  Loader2,
  RefreshCw,
  ExternalLink,
  Code,
  Check,
  PanelLeftClose,
  PanelLeft,
  CircleDot,
  RotateCcw,
  Image as ImageIcon,
  Mic,
  X,
  Volume2,
} from 'lucide-react'

interface MessageAttachment {
  type: 'image' | 'audio'
  name: string
  data: string // base64 or data URL
  mimeType: string
}

interface Message {
  role: 'user' | 'assistant' | 'system'
  content: string
  attachments?: MessageAttachment[]
}

export default function Playground() {
  const queryClient = useQueryClient()
  const [theme, setTheme] = useState<'dark' | 'light'>('dark')
  const [sessions, setSessions] = useState<ChatSession[]>([])
  const [currentSessionId, setCurrentSessionId] = useState<string | null>(null)
  const [messages, setMessages] = useState<Message[]>([])
  const [input, setInput] = useState('')
  const [isStreaming, setIsStreaming] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [curlOpen, setCurlOpen] = useState(false)
  const [copied, setCopied] = useState(false)
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const [providerFilter, setProviderFilter] = useState<'all' | 'local' | 'cloud'>('all')

  // Settings state
  const [selectedModel, setSelectedModel] = useState('')
  const [systemPrompt, setSystemPrompt] = useState('')
  const [streamEnabled, setStreamEnabled] = useState(true)
  const [temperature, setTemperature] = useState(0.7)
  const [maxTokens, setMaxTokens] = useState(2048)
  const [apiKey, setApiKey] = useState(() => {
    if (typeof window !== 'undefined') {
      const stored = localStorage.getItem('llm-router-api-key')
      if (stored) return stored
      // ローカル環境ではsk_debugをデフォルトに（開発モード用）
      const isLocal = window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1'
      if (isLocal) return 'sk_debug'
    }
    return ''
  })

  const messagesEndRef = useRef<HTMLDivElement>(null)
  const abortControllerRef = useRef<AbortController | null>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const imageInputRef = useRef<HTMLInputElement>(null)
  const audioInputRef = useRef<HTMLInputElement>(null)

  // Multimodal attachment state
  const [attachments, setAttachments] = useState<MessageAttachment[]>([])

  // Fetch models
  const { data: models } = useQuery({
    queryKey: ['registered-models'],
    queryFn: modelsApi.getRegistered,
  })

  // Fetch sessions
  const { data: fetchedSessions } = useQuery({
    queryKey: ['chat-sessions'],
    queryFn: chatApi.getSessions,
  })

  useEffect(() => {
    if (fetchedSessions) {
      setSessions(fetchedSessions as ChatSession[])
    }
  }, [fetchedSessions])

  // Set default model (registered モデルのみから選択)
  useEffect(() => {
    if (models && !selectedModel) {
      const allModels = models as RegisteredModelView[]
      // Filter for registered models, but accept models with state or lifecycle_status
      const registeredModels = allModels.filter(m => {
        const isRegistered = m.lifecycle_status === 'registered' || (m as any).state === 'ready'
        return isRegistered
      })
      if (registeredModels.length > 0) {
        setSelectedModel(registeredModels[0].name)
      }
    }
  }, [models, selectedModel])

  // Scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  // Theme toggle
  const toggleTheme = () => {
    const newTheme = theme === 'dark' ? 'light' : 'dark'
    setTheme(newTheme)
    document.documentElement.classList.toggle('dark', newTheme === 'dark')
  }

  // Save API key to localStorage
  const handleApiKeyChange = (value: string) => {
    setApiKey(value)
    localStorage.setItem('llm-router-api-key', value)
  }

  // Create new session
  const createSession = useCallback(() => {
    const newSession: ChatSession = {
      id: crypto.randomUUID(),
      title: 'New Chat',
      messages: [],
      model: selectedModel,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    }
    setSessions((prev) => [newSession, ...prev])
    setCurrentSessionId(newSession.id)
    setMessages([])
  }, [selectedModel])

  // Load session
  const loadSession = (session: ChatSession) => {
    setCurrentSessionId(session.id)
    setMessages(session.messages || [])
    if (session.model) {
      setSelectedModel(session.model)
    }
  }

  // Delete session
  const deleteSession = (sessionId: string) => {
    setSessions((prev) => prev.filter((s) => s.id !== sessionId))
    if (currentSessionId === sessionId) {
      setCurrentSessionId(null)
      setMessages([])
    }
    toast({ title: 'Session deleted' })
  }

  // Reset/clear current chat
  const resetChat = () => {
    setMessages([])
    setAttachments([])
    if (currentSessionId) {
      setSessions((prev) =>
        prev.map((s) =>
          s.id === currentSessionId
            ? { ...s, messages: [], updated_at: new Date().toISOString() }
            : s
        )
      )
    }
    toast({ title: 'Chat cleared' })
  }

  // Handle file attachments
  const handleFileAttachment = async (file: File, type: 'image' | 'audio') => {
    if (!file) return

    // Size limit: 4MB
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

  // Extract and render media from assistant message content
  const extractMediaFromContent = (content: string) => {
    const imageUrlRegex = /(data:image\/[^;]+;base64,[^\s"'<>]+|https?:\/\/[^\s"'<>]+\.(png|jpg|jpeg|gif|webp))/gi
    const audioUrlRegex = /(data:audio\/[^;]+;base64,[^\s"'<>]+|https?:\/\/[^\s"'<>]+\.(mp3|wav|ogg|m4a))/gi

    const imageMatches = content.match(imageUrlRegex) || []
    const audioMatches = content.match(audioUrlRegex) || []

    return { imageMatches, audioMatches }
  }

  // Toggle sidebar
  const toggleSidebar = () => {
    setSidebarCollapsed((prev) => !prev)
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

    // Update session
    if (currentSessionId) {
      setSessions((prev) =>
        prev.map((s) =>
          s.id === currentSessionId
            ? {
                ...s,
                messages: newMessages,
                title: newMessages.length === 1 ? input.trim().slice(0, 30) : s.title,
                updated_at: new Date().toISOString(),
              }
            : s
        )
      )
    }

    setIsStreaming(true)
    abortControllerRef.current = new AbortController()

    try {
      // Transform messages to multimodal format if attachments exist
      const transformMessage = (msg: Message) => {
        if (!msg.attachments || msg.attachments.length === 0) {
          return { role: msg.role, content: msg.content }
        }
        // Build multimodal content array
        const content: Array<any> = []
        if (msg.content.trim()) {
          content.push({ type: 'text', text: msg.content })
        }
        msg.attachments.forEach((att) => {
          if (att.type === 'image') {
            content.push({ type: 'image_url', image_url: { url: att.data } })
          } else if (att.type === 'audio') {
            // Extract base64 data from data URL if needed
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
        // Streaming response
        const headers: Record<string, string> = {
          'Content-Type': 'application/json',
        }
        if (apiKey) {
          headers['Authorization'] = `Bearer ${apiKey}`
        }

        const response = await fetch('/v1/chat/completions', {
          method: 'POST',
          headers,
          body: JSON.stringify({
            model: selectedModel,
            messages: requestMessages,
            stream: true,
            temperature,
            max_tokens: maxTokens,
          }),
          signal: abortControllerRef.current.signal,
        })

        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`)
        }

        const reader = response.body?.getReader()
        if (!reader) throw new Error('No response body')

        const decoder = new TextDecoder()
        let assistantContent = ''

        // Add placeholder message
        setMessages((prev) => [...prev, { role: 'assistant', content: '' }])

        while (true) {
          const { done, value } = await reader.read()
          if (done) break

          const chunk = decoder.decode(value, { stream: true })
          const lines = chunk.split('\n').filter((line) => line.startsWith('data: '))

          for (const line of lines) {
            const data = line.slice(6)
            if (data === '[DONE]') continue

            try {
              const parsed = JSON.parse(data)
              const delta = parsed.choices?.[0]?.delta?.content || ''
              assistantContent += delta

              setMessages((prev) => {
                const updated = [...prev]
                updated[updated.length - 1] = {
                  role: 'assistant',
                  content: assistantContent,
                }
                return updated
              })
            } catch {
              // Skip invalid JSON
            }
          }
        }

        // Update session with final messages
        if (currentSessionId) {
          setSessions((prev) =>
            prev.map((s) =>
              s.id === currentSessionId
                ? {
                    ...s,
                    messages: [...newMessages, { role: 'assistant', content: assistantContent }],
                    updated_at: new Date().toISOString(),
                  }
                : s
            )
          )
        }
      } else {
        // Non-streaming response
        const nonStreamHeaders: Record<string, string> = {
          'Content-Type': 'application/json',
        }
        if (apiKey) {
          nonStreamHeaders['Authorization'] = `Bearer ${apiKey}`
        }

        const response = await fetch('/v1/chat/completions', {
          method: 'POST',
          headers: nonStreamHeaders,
          body: JSON.stringify({
            model: selectedModel,
            messages: requestMessages,
            stream: false,
            temperature,
            max_tokens: maxTokens,
          }),
          signal: abortControllerRef.current.signal,
        })

        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`)
        }

        const data = await response.json()
        const assistantMessage: Message = {
          role: 'assistant',
          content: data.choices?.[0]?.message?.content || '',
        }

        setMessages((prev) => [...prev, assistantMessage])

        // Update session
        if (currentSessionId) {
          setSessions((prev) =>
            prev.map((s) =>
              s.id === currentSessionId
                ? {
                    ...s,
                    messages: [...newMessages, assistantMessage],
                    updated_at: new Date().toISOString(),
                  }
                : s
            )
          )
        }
      }
    } catch (error) {
      if ((error as Error).name !== 'AbortError') {
        toast({
          title: 'Failed to send message',
          description: error instanceof Error ? error.message : 'Unknown error',
          variant: 'destructive',
        })
        // Remove user message on error
        setMessages(messages)
      }
    } finally {
      setIsStreaming(false)
      abortControllerRef.current = null
      // 送信完了後に入力欄にフォーカスを戻す
      inputRef.current?.focus()
    }
  }

  // Stop generation
  const stopGeneration = () => {
    abortControllerRef.current?.abort()
    setIsStreaming(false)
  }

  // Generate cURL command
  const generateCurl = () => {
    const requestMessages = systemPrompt
      ? [{ role: 'system', content: systemPrompt }, ...messages]
      : messages

    const authHeader = apiKey ? `\n  -H 'Authorization: Bearer ${apiKey}' \\` : ''

    return `curl -X POST 'http://localhost:8080/v1/chat/completions' \\
  -H 'Content-Type: application/json' \\${authHeader}
  -d '${JSON.stringify(
    {
      model: selectedModel,
      messages: requestMessages,
      stream: streamEnabled,
      temperature,
      max_tokens: maxTokens,
    },
    null,
    2
  )}'`
  }

  // Copy to clipboard
  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
      toast({ title: 'Copied to clipboard' })
    } catch {
      toast({ title: 'Failed to copy', variant: 'destructive' })
    }
  }

  // ルーターにキャッシュ完了（registered）のモデルのみを選択可能にする
  const allModels = (models as RegisteredModelView[] | undefined) || []
  const availableModels = allModels.filter(m => m.lifecycle_status === 'registered')

  return (
    <div className="flex h-screen bg-background">
      {/* Sidebar */}
      <div
        id="sidebar"
        className={cn(
          'border-r flex flex-col transition-all duration-300',
          sidebarCollapsed ? 'w-0 overflow-hidden' : 'w-64'
        )}
      >
        {/* Header */}
        <div className="p-4 border-b">
          <div className="flex items-center gap-2">
            <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary/10">
              <Cpu className="h-4 w-4 text-primary" />
            </div>
            <div>
              <h1 className="font-semibold text-sm">LLM Router</h1>
              <p className="text-xs text-muted-foreground">Playground</p>
            </div>
          </div>
        </div>

        {/* New Chat Button */}
        <div className="p-3">
          <Button id="new-chat" className="w-full" onClick={createSession}>
            <Plus className="mr-2 h-4 w-4" />
            New Chat
          </Button>
        </div>

        {/* Sessions List */}
        <ScrollArea className="flex-1">
          <div id="session-list" className="p-2 space-y-1">
            {sessions.map((session) => (
              <div
                key={session.id}
                className={cn(
                  'group flex items-center gap-2 rounded-lg px-3 py-2 cursor-pointer hover:bg-muted/50',
                  currentSessionId === session.id && 'bg-muted'
                )}
                onClick={() => loadSession(session)}
              >
                <MessageSquare className="h-4 w-4 shrink-0 text-muted-foreground" />
                <span className="flex-1 truncate text-sm">{session.title}</span>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-6 w-6 opacity-0 group-hover:opacity-100"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <MoreVertical className="h-3 w-3" />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    <DropdownMenuItem
                      onClick={(e) => {
                        e.stopPropagation()
                        deleteSession(session.id)
                      }}
                      className="text-destructive"
                    >
                      <Trash2 className="mr-2 h-4 w-4" />
                      Delete
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>
            ))}
          </div>
        </ScrollArea>

        {/* Footer */}
        <div className="p-3 border-t space-y-2">
          <Button
            variant="outline"
            className="w-full justify-start"
            onClick={() => window.location.href = '/dashboard/'}
          >
            <ExternalLink className="mr-2 h-4 w-4" />
            Dashboard
          </Button>
          <div className="flex gap-2">
            <Button variant="ghost" size="icon" onClick={toggleTheme}>
              {theme === 'dark' ? (
                <Sun className="h-4 w-4" />
              ) : (
                <Moon className="h-4 w-4" />
              )}
            </Button>
            <Button
              id="settings-toggle"
              variant="ghost"
              size="icon"
              onClick={() => setSettingsOpen(true)}
            >
              <Settings className="h-4 w-4" />
            </Button>
          </div>
        </div>
      </div>

      {/* Main Content */}
      <div className="flex-1 flex flex-col">
        {/* Chat Header */}
        <div className="h-14 border-b flex items-center justify-between px-4">
          <div className="flex items-center gap-3">
            {/* Sidebar Toggle */}
            <Button
              id="sidebar-toggle"
              variant="ghost"
              size="icon"
              onClick={toggleSidebar}
              title={sidebarCollapsed ? 'Show sidebar' : 'Hide sidebar'}
            >
              {sidebarCollapsed ? (
                <PanelLeft className="h-4 w-4" />
              ) : (
                <PanelLeftClose className="h-4 w-4" />
              )}
            </Button>

            <Select value={selectedModel} onValueChange={setSelectedModel}>
              <SelectTrigger id="model-select" className="w-64">
                <SelectValue placeholder="Select a model" />
              </SelectTrigger>
              <SelectContent>
                {availableModels.length === 0 ? (
                  <SelectItem value="__no_models__" disabled>
                    No models available
                  </SelectItem>
                ) : (
                  availableModels
                    .filter((model) => model.name && model.name.length > 0)
                    .map((model) => (
                      <SelectItem key={model.name} value={model.name}>
                        {model.name}
                      </SelectItem>
                    ))
                )}
              </SelectContent>
            </Select>

            {/* Router Status */}
            <span id="router-status" className="flex items-center gap-1.5 text-xs text-muted-foreground">
              <CircleDot className="h-3 w-3 text-green-500" />
              Router: Online
            </span>

            {streamEnabled && (
              <Badge variant="secondary" className="text-xs">
                Streaming
              </Badge>
            )}
            {!apiKey && (
              <Badge variant="destructive" className="text-xs cursor-pointer" onClick={() => setSettingsOpen(true)}>
                API Key Required
              </Badge>
            )}
          </div>
          <Button id="copy-curl" variant="outline" size="sm" onClick={() => setCurlOpen(true)}>
            <Code className="mr-2 h-4 w-4" />
            cURL
          </Button>
        </div>

        {/* Messages */}
        <ScrollArea className="flex-1 p-4">
          <div id="chat-log" className="max-w-3xl mx-auto space-y-4">
            {messages.length === 0 ? (
              <div className="chat-welcome flex flex-col items-center justify-center h-64 text-center">
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
                    message.role === 'user' ? 'message--user justify-end' : 'message--assistant'
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
                      <p className="message-text text-sm whitespace-pre-wrap">{message.content}</p>
                    )}

                    {/* User attachments (input) */}
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

                    {/* Assistant media output (detected from content) */}
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
                                    data-testid="playground-assistant-image"
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
                                    data-testid="playground-assistant-audio"
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
          <div id="chat-form" className="max-w-3xl mx-auto space-y-3">
            {/* Attachment Preview */}
            {attachments.length > 0 && (
              <div className="flex flex-wrap gap-2 p-3 rounded-lg bg-muted/40 border border-muted-foreground/10" data-testid="playground-attachment-preview">
                {attachments.map((attachment, idx) => (
                  <div
                    key={idx}
                    className="relative group inline-block rounded-md overflow-hidden bg-background border border-border"
                    data-testid={attachment.type === 'image' ? 'playground-attachment-image' : 'playground-attachment-audio'}
                  >
                    {attachment.type === 'image' && (
                      <>
                        <img
                          src={attachment.data}
                          alt={attachment.name}
                          className="h-16 w-16 object-cover"
                        />
                        <div className="absolute inset-0 bg-black/0 group-hover:bg-black/40 transition-colors flex items-center justify-center opacity-0 group-hover:opacity-100">
                          <button
                            onClick={() => removeAttachment(idx)}
                            className="text-white bg-destructive rounded-full p-1 hover:bg-destructive/80"
                            data-testid="playground-attachment-remove"
                          >
                            <X className="h-3 w-3" />
                          </button>
                        </div>
                      </>
                    )}
                    {attachment.type === 'audio' && (
                      <div className="h-16 w-16 flex flex-col items-center justify-center bg-muted gap-1">
                        <Mic className="h-5 w-5 text-muted-foreground" />
                        <button
                          onClick={() => removeAttachment(idx)}
                          className="absolute -top-2 -right-2 text-white bg-destructive rounded-full p-0.5 hover:bg-destructive/80"
                          data-testid="playground-attachment-remove"
                        >
                          <X className="h-3 w-3" />
                        </button>
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}

            {/* Input with attachment buttons */}
            <div className="flex gap-2">
              {/* Hidden file inputs */}
              <input
                ref={imageInputRef}
                id="playground-image-input"
                data-testid="playground-image-input"
                type="file"
                accept="image/*"
                className="hidden"
                onChange={(e) => e.target.files?.[0] && handleFileAttachment(e.target.files[0], 'image')}
              />
              <input
                ref={audioInputRef}
                id="playground-audio-input"
                data-testid="playground-audio-input"
                type="file"
                accept="audio/*"
                className="hidden"
                onChange={(e) => e.target.files?.[0] && handleFileAttachment(e.target.files[0], 'audio')}
              />

              {/* Attachment buttons */}
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

              {/* Text input */}
              <Input
                ref={inputRef}
                id="chat-input"
                data-testid="playground-chat-input"
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

              {/* Send/Stop button */}
              {isStreaming ? (
                <Button id="stop-button" variant="destructive" onClick={stopGeneration} className="shrink-0">
                  <RefreshCw className="mr-2 h-4 w-4 animate-spin" />
                  Stop
                </Button>
              ) : (
                <Button
                  id="send-button"
                  data-testid="playground-send"
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
        <DialogContent id="settings-modal">
          <DialogHeader>
            <DialogTitle>Settings</DialogTitle>
            <DialogDescription>
              Configure your chat preferences.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label>API Key</Label>
              <Input
                id="api-key-input"
                type="password"
                placeholder="sk-..."
                value={apiKey}
                onChange={(e) => handleApiKeyChange(e.target.value)}
              />
              <p className="text-xs text-muted-foreground">
                Required for OpenAI compatible API authentication
              </p>
            </div>
            <Separator />
            <div className="space-y-2">
              <Label>Model Provider Filter</Label>
              <div className="flex gap-2">
                <button
                  className={cn(
                    'provider-btn flex-1 px-3 py-1.5 text-sm rounded-md border transition-colors',
                    providerFilter === 'local' ? 'provider-btn--active bg-primary text-primary-foreground' : 'bg-muted'
                  )}
                  data-provider="local"
                  onClick={() => setProviderFilter('local')}
                >
                  Local
                </button>
                <button
                  className={cn(
                    'provider-btn flex-1 px-3 py-1.5 text-sm rounded-md border transition-colors',
                    providerFilter === 'cloud' ? 'provider-btn--active bg-primary text-primary-foreground' : 'bg-muted'
                  )}
                  data-provider="cloud"
                  onClick={() => setProviderFilter('cloud')}
                >
                  Cloud
                </button>
                <button
                  className={cn(
                    'provider-btn flex-1 px-3 py-1.5 text-sm rounded-md border transition-colors',
                    providerFilter === 'all' ? 'provider-btn--active bg-primary text-primary-foreground' : 'bg-muted'
                  )}
                  data-provider="all"
                  onClick={() => setProviderFilter('all')}
                >
                  All
                </button>
              </div>
              <p className="text-xs text-muted-foreground">
                Filter models by provider type
              </p>
            </div>
            <Separator />
            <div className="space-y-2">
              <Label>System Prompt</Label>
              <Textarea
                id="system-prompt"
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
              <Switch id="stream-toggle" checked={streamEnabled} onCheckedChange={setStreamEnabled} />
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
              <Input
                type="number"
                value={maxTokens}
                onChange={(e) => setMaxTokens(parseInt(e.target.value) || 2048)}
                min={1}
                max={32000}
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div>
                <Label>Clear Chat</Label>
                <p className="text-xs text-muted-foreground">
                  Clear all messages in current session
                </p>
              </div>
              <Button
                id="reset-chat"
                variant="outline"
                size="sm"
                onClick={resetChat}
              >
                <RotateCcw className="mr-2 h-4 w-4" />
                Reset
              </Button>
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
              variant="ghost"
              size="sm"
              className="absolute right-2 top-2"
              onClick={() => copyToClipboard(generateCurl())}
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
