/**
 * In-memory stand-in for the browser WebSocket. Installed globally by
 * `setup.ts`, so no component test opens a real connection.
 */
export class FakeWebSocket {
  static instances: FakeWebSocket[] = []

  /** The socket opened most recently by the code under test. */
  static latest(): FakeWebSocket {
    const socket = FakeWebSocket.instances[FakeWebSocket.instances.length - 1]
    if (!socket) throw new Error('No WebSocket has been opened')
    return socket
  }

  onopen: (() => void) | null = null
  onmessage: ((event: { data: string }) => void) | null = null
  onclose: (() => void) | null = null
  onerror: (() => void) | null = null

  constructor(public readonly url: string) {
    FakeWebSocket.instances.push(this)
  }

  close() {
    this.onclose?.()
  }

  /** Deliver a server message to the client. */
  receive(raw: string) {
    this.onmessage?.({ data: raw })
  }
}
