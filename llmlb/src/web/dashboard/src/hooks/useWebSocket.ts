import { useEffect, useRef, useCallback, useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'

export type DashboardEventType =
  | 'connected'
  | 'NodeRegistered'
  | 'NodeStatusChanged'
  | 'MetricsUpdated'
  | 'NodeRemoved'
  | 'TpsUpdated'

export interface DashboardEvent {
  type: DashboardEventType
  data?: {
    runtime_id?: string
    endpoint_id?: string
    machine_name?: string
    ip_address?: string
    status?: string
    old_status?: string
    new_status?: string
    cpu_usage?: number
    memory_usage?: number
    gpu_usage?: number
    model_id?: string
    tps?: number
    output_tokens?: number
    duration_ms?: number
  }
  message?: string
}

interface UseWebSocketOptions {
  onMessage?: (event: DashboardEvent) => void
  onConnect?: () => void
  onDisconnect?: () => void
  reconnectInterval?: number
}

export function useWebSocket(options: UseWebSocketOptions = {}) {
  const {
    onMessage,
    onConnect,
    onDisconnect,
    reconnectInterval = 3000,
  } = options

  const queryClient = useQueryClient()
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const [isConnected, setIsConnected] = useState(false)
  const [lastEvent, setLastEvent] = useState<DashboardEvent | null>(null)

  const connect = useCallback(() => {
    // Determine WebSocket URL based on current location
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const wsUrl = `${protocol}//${window.location.host}/ws/dashboard`

    try {
      const ws = new WebSocket(wsUrl)
      wsRef.current = ws

      ws.onopen = () => {
        setIsConnected(true)
        onConnect?.()
      }

      ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data) as DashboardEvent
          setLastEvent(data)
          onMessage?.(data)

          // Invalidate relevant queries based on event type
          // Query keys must match those used in Dashboard.tsx
          switch (data.type) {
            case 'NodeRegistered':
            case 'NodeRemoved':
            case 'NodeStatusChanged':
              // Invalidate dashboard overview query (includes endpoints, stats)
              queryClient.invalidateQueries({ queryKey: ['dashboard-overview'] })
              queryClient.invalidateQueries({ queryKey: ['request-responses'] })
              break
            case 'MetricsUpdated':
              // Invalidate dashboard overview for metrics updates
              queryClient.invalidateQueries({ queryKey: ['dashboard-overview'] })
              break
            case 'TpsUpdated':
              // SPEC-4bb5b55f: Invalidate TPS data for the affected endpoint
              if (data.data?.endpoint_id) {
                queryClient.invalidateQueries({ queryKey: ['endpoint-model-tps', data.data.endpoint_id] })
              }
              break
          }
        } catch (err) {
          console.error('Failed to parse WebSocket message:', err)
        }
      }

      ws.onclose = () => {
        setIsConnected(false)
        onDisconnect?.()
        wsRef.current = null

        // Schedule reconnection
        if (reconnectTimeoutRef.current) {
          clearTimeout(reconnectTimeoutRef.current)
        }
        reconnectTimeoutRef.current = setTimeout(connect, reconnectInterval)
      }

      ws.onerror = (error) => {
        console.error('WebSocket error:', error)
        ws.close()
      }
    } catch (err) {
      console.error('Failed to create WebSocket:', err)
      // Schedule reconnection
      reconnectTimeoutRef.current = setTimeout(connect, reconnectInterval)
    }
  }, [onMessage, onConnect, onDisconnect, reconnectInterval, queryClient])

  const disconnect = useCallback(() => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current)
      reconnectTimeoutRef.current = null
    }
    if (wsRef.current) {
      wsRef.current.close()
      wsRef.current = null
    }
    setIsConnected(false)
  }, [])

  useEffect(() => {
    connect()
    return () => {
      disconnect()
    }
  }, [connect, disconnect])

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
export function useDashboardWebSocket() {
  const { isConnected, lastEvent, reconnect } = useWebSocket({
    onConnect: () => {
      console.log('Dashboard WebSocket connected')
    },
    onDisconnect: () => {
      console.log('Dashboard WebSocket disconnected')
    },
  })

  return {
    isConnected,
    lastEvent,
    reconnect,
  }
}
