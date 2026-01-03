// useMatchWebSocket Hook
// Provides real-time WebSocket connection for match status updates

import { useEffect, useRef, useState, useCallback } from 'react'

export interface MatchStatusEvent {
  matchId: string
  userId: string
  notes?: string
  reason?: string
}

export interface MatchWebSocketMessage {
  type: 'MatchConfirmed' | 'MatchRejected' | 'MatchUndone' | 'Ping' | 'NewMatch'
  payload?: MatchStatusEvent | unknown
}

export interface UseMatchWebSocketOptions {
  onMatchUpdated: (matchId: string, newStatus: string, byUserId: string) => void
  onConnectionChange?: (connected: boolean) => void
  autoReconnect?: boolean
  maxReconnectAttempts?: number
}

export interface UseMatchWebSocketReturn {
  isConnected: boolean
  reconnect: () => void
  disconnect: () => void
}

const WS_BASE_URL = import.meta.env.VITE_WS_URL || 'ws://localhost:8081'
const INITIAL_RECONNECT_DELAY = 1000
const MAX_RECONNECT_DELAY = 30000

/**
 * Hook for WebSocket connection to receive real-time match updates.
 * Implements auto-reconnect with exponential backoff.
 */
export function useMatchWebSocket(
  options: UseMatchWebSocketOptions,
): UseMatchWebSocketReturn {
  const {
    onMatchUpdated,
    onConnectionChange,
    autoReconnect = true,
    maxReconnectAttempts = 10,
  } = options

  const [isConnected, setIsConnected] = useState(false)
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectAttemptRef = useRef(0)
  const reconnectTimeoutRef = useRef<NodeJS.Timeout | null>(null)
  const mountedRef = useRef(true)

  // Calculate reconnect delay with exponential backoff
  const getReconnectDelay = useCallback(() => {
    const delay = Math.min(
      INITIAL_RECONNECT_DELAY * Math.pow(2, reconnectAttemptRef.current),
      MAX_RECONNECT_DELAY,
    )
    return delay
  }, [])

  // Handle incoming WebSocket messages
  const handleMessage = useCallback(
    (event: MessageEvent) => {
      try {
        const message: MatchWebSocketMessage = JSON.parse(event.data)

        switch (message.type) {
          case 'MatchConfirmed': {
            const payload = message.payload as MatchStatusEvent
            onMatchUpdated(payload.matchId, 'Confirmed', payload.userId)
            break
          }
          case 'MatchRejected': {
            const payload = message.payload as MatchStatusEvent
            onMatchUpdated(payload.matchId, 'Rejected', payload.userId)
            break
          }
          case 'MatchUndone': {
            const payload = message.payload as MatchStatusEvent
            onMatchUpdated(payload.matchId, 'Pending', payload.userId)
            break
          }
          case 'Ping':
            // Heartbeat - no action needed
            break
          default:
            // Ignore other message types
            break
        }
      } catch (error) {
        console.error('Failed to parse WebSocket message:', error)
      }
    },
    [onMatchUpdated],
  )

  // Connect to WebSocket
  const connect = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      return
    }

    try {
      const ws = new WebSocket(`${WS_BASE_URL}/ws`)

      ws.onopen = () => {
        if (!mountedRef.current) return
        console.log('WebSocket connected')
        setIsConnected(true)
        onConnectionChange?.(true)
        reconnectAttemptRef.current = 0
      }

      ws.onclose = (event) => {
        if (!mountedRef.current) return
        setIsConnected(false)
        onConnectionChange?.(false)

        // Auto-reconnect if enabled and not a clean close
        if (
          autoReconnect &&
          event.code !== 1000 &&
          reconnectAttemptRef.current < maxReconnectAttempts
        ) {
          const delay = getReconnectDelay()
          // Only log first few reconnection attempts
          if (reconnectAttemptRef.current < 3) {
            console.log(
              `Match WebSocket: reconnecting in ${delay}ms (attempt ${reconnectAttemptRef.current + 1}/${maxReconnectAttempts})`,
            )
          }
          reconnectTimeoutRef.current = setTimeout(() => {
            reconnectAttemptRef.current++
            connect()
          }, delay)
        }
      }

      ws.onerror = () => {
        // WebSocket errors are expected when server is unavailable
        // The onclose handler will manage reconnection
        if (import.meta.env.DEV && reconnectAttemptRef.current === 0) {
          console.warn(
            'Match WebSocket: connection failed (server may be unavailable)',
          )
        }
      }

      ws.onmessage = handleMessage

      wsRef.current = ws
    } catch (error) {
      console.error('Failed to create WebSocket connection:', error)
    }
  }, [
    autoReconnect,
    maxReconnectAttempts,
    getReconnectDelay,
    handleMessage,
    onConnectionChange,
  ])

  // Disconnect from WebSocket
  const disconnect = useCallback(() => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current)
      reconnectTimeoutRef.current = null
    }

    if (wsRef.current) {
      wsRef.current.close(1000, 'Client disconnect')
      wsRef.current = null
    }

    setIsConnected(false)
  }, [])

  // Manual reconnect
  const reconnect = useCallback(() => {
    disconnect()
    reconnectAttemptRef.current = 0
    connect()
  }, [connect, disconnect])

  // Connect on mount, disconnect on unmount
  useEffect(() => {
    mountedRef.current = true
    connect()

    return () => {
      mountedRef.current = false
      disconnect()
    }
  }, [connect, disconnect])

  return {
    isConnected,
    reconnect,
    disconnect,
  }
}
