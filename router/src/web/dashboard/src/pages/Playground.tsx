import { useState, useEffect, useRef, useCallback, useMemo, type ClipboardEvent } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useDropzone, type Accept, type FileRejection } from 'react-dropzone'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import remarkBreaks from 'remark-breaks'
import {
  chatApi,
  modelsApi,
  type CapabilitySupport,
  type ChatAttachment,
  type ChatContentPart,
  type ChatMessage,
  type ChatSession,
  type ModelCapabilities,
  type ModelInfo,
} from '@/lib/api'
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
  Image as ImageIcon,
  Mic,
  X,
} from 'lucide-react'

const ALLOWED_IMAGE_MIMES = ['image/png', 'image/jpeg'] as const
const ALLOWED_AUDIO_MIMES = ['audio/wav', 'audio/mpeg', 'audio/mp3'] as const

const MAX_IMAGE_BYTES = 5 * 1024 * 1024
const MAX_AUDIO_BYTES = 10 * 1024 * 1024

const IMAGE_ACCEPT = {
  'image/png': [],
  'image/jpeg': [],
} satisfies Accept

const AUDIO_ACCEPT = {
  'audio/wav': [],
  'audio/mpeg': [],
  'audio/mp3': [],
} satisfies Accept

function audioFormatFromMime(mime: string): string | null {
  switch (mime) {
    case 'audio/wav':
      return 'wav'
    case 'audio/mpeg':
    case 'audio/mp3':
      return 'mp3'
    default:
      return null
  }
}

function mimeFromAudioFormat(format: string): string {
  switch (format) {
    case 'wav':
      return 'audio/wav'
    case 'mp3':
      return 'audio/mpeg'
    default:
      return 'audio/wav'
  }
}

function arrayBufferToBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer)
  let binary = ''
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i])
  }
  return btoa(binary)
}

async function fileToDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onerror = () => reject(new Error('Failed to read file'))
    reader.onload = () => resolve(String(reader.result))
    reader.readAsDataURL(file)
  })
}

function redactInlineMediaInText(text: string): string {
  return text.replace(/data:(image|audio)\/[a-zA-Z0-9.+-]+;base64,[A-Za-z0-9+/=]+/g, '[media]')
}

function sanitizeMessageForStorage(message: ChatMessage): ChatMessage {
  if (typeof message.content === 'string') {
    return { ...message, content: redactInlineMediaInText(message.content) }
  }

  const summarized = message.content
    .map((p) => {
      switch (p.type) {
        case 'text':
          return p.text
        case 'image_url':
          return '[image]'
        case 'input_audio':
          return '[audio]'
        default:
          return ''
      }
    })
    .filter(Boolean)
    .join('\n')

  return { ...message, content: redactInlineMediaInText(summarized) }
}

function sanitizeSessionsForStorage(sessions: ChatSession[]): ChatSession[] {
  return sessions.map((s) => ({
    ...s,
    messages: (s.messages || []).map(sanitizeMessageForStorage),
  }))
}

function getModelCapabilities(modelName: string): ModelCapabilities {
  if (!modelName) {
    return { input_image: 'unknown', input_audio: 'unknown' }
  }

  if (modelName.startsWith('openai:')) {
    return { input_image: 'supported', input_audio: 'supported' }
  }

  if (modelName.startsWith('google:') || modelName.startsWith('anthropic:')) {
    return { input_image: 'unknown', input_audio: 'unknown' }
  }

  // Local models: treat as unsupported for MVP (avoid confusing failures)
  return { input_image: 'unsupported', input_audio: 'unsupported' }
}

function capabilityLabel(value: CapabilitySupport): string {
  switch (value) {
    case 'supported':
      return '対応'
    case 'unsupported':
      return '非対応'
    default:
      return '不明'
  }
}

function capabilityBadgeVariant(value: CapabilitySupport): 'success' | 'warning' | 'outline' {
  switch (value) {
    case 'supported':
      return 'success'
    case 'unknown':
      return 'warning'
    default:
      return 'outline'
  }
}

