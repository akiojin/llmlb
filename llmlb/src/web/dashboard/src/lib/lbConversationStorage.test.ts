import { beforeEach, describe, expect, it, vi } from 'vitest'
import { lbConversationStorage, type LbConversation } from './lbConversationStorage'

function conversation(): LbConversation {
  return {
    version: 1,
    messages: [
      { role: 'user', content: 'Question', attachments: [{
        type: 'image', name: 'photo.png', mimeType: 'image/png', data: 'data:image/png;base64,YQ==',
      }] },
      { role: 'assistant', content: 'Answer', reasoning: 'Reasoning' },
    ],
    settings: {
      selectedModel: 'model-a', systemPrompt: 'Be helpful.', streamEnabled: true,
      temperature: 0.7, maxTokens: 16384, useMaxContext: false,
    },
  }
}

// jsdom has no IndexedDB. Keep storage I/O controlled here; browser E2E covers
// the real database implementation. Completion is separate from request success.
function database(openFailure: 'blocked' | 'error' | null = null) {
  const values = new Map<string, unknown>()
  const transactions: Array<{ oncomplete: (() => void) | null; onerror: (() => void) | null; onabort: (() => void) | null; error: Error | null }> = []
  let autoComplete = true
  let transactionError: Error | null = null
  const db = {
    objectStoreNames: { contains: () => true },
    createObjectStore: vi.fn(),
    close: vi.fn(),
    transaction: vi.fn(() => {
      const tx = {
        oncomplete: null as (() => void) | null,
        onerror: null as (() => void) | null,
        onabort: null as (() => void) | null,
        error: transactionError,
        objectStore: () => ({
          get: (key: string) => {
            const request = { result: values.get(key) }
            queueMicrotask(() => {
              if (tx.error) tx.onerror?.()
              else if (autoComplete) tx.oncomplete?.()
            })
            return request
          },
          put: (value: unknown, key: string) => {
            queueMicrotask(() => {
              if (tx.error) tx.onabort?.()
              else {
                values.set(key, value)
                if (autoComplete) tx.oncomplete?.()
              }
            })
          },
          delete: (key: string) => {
            queueMicrotask(() => {
              if (tx.error) tx.onabort?.()
              else {
                values.delete(key)
                if (autoComplete) tx.oncomplete?.()
              }
            })
          },
        }),
      }
      transactions.push(tx)
      return tx
    }),
  }
  const factory = {
    open: vi.fn(() => {
      const request = {
        result: db, error: openFailure === 'error' ? new Error('Storage denied') : null,
        onupgradeneeded: null as (() => void) | null,
        onsuccess: null as (() => void) | null,
        onerror: null as (() => void) | null,
        onblocked: null as (() => void) | null,
      }
      queueMicrotask(() => {
        if (openFailure === 'error') request.onerror?.()
        else if (openFailure === 'blocked') {
          request.onblocked?.()
          queueMicrotask(() => request.onsuccess?.())
        } else request.onsuccess?.()
      })
      return request
    }),
  }
  vi.stubGlobal('indexedDB', factory)
  return {
    values, db, factory, transactions,
    holdCompletion: () => { autoComplete = false },
    failTransaction: (error: Error | null) => { transactionError = error },
  }
}

