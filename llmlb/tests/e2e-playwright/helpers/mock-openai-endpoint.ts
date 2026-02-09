import http, { type IncomingMessage, type ServerResponse } from 'node:http'
import { type AddressInfo } from 'node:net'

export interface MockOpenAIEndpointServer {
  baseUrl: string
  models: string[]
  close: () => Promise<void>
}

function readBody(req: IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = []
    req.on('data', (chunk) => chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk)))
    req.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')))
    req.on('error', reject)
  })
}

function writeJson(res: ServerResponse, status: number, value: unknown): void {
  const body = JSON.stringify(value)
  res.writeHead(status, {
    'Content-Type': 'application/json; charset=utf-8',
    'Content-Length': Buffer.byteLength(body),
  })
  res.end(body)
}

function sseWrite(res: ServerResponse, data: unknown): void {
  res.write(`data: ${typeof data === 'string' ? data : JSON.stringify(data)}\n\n`)
}

function extractLastUserText(messages: unknown): string {
  if (!Array.isArray(messages)) return ''
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const m = messages[i] as { role?: unknown; content?: unknown } | null
    if (!m || m.role !== 'user') continue
    if (typeof m.content === 'string') return m.content
    try {
      return JSON.stringify(m.content)
    } catch {
      return ''
    }
  }
  return ''
}

export async function startMockOpenAIEndpointServer(options?: {
  models?: string[]
  supportsResponsesApi?: boolean
}): Promise<MockOpenAIEndpointServer> {
  const models = options?.models?.length ? options.models : ['mock-model-a', 'mock-model-b']
  const supportsResponsesApi = options?.supportsResponsesApi ?? false

  // reqwest (llmlb) uses keep-alive connections; server.close() waits for them.
  // Track sockets and destroy them on shutdown so afterAll doesn't hang.
  const sockets = new Set<import('node:net').Socket>()

  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url || '/', 'http://127.0.0.1')

    // Basic health endpoint for llmlb's feature detection.
    if (req.method === 'GET' && url.pathname === '/health') {
      return writeJson(res, 200, { supports_responses_api: supportsResponsesApi })
    }

    // OpenAI-compatible models listing.
    if (req.method === 'GET' && url.pathname === '/v1/models') {
      const created = Math.floor(Date.now() / 1000)
      return writeJson(res, 200, {
        object: 'list',
        data: models.map((id) => ({
          id,
          object: 'model',
          created,
          owned_by: 'mock',
        })),
      })
    }

    // OpenAI-compatible chat completions.
    if (req.method === 'POST' && url.pathname === '/v1/chat/completions') {
      let parsed: any
      try {
        const bodyText = await readBody(req)
        parsed = JSON.parse(bodyText || '{}')
      } catch {
        return writeJson(res, 400, {
          error: { message: 'invalid_json', type: 'invalid_request_error' },
        })
      }

      const model = typeof parsed?.model === 'string' ? parsed.model : 'unknown'
      const lastUser = extractLastUserText(parsed?.messages)
      const reply = `MOCK_OK model=${model} user=${lastUser}`

      const stream = parsed?.stream === true
      if (!stream) {
        const created = Math.floor(Date.now() / 1000)
        return writeJson(res, 200, {
          id: `chatcmpl_mock_${created}`,
          object: 'chat.completion',
          created,
          model,
          choices: [
            {
              index: 0,
              message: { role: 'assistant', content: reply },
              finish_reason: 'stop',
            },
          ],
        })
      }

      res.writeHead(200, {
        'Content-Type': 'text/event-stream; charset=utf-8',
        'Cache-Control': 'no-cache',
        Connection: 'keep-alive',
      })

      // A few chunks so the UI streaming path is exercised.
      const parts = [reply.slice(0, 12), reply.slice(12, 24), reply.slice(24)]
      for (const part of parts) {
        sseWrite(res, { choices: [{ delta: { content: part } }] })
      }
      sseWrite(res, '[DONE]')
      res.end()
      return
    }

    // Default: Not found.
    writeJson(res, 404, { error: { message: 'not_found' } })
  })

  server.on('connection', (socket) => {
    sockets.add(socket)
    socket.on('close', () => sockets.delete(socket))
  })

  await new Promise<void>((resolve) => {
    server.listen(0, '127.0.0.1', resolve)
  })

  const addr = server.address() as AddressInfo
  const baseUrl = `http://127.0.0.1:${addr.port}`

  return {
    baseUrl,
    models,
    close: () =>
      new Promise<void>((resolve, reject) => {
        for (const s of sockets) s.destroy()
        server.close((err) => {
          if (err) reject(err)
          else resolve()
        })
      }),
  }
}