function buildUserContent(
  text: string,
  attachment: ChatAttachment | null
): string | ChatContentPart[] {
  const trimmed = text.trim()

  if (!attachment) {
    return trimmed
  }

  const parts: ChatContentPart[] = [{ type: 'text', text: trimmed }]

  if (attachment.kind === 'image') {
    parts.push({
      type: 'image_url',
      image_url: { url: attachment.data_url },
    })
  } else {
    parts.push({
      type: 'input_audio',
      input_audio: { data: attachment.base64_data, format: attachment.format },
    })
  }

  return parts
}

function isLikelyImageUrl(url: string): boolean {
  if (url.startsWith('data:image/')) return true
  return /^https?:\/\//i.test(url) && /\.(png|jpe?g|gif|webp)(\?|#|$)/i.test(url)
}

function isLikelyAudioUrl(url: string): boolean {
  if (url.startsWith('data:audio/')) return true
  return /^https?:\/\//i.test(url) && /\.(wav|mp3|ogg|m4a)(\?|#|$)/i.test(url)
}

function wrapInlineMediaDataUrlsAsAutolinks(text: string): string {
  const re = /data:(image|audio)\/[a-zA-Z0-9.+-]+;base64,[A-Za-z0-9+/=]+/g
  let out = ''
  let lastIndex = 0

  for (const match of text.matchAll(re)) {
    const url = match[0]
    const start = match.index ?? 0
    const end = start + url.length

    out += text.slice(lastIndex, start)

    const before = start > 0 ? text[start - 1] : ''
    const after = end < text.length ? text[end] : ''
    if (before === '<' && after === '>') {
      out += url
    } else {
      out += `<${url}>`
    }

    lastIndex = end
  }

  out += text.slice(lastIndex)
  return out
}

function markdownUrlTransform(url: string): string {
  const lower = url.toLowerCase()
  if (lower.startsWith('https://') || lower.startsWith('http://') || lower.startsWith('mailto:')) {
    return url
  }
  if (lower.startsWith('data:image/') || lower.startsWith('data:audio/')) {
    return url
  }
  return ''
}

function mediaEmbedFromHref(
  href: string,
  role: ChatMessage['role'],
  state: { imageAssigned: boolean; audioAssigned: boolean }
): React.ReactNode {
  if (isLikelyImageUrl(href)) {
    const testId =
      role === 'assistant' && !state.imageAssigned ? 'playground-assistant-image' : undefined
    if (role === 'assistant' && !state.imageAssigned) state.imageAssigned = true
    return (
      <span className="block mt-2 space-y-1">
        <img
          src={href}
          alt="embedded"
          className="block max-h-64 rounded border"
          data-testid={testId}
          loading="lazy"
        />
        <a
          href={href}
          target="_blank"
          rel="noreferrer"
          className="text-xs underline inline-flex items-center gap-1"
        >
          <ExternalLink className="h-3 w-3" />
          Open image
        </a>
      </span>
    )
  }

  if (isLikelyAudioUrl(href)) {
    const testId =
      role === 'assistant' && !state.audioAssigned ? 'playground-assistant-audio' : undefined
    if (role === 'assistant' && !state.audioAssigned) state.audioAssigned = true
    return (
      <span className="block mt-2 space-y-1">
        <audio controls src={href} className="w-full" data-testid={testId} />
        <a
          href={href}
          target="_blank"
          rel="noreferrer"
          className="text-xs underline inline-flex items-center gap-1"
        >
          <ExternalLink className="h-3 w-3" />
          Open audio
        </a>
      </span>
    )
  }

  return null
}

