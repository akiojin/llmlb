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
} from 'lucide-react'

interface Message {
  role: 'user' | 'assistant' | 'system'
  content: string
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

  // Set default model
  useEffect(() => {
    if (models && !selectedModel) {
      const readyModels = (models as RegisteredModelView[]).filter((m) => m.ready)
      if (readyModels.length > 0) {
        setSelectedModel(readyModels[0].name)
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

  // Send message
  const sendMessage = async () => {
    if (!input.trim() || !selectedModel || isStreaming) return

    const userMessage: Message = { role: 'user', content: input.trim() }
    const newMessages = [...messages, userMessage]
    setMessages(newMessages)
    setInput('')

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
      const requestMessages = systemPrompt
        ? [{ role: 'system' as const, content: systemPrompt }, ...newMessages]
        : newMessages

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

  const readyModels = (models as RegisteredModelView[] | undefined)?.filter((m) => m.ready) || []

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
              <h1 className="font-semibold text-sm">LLM Router</h1>
              <p className="text-xs text-muted-foreground">Playground</p>
            </div>
          </div>
        </div>

        {/* New Chat Button */}
        <div className="p-3">
          <Button className="w-full" onClick={createSession}>
            <Plus className="mr-2 h-4 w-4" />
            New Chat
          </Button>
        </div>

        {/* Sessions List */}
        <ScrollArea className="flex-1">
          <div className="p-2 space-y-1">
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
            <Select value={selectedModel} onValueChange={setSelectedModel}>
              <SelectTrigger className="w-64">
                <SelectValue placeholder="Select a model" />
              </SelectTrigger>
              <SelectContent>
                {readyModels.length === 0 ? (
                  <SelectItem value="__no_models__" disabled>
                    No models available
                  </SelectItem>
                ) : (
                  readyModels
                    .filter((model) => model.name && model.name.length > 0)
                    .map((model) => (
                      <SelectItem key={model.name} value={model.name}>
                        {model.name}
                      </SelectItem>
                    ))
                )}
              </SelectContent>
            </Select>
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
                    message.role === 'user' && 'justify-end'
                  )}
                >
                  {message.role === 'assistant' && (
                    <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary/10">
                      <Bot className="h-4 w-4 text-primary" />
                    </div>
                  )}
                  <div
                    className={cn(
                      'rounded-lg px-4 py-2 max-w-[80%]',
                      message.role === 'user'
                        ? 'bg-primary text-primary-foreground'
                        : 'bg-muted'
                    )}
                  >
                    <p className="text-sm whitespace-pre-wrap">{message.content}</p>
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
        <div className="border-t p-4">
          <div className="max-w-3xl mx-auto flex gap-2">
            <Input
              placeholder="Type a message..."
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault()
                  sendMessage()
                }
              }}
              disabled={isStreaming}
              className="flex-1"
            />
            {isStreaming ? (
              <Button variant="destructive" onClick={stopGeneration}>
                <RefreshCw className="mr-2 h-4 w-4 animate-spin" />
                Stop
              </Button>
            ) : (
              <Button onClick={sendMessage} disabled={!input.trim() || !selectedModel}>
                <Send className="mr-2 h-4 w-4" />
                Send
              </Button>
            )}
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
              <Label>API Key</Label>
              <Input
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
              <Input
                type="number"
                value={maxTokens}
                onChange={(e) => setMaxTokens(parseInt(e.target.value) || 2048)}
                min={1}
                max={32000}
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
