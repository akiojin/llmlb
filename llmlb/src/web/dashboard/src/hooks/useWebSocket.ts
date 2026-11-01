import { useEffect, useRef, useCallback, useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { invalidateDashboardSubscriptions } from './dashboardSubscriptions'

import type { DashboardChange } from '@/lib/dashboardResources'

interface UseWebSocketOptions {
  onMessage?: (event: DashboardChange) => void
  onConnect?: () => void
  onDisconnect?: () => void
  enabled?: boolean
}

/** Calculate reconnect delay with exponential backoff and jitter */
function getReconnectDelay(attempt: number): number {
  const base = Math.min(1000 * Math.pow(2, attempt), 30000)
  const jitter = base * 0.2 * Math.random()
  return base + jitter
}

export function useWebSocket(options: UseWebSocketOptions = {}) {
  const {
    onMessage,
    onConnect,
    onDisconnect,
    enabled = true,
  } = options

  const queryClient = useQueryClient()
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const reconnectAttemptRef = useRef(0)
  const connectRef = useRef<() => void>(() => {})
  const shouldReconnectRef = useRef(false)
  const generationRef = useRef(0)
  const [isConnected, setIsConnected] = useState(false)
  const [lastEvent, setLastEvent] = useState<DashboardChange | null>(null)

  // Stabilize callback references with useRef to prevent infinite reconnection loops.
  // Without this, inline callbacks passed by callers create new references every render,
  // which would trigger useCallback/useEffect dependency changes and cause
  // connect → disconnect → reconnect cycles on every render.
  const onMessageRef = useRef(onMessage)
  const onConnectRef = useRef(onConnect)
  const onDisconnectRef = useRef(onDisconnect)
  useEffect(() => {
    onMessageRef.current = onMessage
    onConnectRef.current = onConnect
    onDisconnectRef.current = onDisconnect
  })

  const scheduleReconnect = useCallback(() => {
    if (!shouldReconnectRef.current) return

    const delay = getReconnectDelay(reconnectAttemptRef.current)
    reconnectAttemptRef.current += 1
    const generation = generationRef.current
    const timeout = setTimeout(() => {
      if (!shouldReconnectRef.current || generation !== generationRef.current || reconnectTimeoutRef.current !== timeout) return
      reconnectTimeoutRef.current = null
      connectRef.current()
    }, delay)
    reconnectTimeoutRef.current = timeout
  }, [])

  const connect = useCallback(() => {
    if (!shouldReconnectRef.current || wsRef.current) return
    const generation = generationRef.current

    // Determine WebSocket URL based on current location
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const wsUrl = `${protocol}//${window.location.host}/ws/dashboard`

    try {
      const ws = new WebSocket(wsUrl)
      wsRef.current = ws
      // Queued events can outlive cleanup, including a later login session.
      const isCurrent = () => shouldReconnectRef.current && generation === generationRef.current && wsRef.current === ws

      ws.onopen = () => {
        if (!isCurrent()) return
        reconnectAttemptRef.current = 0
        setIsConnected(true)
        onConnectRef.current?.()
      }

      ws.onmessage = (event) => {
        if (!isCurrent()) return
        try {
          const data = JSON.parse(event.data) as DashboardChange
          setLastEvent(data)
          onMessageRef.current?.(data)

          invalidateDashboardSubscriptions(queryClient, data)
        } catch (err) {
          console.error('Failed to parse WebSocket message:', err)
        }
      }

      ws.onclose = () => {
        if (!isCurrent()) return
        setIsConnected(false)
        onDisconnectRef.current?.()
        wsRef.current = null

        // Schedule reconnection with exponential backoff
        if (shouldReconnectRef.current) {
          if (reconnectTimeoutRef.current) {
            clearTimeout(reconnectTimeoutRef.current)
          }
          scheduleReconnect()
        }
      }

      // Browser fires onclose automatically after onerror; no manual ws.close() needed
      ws.onerror = () => {
        if (!isCurrent()) return
        console.warn('WebSocket connection error')
      }
    } catch (err) {
      console.error('Failed to create WebSocket:', err)
      // Schedule reconnection with exponential backoff
      scheduleReconnect()
    }
  }, [queryClient, scheduleReconnect])

  useEffect(() => {
    connectRef.current = connect
  }, [connect])

  const disconnect = useCallback(() => {
    shouldReconnectRef.current = false
    generationRef.current += 1
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current)
      reconnectTimeoutRef.current = null
    }
    reconnectAttemptRef.current = 0
    setIsConnected(false)
    if (wsRef.current) {
      const ws = wsRef.current
      wsRef.current = null
      ws.onopen = ws.onmessage = ws.onclose = ws.onerror = null
      ws.close()
      onDisconnectRef.current?.()
    }
  }, [])

  useEffect(() => {
    if (!enabled) return
    shouldReconnectRef.current = true
    connect()
    return () => {
      disconnect()
    }
  }, [connect, disconnect, enabled])

  return {
    isConnected,
    lastEvent,
    reconnect: connect,
    disconnect,
  }
}

/**
 * Hook specifically for dashboard page
 * Connects to WebSocket and provides connection status
 */
interface UseDashboardWebSocketOptions {
  enabled?: boolean
}

export function useDashboardWebSocket(options: UseDashboardWebSocketOptions = {}) {
  const { enabled = true } = options
  const { isConnected, lastEvent, reconnect } = useWebSocket({
    enabled,
  })

  return {
    isConnected,
    lastEvent,
    reconnect,
  }
}