function MessageMarkdown({ text, role }: { text: string; role: ChatMessage['role'] }) {
  const stateRef = useRef({ imageAssigned: false, audioAssigned: false })
  const normalized = useMemo(() => wrapInlineMediaDataUrlsAsAutolinks(text), [text])

  return (
    <ReactMarkdown
      urlTransform={markdownUrlTransform}
      remarkPlugins={[remarkGfm, remarkBreaks]}
      components={{
        p: ({ children }) => <p className="text-sm whitespace-pre-wrap m-0">{children}</p>,
        pre: ({ children }) => (
          <pre className="text-xs font-mono whitespace-pre-wrap m-0">{children}</pre>
        ),
        code: ({ children }) => <code className="text-xs font-mono">{children}</code>,
        img: ({ src }) => {
          if (!src) return null
          let testId: string | undefined
          if (role === 'assistant' && !stateRef.current.imageAssigned) {
            stateRef.current.imageAssigned = true
            testId = 'playground-assistant-image'
          }
          return (
            <span className="block mt-2 space-y-1">
              <img
                src={src}
                alt="embedded"
                className="block max-h-64 rounded border"
                data-testid={testId}
                loading="lazy"
              />
              <a
                href={src}
                target="_blank"
                rel="noreferrer"
                className="text-xs underline inline-flex items-center gap-1"
              >
                <ExternalLink className="h-3 w-3" />
                Open image
              </a>
            </span>
          )
        },
        a: ({ href, children }) => {
          if (!href) return <span>{children}</span>

          const embedded = mediaEmbedFromHref(href, role, stateRef.current)
          if (embedded) return embedded

          return (
            <a
              href={href}
              target="_blank"
              rel="noreferrer"
              className="underline inline-flex items-center gap-1 break-all"
            >
              {children}
              <ExternalLink className="h-3 w-3" />
            </a>
          )
        },
      }}
    >
      {normalized}
    </ReactMarkdown>
  )
}

function MessageContent({ content, role }: { content: string | ChatContentPart[]; role: ChatMessage['role'] }) {
  if (typeof content === 'string') {
    return <MessageMarkdown text={content} role={role} />
  }

  const testIdState = { imageAssigned: false, audioAssigned: false }

  return (
    <div className="space-y-2">
      {content.map((part, idx) => {
        switch (part.type) {
          case 'text': {
            if (!part.text) return null
            return <MessageMarkdown key={`text-${idx}`} text={part.text} role={role} />
          }
          case 'image_url': {
            const href = part.image_url.url
            if (!href) return null
            const testId =
              role === 'assistant' && !testIdState.imageAssigned
                ? 'playground-assistant-image'
                : undefined
            if (role === 'assistant' && !testIdState.imageAssigned) testIdState.imageAssigned = true
            return (
              <div key={`img-${idx}`} className="space-y-1">
                <img
                  src={href}
                  alt="embedded"
                  className="max-h-64 rounded border"
                  data-testid={testId}
                  loading="lazy"
                />
                <a
                  href={href}
                  target="_blank"
                  rel="noreferrer"
                  className="text-xs underline inline-flex items-center gap-1"
                >
                  <ExternalLink className="h-3 w-3" />
                  Open image
                </a>
              </div>
            )
          }
          case 'input_audio': {
            const mime = mimeFromAudioFormat(part.input_audio.format)
            const href = `data:${mime};base64,${part.input_audio.data}`
            const testId =
              role === 'assistant' && !testIdState.audioAssigned
                ? 'playground-assistant-audio'
                : undefined
            if (role === 'assistant' && !testIdState.audioAssigned) testIdState.audioAssigned = true
            return (
              <div key={`audio-${idx}`} className="space-y-1">
                <audio controls src={href} className="w-full" data-testid={testId} />
                <a
                  href={href}
                  target="_blank"
                  rel="noreferrer"
                  className="text-xs underline inline-flex items-center gap-1"
                >
                  <ExternalLink className="h-3 w-3" />
                  Open audio
                </a>
              </div>
            )
          }
          default:
            return null
        }
      })}
    </div>
  )
}