describe('LB conversation storage', () => {
  beforeEach(() => { vi.stubGlobal('indexedDB', undefined) })

  it('roundtrips reasoning, settings and a complete 4MB attachment', async () => {
    const idb = database()
    const saved = conversation()
    saved.messages[0].attachments![0].data = `data:image/png;base64,${'YWFh'.repeat(Math.floor(4 * 1024 * 1024 / 3)) + 'YQ=='}`
    await lbConversationStorage.save('user-a', saved)
    expect(await lbConversationStorage.load('user-a')).toEqual(saved)
    expect(typeof idb.values.get('lb:user-a')).toBe('string')
    expect(idb.factory.open).toHaveBeenCalledWith('llmlb-lb-playground', 1)
  })

  it('isolates user keys and removes only the cleared user conversation', async () => {
    database()
    const saved = conversation()
    await lbConversationStorage.save('user-a', saved)
    await lbConversationStorage.save('user-b', { ...saved, messages: [] })
    await lbConversationStorage.clear('user-a')
    expect(await lbConversationStorage.load('user-a')).toBeNull()
    expect(await lbConversationStorage.load('user-b')).toEqual({ ...saved, messages: [] })
  })

  it('whitelists persisted fields instead of saving credentials or execution state', async () => {
    const idb = database()
    const saved = {
      ...conversation(), jwt: 'secret-jwt', isStreaming: true,
      settings: { ...conversation().settings, apiKey: 'secret-api-key' },
      messages: conversation().messages.map((message) => ({ ...message, controller: 'runtime-state' })),
    }
    await lbConversationStorage.save('user-a', saved)
    expect(JSON.parse(idb.values.get('lb:user-a') as string)).toEqual(conversation())
    idb.values.set('lb:user-a', JSON.stringify(saved))
    expect(await lbConversationStorage.load('user-a')).toEqual(conversation())
  })

  it('returns null for an absent conversation', async () => {
    database()
    expect(await lbConversationStorage.load('unknown')).toBeNull()
  })

  it.each([
    '{broken',
    JSON.stringify({ ...conversation(), version: 2 }),
    JSON.stringify({ ...conversation(), messages: [{ role: 'invalid', content: 'x' }] }),
    JSON.stringify({ ...conversation(), settings: { ...conversation().settings, maxTokens: 'invalid' } }),
  ])('rejects corrupt or unsupported data without inventing a conversation: %s', async (value) => {
    const idb = database()
    idb.values.set('lb:user-a', value)
    await expect(lbConversationStorage.load('user-a')).rejects.toThrow()
  })

  it('waits for transaction completion before confirming persistence', async () => {
    const idb = database()
    idb.holdCompletion()
    let saved = false
    const pending = lbConversationStorage.save('user-a', conversation()).then(() => { saved = true })
    void pending.catch(() => undefined)
    await vi.waitFor(() => expect(idb.transactions).toHaveLength(1))
    expect(saved).toBe(false)
    idb.transactions[0].oncomplete?.()
    await pending
    expect(saved).toBe(true)
    expect(idb.db.close).toHaveBeenCalled()
  })

  it('serializes writes, clear and a remount load in invocation order', async () => {
    const idb = database()
    idb.holdCompletion()
    const save = lbConversationStorage.save('user-a', conversation())
    const clear = lbConversationStorage.clear('user-a')
    const load = lbConversationStorage.load('user-a')
    void save.catch(() => undefined)
    void clear.catch(() => undefined)
    void load.catch(() => undefined)
    await vi.waitFor(() => expect(idb.transactions).toHaveLength(1))
    idb.transactions[0].oncomplete?.()
    await save
    await vi.waitFor(() => expect(idb.transactions).toHaveLength(2))
    idb.transactions[1].oncomplete?.()
    await clear
    await vi.waitFor(() => expect(idb.transactions).toHaveLength(3))
    idb.transactions[2].oncomplete?.()
    expect(await load).toBeNull()
  })

  it('propagates quota errors rather than reporting a successful save', async () => {
    const idb = database()
    idb.failTransaction(new DOMException('Quota exceeded', 'QuotaExceededError'))
    await expect(lbConversationStorage.save('user-a', conversation())).rejects.toThrow('Quota exceeded')
    expect(idb.db.close).toHaveBeenCalled()
    idb.failTransaction(null)
    await lbConversationStorage.clear('user-a')
    expect(await lbConversationStorage.load('user-a')).toBeNull()
  })

  it('rejects a blocked open and closes a subsequently opened connection', async () => {
    const idb = database('blocked')
    await expect(lbConversationStorage.load('user-a')).rejects.toThrow('blocked')
    await vi.waitFor(() => expect(idb.db.close).toHaveBeenCalled())
  })

  it('propagates an open failure', async () => {
    database('error')
    await expect(lbConversationStorage.load('user-a')).rejects.toThrow('Storage denied')
  })

  it('rejects an unauthenticated user key before accessing storage', async () => {
    const idb = database()
    await expect(lbConversationStorage.load('')).rejects.toThrow('signed-in user')
    await expect(lbConversationStorage.save('', conversation())).rejects.toThrow('signed-in user')
    await expect(lbConversationStorage.clear('')).rejects.toThrow('signed-in user')
    expect(idb.factory.open).not.toHaveBeenCalled()
  })

  it('rejects invalid write data before replacing the saved conversation', async () => {
    const idb = database()
    const saved = conversation()
    await lbConversationStorage.save('user-a', saved)
    await expect(lbConversationStorage.save('user-a', {
      ...saved, settings: { ...saved.settings, temperature: Number.NaN },
    })).rejects.toThrow()
    expect(await lbConversationStorage.load('user-a')).toEqual(saved)
    expect(idb.values.size).toBe(1)
  })

  it('rejects unavailable IndexedDB', async () => {
    await expect(lbConversationStorage.load('user-a')).rejects.toThrow()
    await expect(lbConversationStorage.save('user-a', conversation())).rejects.toThrow()
    await expect(lbConversationStorage.clear('user-a')).rejects.toThrow()
  })
})
