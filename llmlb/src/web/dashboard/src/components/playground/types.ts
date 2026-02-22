import type { ChatMessage } from '@/lib/api'

export const API_KEY_STORAGE_KEY = 'lb_playground_api_key'
export const MAX_ATTACHMENT_BYTES = 4 * 1024 * 1024

export interface MessageAttachment {
  type: 'image' | 'audio'
  name: string
  data: string
  mimeType: string
}

export interface Message {
  role: 'user' | 'assistant' | 'system'
  content: string
  attachments?: MessageAttachment[]
}

export function getErrorMessage(status: number): string {
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

export function transformMessage(msg: Message): ChatMessage {
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

export function extractMediaFromContent(content: string) {
  const imageUrlRegex = /(data:image\/[^;]+;base64,[^\s"'<>]+|https?:\/\/[^\s"'<>]+\.(png|jpg|jpeg|gif|webp))/gi
  const audioUrlRegex = /(data:audio\/[^;]+;base64,[^\s"'<>]+|https?:\/\/[^\s"'<>]+\.(mp3|wav|ogg|m4a))/gi

  const imageMatches = content.match(imageUrlRegex) || []
  const audioMatches = content.match(audioUrlRegex) || []

  return { imageMatches, audioMatches }
}