function describeDropzoneRejection(
  kind: 'image' | 'audio',
  rejections: FileRejection[]
): { title: string; description?: string } {
  const errors = rejections.flatMap((r) => r.errors)
  const isTooLarge = errors.some((e) => e.code === 'file-too-large')
  const isInvalidType = errors.some((e) => e.code === 'file-invalid-type')
  const isTooMany = errors.some((e) => e.code === 'too-many-files')

  if (isTooMany) {
    return { title: 'ファイルは1つだけ選択できます', description: '画像/音声は同時添付できません' }
  }

  if (isTooLarge) {
    const maxMb = Math.floor((kind === 'image' ? MAX_IMAGE_BYTES : MAX_AUDIO_BYTES) / (1024 * 1024))
    return { title: `${kind === 'image' ? '画像' : '音声'}サイズが大きすぎます`, description: `上限: ${maxMb}MB` }
  }

  if (isInvalidType) {
    const allowed = kind === 'image' ? ALLOWED_IMAGE_MIMES.join(', ') : ALLOWED_AUDIO_MIMES.join(', ')
    return { title: `${kind === 'image' ? '画像' : '音声'}形式が未対応です`, description: `許可形式: ${allowed}` }
  }

  return { title: '添付に失敗しました' }
}

export default function Playground() {
  const [theme, setTheme] = useState<'dark' | 'light'>('dark')
  const [sessions, setSessions] = useState<ChatSession[]>([])
  const [currentSessionId, setCurrentSessionId] = useState<string | null>(null)
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [input, setInput] = useState('')
  const [attachment, setAttachment] = useState<ChatAttachment | null>(null)
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
      const readyModels = (models as ModelInfo[]).filter((m) => m.state === 'ready')
      if (readyModels.length > 0) {
        setSelectedModel(readyModels[0].name)
      }
    }
  }, [models, selectedModel])

  // Scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  // Persist sessions (without raw attachment data)
  useEffect(() => {
    chatApi.saveSessions(sanitizeSessionsForStorage(sessions)).catch(() => {
      // localStorage may be unavailable (private mode, etc.)
    })
  }, [sessions])

  // Theme toggle
  const toggleTheme = () => {
    const newTheme = theme === 'dark' ? 'light' : 'dark'
    setTheme(newTheme)
    document.documentElement.classList.toggle('dark', newTheme === 'dark')
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
    setAttachment((prev) => {
      if (prev?.kind === 'audio' && prev.preview_url.startsWith('blob:')) {
        URL.revokeObjectURL(prev.preview_url)
      }
      return null
    })
  }, [selectedModel])

  // Load session
  const loadSession = (session: ChatSession) => {
    setCurrentSessionId(session.id)
    setMessages(session.messages || [])
    setAttachment((prev) => {
      if (prev?.kind === 'audio' && prev.preview_url.startsWith('blob:')) {
        URL.revokeObjectURL(prev.preview_url)
      }
      return null
    })
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
      setAttachment((prev) => {
        if (prev?.kind === 'audio' && prev.preview_url.startsWith('blob:')) {
          URL.revokeObjectURL(prev.preview_url)
        }
        return null
      })
    }
    toast({ title: 'Session deleted' })
  }

  const selectedModelInfo = useMemo(
    () => (models as ModelInfo[] | undefined)?.find((m) => m.name === selectedModel),
    [models, selectedModel]
  )

  const capabilities = useMemo(
    () => selectedModelInfo?.capabilities ?? getModelCapabilities(selectedModel),
    [selectedModel, selectedModelInfo?.capabilities]
  )
  const canAttachImage =
    capabilities.input_image === 'supported' && (!attachment || attachment.kind === 'image')
  const canAttachAudio =
    capabilities.input_audio === 'supported' && (!attachment || attachment.kind === 'audio')

  const imageDropzone = useDropzone({
    accept: IMAGE_ACCEPT,
    maxSize: MAX_IMAGE_BYTES,
    multiple: false,
    disabled: !canAttachImage || isStreaming,
    noClick: true,
    noKeyboard: true,
    onDropAccepted: (files) => {
      void attachImageFile(files[0])
    },
    onDropRejected: (rejections) => {
      const { title, description } = describeDropzoneRejection('image', rejections)
      toast({ title, description, variant: 'destructive' })
    },
  })

  const audioDropzone = useDropzone({
    accept: AUDIO_ACCEPT,
    maxSize: MAX_AUDIO_BYTES,
    multiple: false,
    disabled: !canAttachAudio || isStreaming,
    noClick: true,
    noKeyboard: true,
    onDropAccepted: (files) => {
      void attachAudioFile(files[0])
    },
    onDropRejected: (rejections) => {
      const { title, description } = describeDropzoneRejection('audio', rejections)
      toast({ title, description, variant: 'destructive' })
    },
  })

  const clearAttachment = useCallback(() => {
    setAttachment((prev) => {
      if (prev?.kind === 'audio' && prev.preview_url.startsWith('blob:')) {
        URL.revokeObjectURL(prev.preview_url)
      }
      return null
    })
  }, [])

  const attachImageFile = useCallback(
    async (file: File) => {
      if (attachment && attachment.kind !== 'image') {
        toast({
          title: '音声を削除してから画像を添付してください',
          variant: 'destructive',
        })
        return
      }

      if (capabilities.input_image !== 'supported') {
        toast({
          title:
            capabilities.input_image === 'unknown'
              ? 'このモデルが画像入力に対応しているか不明です'
              : 'このモデルは画像入力に対応していません',
          variant: 'destructive',
        })
        return
      }

      if (!ALLOWED_IMAGE_MIMES.includes(file.type as any)) {
        toast({
          title: '画像形式が未対応です',
          description: `許可形式: ${ALLOWED_IMAGE_MIMES.join(', ')}`,
          variant: 'destructive',
        })
        return
      }

      if (file.size > MAX_IMAGE_BYTES) {
        toast({
          title: '画像サイズが大きすぎます',
          description: `上限: ${Math.floor(MAX_IMAGE_BYTES / (1024 * 1024))}MB`,
          variant: 'destructive',
        })
        return
      }

      const dataUrl = await fileToDataUrl(file)
      clearAttachment()
      setAttachment({
        kind: 'image',
        mime: file.type,
        name: file.name,
        size_bytes: file.size,
        data_url: dataUrl,
      })
    },
    [attachment, capabilities.input_image, clearAttachment]
  )

  const attachAudioFile = useCallback(
    async (file: File) => {
      if (attachment && attachment.kind !== 'audio') {
        toast({
          title: '画像を削除してから音声を添付してください',
          variant: 'destructive',
        })
        return
      }

      if (capabilities.input_audio !== 'supported') {
        toast({
          title:
            capabilities.input_audio === 'unknown'
              ? 'このモデルが音声入力に対応しているか不明です'
              : 'このモデルは音声入力に対応していません',
          variant: 'destructive',
        })
        return
      }

      if (!ALLOWED_AUDIO_MIMES.includes(file.type as any)) {
        toast({
          title: '音声形式が未対応です',
          description: `許可形式: ${ALLOWED_AUDIO_MIMES.join(', ')}`,
          variant: 'destructive',
        })
        return
      }

      if (file.size > MAX_AUDIO_BYTES) {
        toast({
          title: '音声サイズが大きすぎます',
          description: `上限: ${Math.floor(MAX_AUDIO_BYTES / (1024 * 1024))}MB`,
          variant: 'destructive',
        })
        return
      }

      const format = audioFormatFromMime(file.type)
      if (!format) {
        toast({ title: '音声形式の判定に失敗しました', variant: 'destructive' })
        return
      }

      const buffer = await file.arrayBuffer()
      const base64Data = arrayBufferToBase64(buffer)
      const previewUrl = URL.createObjectURL(file)

      clearAttachment()
      setAttachment({
        kind: 'audio',
        mime: file.type,
        name: file.name,
        size_bytes: file.size,
        base64_data: base64Data,
        format,
        preview_url: previewUrl,
      })
    },
    [attachment, capabilities.input_audio, clearAttachment]
  )

  const handlePaste = useCallback(
    async (e: ClipboardEvent) => {
      const items = e.clipboardData?.items
      if (!items || items.length === 0) return

      const files = Array.from(items)
        .filter((item) => item.kind === 'file')
        .map((item) => item.getAsFile())
        .filter((f): f is File => !!f)

      const image = files.find((f) => f.type.startsWith('image/'))
      const audio = files.find((f) => f.type.startsWith('audio/'))

      if (image) {
        e.preventDefault()
        await attachImageFile(image)
        return
      }

      if (audio) {
        e.preventDefault()
        await attachAudioFile(audio)
      }
    },
    [attachAudioFile, attachImageFile]
  )

  // Send message
  const sendMessage = async () => {
    if ((!input.trim() && !attachment) || !selectedModel || isStreaming) return

    const attachmentSnapshot = attachment
    const userContent = buildUserContent(input, attachmentSnapshot)
    if (typeof userContent === 'string' && !userContent.trim()) return

    const userMessage: ChatMessage = { role: 'user', content: userContent }
    const newMessages = [...messages, userMessage]
    setMessages(newMessages)
    setInput('')
    clearAttachment()

    // Update session
    if (currentSessionId) {
      const titleSeed =
        typeof userContent === 'string'
          ? userContent
          : input.trim() || (attachmentSnapshot?.kind === 'image' ? 'Image' : 'Audio')
      setSessions((prev) =>
        prev.map((s) =>
          s.id === currentSessionId
            ? {
                ...s,
                messages: newMessages,
                title: newMessages.length === 1 ? titleSeed.slice(0, 30) : s.title,
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
        const response = await fetch('/v1/chat/completions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
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
        const response = await fetch('/v1/chat/completions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
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
        const rawContent = data.choices?.[0]?.message?.content
        const assistantMessage: ChatMessage = {
          role: 'assistant',
          content: Array.isArray(rawContent) ? rawContent : rawContent || '',
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

    return `curl -X POST 'http://localhost:8080/v1/chat/completions' \\
  -H 'Content-Type: application/json' \\
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

  const readyModels = (models as ModelInfo[] | undefined)?.filter((m) => m.state === 'ready') || []

  return (
      <div className="flex h-screen bg-background">
      {/* Sidebar */}
      <div className="w-64 border-r flex flex-col" data-testid="playground-sidebar">
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
          <div className="p-2 space-y-1" data-testid="playground-session-list">
            {sessions.map((session) => (
              <div
                key={session.id}
                className={cn(
                  'group flex items-center gap-2 rounded-lg px-3 py-2 cursor-pointer hover:bg-muted/50',
                  currentSessionId === session.id && 'bg-muted'
                )}
                onClick={() => loadSession(session)}
                data-testid={`playground-session-${session.id}`}
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
              data-testid="playground-open-settings"
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
              <SelectTrigger className="w-64" data-testid="playground-model-select">
                <SelectValue placeholder="Select a model" />
              </SelectTrigger>
              <SelectContent>
                {readyModels.length === 0 ? (
                  <SelectItem value="__no_models__" disabled>
                    No models available
                  </SelectItem>
                ) : (
                  readyModels.map((model) => (
                    <SelectItem key={model.name} value={model.name}>
                      <span className="flex w-full items-center justify-between gap-2">
                        <span className="truncate">{model.name}</span>
                        <span className="flex items-center gap-1">
                          <Badge
                            variant={capabilityBadgeVariant(model.capabilities.input_image)}
                            className="text-[10px] px-2 py-0.5"
                          >
                            <ImageIcon className="h-3 w-3 mr-1" />
                            {capabilityLabel(model.capabilities.input_image)}
                          </Badge>
                          <Badge
                            variant={capabilityBadgeVariant(model.capabilities.input_audio)}
                            className="text-[10px] px-2 py-0.5"
                          >
                            <Mic className="h-3 w-3 mr-1" />
                            {capabilityLabel(model.capabilities.input_audio)}
                          </Badge>
                        </span>
                      </span>
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
            <div className="flex items-center gap-1">
              <Badge
                variant={capabilityBadgeVariant(capabilities.input_image)}
                className="text-xs"
              >
                <ImageIcon className="h-3 w-3 mr-1" />
                {capabilityLabel(capabilities.input_image)}
              </Badge>
              <Badge
                variant={capabilityBadgeVariant(capabilities.input_audio)}
                className="text-xs"
              >
                <Mic className="h-3 w-3 mr-1" />
                {capabilityLabel(capabilities.input_audio)}
              </Badge>
            </div>
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
                    <MessageContent content={message.content} role={message.role} />
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
          <div className="max-w-3xl mx-auto flex gap-2 items-end">
            <input
              {...imageDropzone.getInputProps({
                className: 'hidden',
                'data-testid': 'playground-image-input',
              })}
            />
            <input
              {...audioDropzone.getInputProps({
                className: 'hidden',
                'data-testid': 'playground-audio-input',
              })}
            />

            <div {...imageDropzone.getRootProps()}>
              <Button
                variant="outline"
                size="icon"
                onClick={() => imageDropzone.open()}
                disabled={!canAttachImage || isStreaming}
                title={
                  canAttachImage
                    ? 'Attach image'
                    : attachment && attachment.kind !== 'image'
                      ? '画像以外の添付が選択中です'
                      : capabilities.input_image === 'unknown'
                        ? 'このモデルの画像入力対応は不明です'
                        : 'このモデルは画像入力に対応していません'
                }
                data-testid="playground-attach-image"
              >
                <ImageIcon className="h-4 w-4" />
              </Button>
            </div>

            <div {...audioDropzone.getRootProps()}>
              <Button
                variant="outline"
                size="icon"
                onClick={() => audioDropzone.open()}
                disabled={!canAttachAudio || isStreaming}
                title={
                  canAttachAudio
                    ? 'Attach audio'
                    : attachment && attachment.kind !== 'audio'
                      ? '音声以外の添付が選択中です'
                      : capabilities.input_audio === 'unknown'
                        ? 'このモデルの音声入力対応は不明です'
                        : 'このモデルは音声入力に対応していません'
                }
                data-testid="playground-attach-audio"
              >
                <Mic className="h-4 w-4" />
              </Button>
            </div>

            <div className="flex-1 space-y-2">
              {attachment && (
                <div
                  className="rounded-md border bg-muted p-2 flex items-center gap-2"
                  data-testid="playground-attachment-preview"
                >
                  {attachment.kind === 'image' ? (
                    <img
                      src={attachment.data_url}
                      alt="attachment"
                      className="h-12 w-12 rounded object-cover border"
                      data-testid="playground-attachment-image"
                    />
                  ) : (
                    <audio
                      controls
                      src={attachment.preview_url}
                      className="w-full"
                      data-testid="playground-attachment-audio"
                    />
                  )}
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={clearAttachment}
                    data-testid="playground-attachment-remove"
                  >
                    <X className="h-4 w-4" />
                  </Button>
                </div>
              )}

              <Textarea
                placeholder="Type a message... (paste image/audio here)"
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onPaste={handlePaste}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !e.shiftKey) {
                    e.preventDefault()
                    sendMessage()
                  }
                }}
                disabled={isStreaming}
                rows={1}
                className="flex-1 min-h-[40px] resize-none"
                data-testid="playground-chat-input"
              />
            </div>

            {isStreaming ? (
              <Button variant="destructive" onClick={stopGeneration} data-testid="playground-stop">
                <RefreshCw className="mr-2 h-4 w-4 animate-spin" />
                Stop
              </Button>
            ) : (
              <Button
                onClick={sendMessage}
                disabled={(!input.trim() && !attachment) || !selectedModel}
                data-testid="playground-send"
              >
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
