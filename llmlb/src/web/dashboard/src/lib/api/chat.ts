// Chat API (OpenAI compatible)

import { ApiError, API_BASE } from './client'
import type { OpenAIModelsResponse } from './models'

export interface ChatMessage {
  role: 'system' | 'user' | 'assistant'
  content: string | Array<unknown>
}

export interface ChatSession {
  id: string
  title: string
  messages: ChatMessage[]
  model?: string
  created_at: string
  updated_at: string
}

export interface ChatCompletionRequest {
  model: string
  messages: ChatMessage[]
  stream?: boolean
  temperature?: number
  max_tokens?: number
  user?: string
}

export const chatApi = {
  complete: async (
    request: ChatCompletionRequest,
    apiKey?: string,
    onChunk?: (chunk: string) => void,
    signal?: AbortSignal
  ) => {
    const headers: HeadersInit = {
      'Content-Type': 'application/json',
    }

    if (apiKey) {
      headers['Authorization'] = `Bearer ${apiKey}`
    }

    const response = await fetch(`${API_BASE}/v1/chat/completions`, {
      method: 'POST',
      headers,
      body: JSON.stringify(request),
      signal,
    })

    if (!response.ok) {
      throw new ApiError(response.status, response.statusText)
    }

    if (request.stream && onChunk) {
      const reader = response.body?.getReader()
      if (!reader) throw new Error('No response body')

      const decoder = new TextDecoder()
      let buffer = ''

      while (true) {
        const { done, value } = await reader.read()
        if (done) break

        buffer += decoder.decode(value, { stream: true })
        const lines = buffer.split('\n')
        buffer = lines.pop() || ''

        for (const line of lines) {
          if (line.startsWith('data: ')) {
            const data = line.slice(6)
            if (data === '[DONE]') continue
            try {
              const parsed = JSON.parse(data)
              const content = parsed.choices?.[0]?.delta?.content
              if (content) {
                onChunk(content)
              }
            } catch {
              // Ignore parse errors
            }
          }
        }
      }

      return null
    }

    return response.json()
  },

  getModels: async (apiKey?: string): Promise<OpenAIModelsResponse> => {
    const headers: HeadersInit = {}
    if (apiKey) {
      headers['Authorization'] = `Bearer ${apiKey}`
    }
    const response = await fetch(`${API_BASE}/v1/models`, { headers })
    if (!response.ok) {
      throw new ApiError(response.status, response.statusText)
    }
    return response.json()
  },

  // Session management (local storage based for now)
  getSessions: async (): Promise<ChatSession[]> => {
    const sessions = localStorage.getItem('chat_sessions')
    return sessions ? JSON.parse(sessions) : []
  },

  saveSessions: async (sessions: ChatSession[]): Promise<void> => {
    localStorage.setItem('chat_sessions', JSON.stringify(sessions))
  },
}
