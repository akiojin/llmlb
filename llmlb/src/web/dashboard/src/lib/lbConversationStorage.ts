import type { Message, MessageAttachment } from '@/components/playground/types'

export interface LbConversation {
  version: 1
  messages: Message[]
  settings: {
    selectedModel: string
    systemPrompt: string
    streamEnabled: boolean
    temperature: number
    maxTokens: number
    useMaxContext: boolean
  }
}

const DATABASE = 'llmlb-lb-playground'
const STORE = 'conversations'

function invalidConversation(): never {
  throw new Error('The saved LB conversation is corrupt or uses an unsupported version.')
}

function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return invalidConversation()
  return value as Record<string, unknown>
}

function attachment(value: unknown): MessageAttachment {
  const item = record(value)
  if ((item.type !== 'image' && item.type !== 'audio') ||
      typeof item.name !== 'string' || typeof item.data !== 'string' || typeof item.mimeType !== 'string') {
    return invalidConversation()
  }
  return { type: item.type, name: item.name, data: item.data, mimeType: item.mimeType }
}

function message(value: unknown): Message {
  const item = record(value)
  if ((item.role !== 'user' && item.role !== 'assistant' && item.role !== 'system') ||
      typeof item.content !== 'string' ||
      (item.reasoning !== undefined && typeof item.reasoning !== 'string') ||
      (item.attachments !== undefined && !Array.isArray(item.attachments))) {
    return invalidConversation()
  }
  return {
    role: item.role,
    content: item.content,
    ...(typeof item.reasoning === 'string' ? { reasoning: item.reasoning } : {}),
    ...(Array.isArray(item.attachments) ? { attachments: item.attachments.map(attachment) } : {}),
  }
}

/** Validate both reads and writes, copying only durable conversation fields. */
function conversation(value: unknown): LbConversation {
  const item = record(value)
  const settings = record(item.settings)
  if (item.version !== 1 || !Array.isArray(item.messages) ||
      typeof settings.selectedModel !== 'string' || typeof settings.systemPrompt !== 'string' ||
      typeof settings.streamEnabled !== 'boolean' || typeof settings.useMaxContext !== 'boolean' ||
      typeof settings.temperature !== 'number' || !Number.isFinite(settings.temperature) ||
      typeof settings.maxTokens !== 'number' || !Number.isSafeInteger(settings.maxTokens) || settings.maxTokens < 1) {
    return invalidConversation()
  }
  return {
    version: 1,
    messages: item.messages.map(message),
    settings: {
      selectedModel: settings.selectedModel,
      systemPrompt: settings.systemPrompt,
      streamEnabled: settings.streamEnabled,
      temperature: settings.temperature,
      maxTokens: settings.maxTokens,
      useMaxContext: settings.useMaxContext,
    },
  }
}

function userKey(userId: string): string {
  if (!userId.trim()) throw new Error('A signed-in user is required to save an LB conversation.')
  return `lb:${userId}`
}

function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    if (typeof indexedDB === 'undefined') {
      reject(new Error('Browser conversation storage is unavailable.'))
      return
    }
    const request = indexedDB.open(DATABASE, 1)
    let blocked = false
    request.onupgradeneeded = () => {
      if (!request.result.objectStoreNames.contains(STORE)) request.result.createObjectStore(STORE)
    }
    request.onerror = () => reject(request.error ?? new Error('Failed to open conversation storage.'))
    request.onblocked = () => {
      blocked = true
      reject(new Error('Conversation storage is blocked by another browser window.'))
    }
    request.onsuccess = () => {
      // A blocked open may eventually succeed after its caller has been notified.
      if (blocked) request.result.close()
      else resolve(request.result)
    }
  })
}

async function transaction<T>(
  mode: IDBTransactionMode,
  operation: (store: IDBObjectStore) => () => T,
): Promise<T> {
  const db = await openDatabase()
  try {
    return await new Promise<T>((resolve, reject) => {
      const tx = db.transaction(STORE, mode)
      const readResult = operation(tx.objectStore(STORE))
      tx.oncomplete = () => resolve(readResult())
      tx.onerror = () => reject(tx.error ?? new Error('Failed to persist the LB conversation.'))
      tx.onabort = () => reject(tx.error ?? new Error('Conversation storage transaction was aborted.'))
    })
  } finally {
    db.close()
  }
}

// Keep ordering across component unmount/remount. A failed operation must not
// prevent a later clear or read, and clear must follow every preceding save.
let tail: Promise<unknown> = Promise.resolve()
function serialized<T>(operation: () => Promise<T>): Promise<T> {
  const result = tail.then(operation)
  tail = result.catch(() => undefined)
  return result
}

export const lbConversationStorage = {
  async load(userId: string): Promise<LbConversation | null> {
    const key = userKey(userId)
    return serialized(async () => {
      const saved = await transaction('readonly', (store) => {
        const request = store.get(key)
        return () => request.result as unknown
      })
      if (saved === undefined) return null
      if (typeof saved !== 'string') return invalidConversation()
      return conversation(JSON.parse(saved))
    })
  },
  async save(userId: string, value: LbConversation): Promise<void> {
    const key = userKey(userId)
    // Snapshot at invocation time; do not retain mutable caller state in the queue.
    const saved = JSON.stringify(conversation(value))
    return serialized(() => transaction('readwrite', (store) => {
      store.put(saved, key)
      return () => undefined
    }))
  },
  async clear(userId: string): Promise<void> {
    const key = userKey(userId)
    return serialized(() => transaction('readwrite', (store) => {
      store.delete(key)
      return () => undefined
    }))
  },
}
