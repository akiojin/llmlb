import '@testing-library/jest-dom/vitest'
import { cleanup, configure } from '@testing-library/react'
import { afterEach, beforeEach, vi } from 'vitest'
import { toast } from '@/hooks/use-toast'
import { FakeWebSocket } from './fake-websocket'

// The default 1s is too tight for the first render of a large page on a busy
// CI runner. This only bounds how long a failing query waits.
configure({ asyncUtilTimeout: 5000 })

let unstubbedFetch = vi.fn<typeof fetch>()

beforeEach(() => {
  // Component tests never talk to a server. A request that a test did not
  // stub is rejected, and fails that test in afterEach.
  unstubbedFetch = vi.fn<typeof fetch>((input) =>
    Promise.reject(new Error(`Unstubbed fetch in component test: ${String(input)}`)),
  )
  vi.stubGlobal('fetch', unstubbedFetch)
  FakeWebSocket.instances = []
  vi.stubGlobal('WebSocket', FakeWebSocket)

  // jsdom does not implement matchMedia (used by useTheme).
  vi.stubGlobal(
    'matchMedia',
    vi.fn((query: string) => ({
      matches: false,
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })),
  )

  // Layout APIs that jsdom lacks and Radix UI / Recharts call on mount.
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  )
  Element.prototype.scrollIntoView = vi.fn()
  Element.prototype.hasPointerCapture = vi.fn(() => false)
  Element.prototype.setPointerCapture = vi.fn()
  Element.prototype.releasePointerCapture = vi.fn()
})

// Vitest globals are disabled, so React Testing Library cannot register its
// own cleanup hook. Unmount rendered trees explicitly between tests.
afterEach(() => {
  cleanup()
  localStorage.clear()
  // The toast store is module state and keeps its last toast, which the next
  // test would render again. Replace it with an empty, already closed one.
  toast({}).dismiss()

  const requests = unstubbedFetch.mock.calls.map(([input]) => String(input))
  if (requests.length > 0) {
    throw new Error(
      `Component test reached the network without a stub: ${requests.join(', ')}. ` +
        'Stub the API method with vi.spyOn.',
    )
  }
})
