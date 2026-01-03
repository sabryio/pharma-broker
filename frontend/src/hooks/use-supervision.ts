import { useEffect, useRef, useState, useCallback } from 'react'
import {
  useQuery,
  useMutation,
  useQueryClient,
  keepPreviousData,
} from '@tanstack/react-query'
import {
  getSupervisionStats,
  getSupervisionConfig,
  updateSupervisionConfig,
  getSupervisionAudit,
  overrideDecision,
  undoApproval,
  pauseSystem,
  resumeSystem,
} from '@/api/supervision'
import type {
  AuditQueryParams,
  AutoApproveConfig,
  LiveFeedItem,
  AutoApproveEvent,
  QueuedForReviewEvent,
  AutoApproveBlockedEvent,
  AutoApproveOverrideEvent,
  AutoApproveUndoEvent,
  AutoApprovePauseEvent,
} from '@/schema/supervision'
import { useAppSelector } from '@/store/hooks'
import { selectUserId } from '@/store/slices/sessionSlice'
import { queryKeys } from './query-keys'

// =============================================================================
// WebSocket Types
// =============================================================================

interface SupervisionWebSocketMessage {
  type:
    | 'AutoApproved'
    | 'AutoApproveOverridden'
    | 'AutoApproveUndone'
    | 'AutoApprovePaused'
    | 'AutoApproveResumed'
    | 'QueuedForReview'
    | 'AutoApproveBlocked'
    | 'Ping'
  payload?: unknown
}

// =============================================================================
// Hooks
// =============================================================================

/**
 * Hook to fetch supervision statistics
 * Requirements: 3.2
 */
export function useSupervisionStats() {
  const autoRefreshInterval = useAppSelector(
    (state) => state.session.preferences.autoRefreshInterval,
  )

  return useQuery({
    queryKey: queryKeys.supervision.stats(),
    queryFn: getSupervisionStats,
    staleTime: 10 * 1000,
    refetchInterval: autoRefreshInterval > 0 ? autoRefreshInterval : 5000,
  })
}

/**
 * Hook to fetch supervision config with stats
 * Requirements: 5.1
 */
export function useSupervisionConfig() {
  return useQuery({
    queryKey: queryKeys.supervision.config(),
    queryFn: getSupervisionConfig,
    staleTime: 30 * 1000,
  })
}

/**
 * Hook to update supervision config
 * Requirements: 5.1
 */
export function useUpdateSupervisionConfig() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (config: AutoApproveConfig) => updateSupervisionConfig(config),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.supervision.all })
    },
  })
}

/**
 * Hook to fetch supervision audit log
 * Requirements: 2.3
 */
export function useSupervisionAudit(params: AuditQueryParams = {}) {
  return useQuery({
    queryKey: queryKeys.supervision.auditFiltered(params),
    queryFn: () => getSupervisionAudit(params),
    placeholderData: keepPreviousData,
    staleTime: 10 * 1000,
  })
}

/**
 * Hook to override an AI decision
 * Requirements: 4.1
 */
export function useOverrideDecision() {
  const queryClient = useQueryClient()
  const userId = useAppSelector(selectUserId)

  return useMutation({
    mutationFn: ({ matchId, reason }: { matchId: string; reason: string }) =>
      overrideDecision(matchId, { userId, reason }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.supervision.all })
      queryClient.invalidateQueries({ queryKey: queryKeys.matchReviews.all })
    },
  })
}

/**
 * Hook to undo an auto-approval
 * Requirements: 4.2
 */
export function useUndoApproval() {
  const queryClient = useQueryClient()
  const userId = useAppSelector(selectUserId)

  return useMutation({
    mutationFn: (matchId: string) => undoApproval(matchId, { userId }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.supervision.all })
      queryClient.invalidateQueries({ queryKey: queryKeys.matchReviews.all })
    },
  })
}

/**
 * Hook to pause the auto-approve system
 */
export function usePauseSystem() {
  const queryClient = useQueryClient()
  const userId = useAppSelector(selectUserId)

  return useMutation({
    mutationFn: (reason: string) => pauseSystem({ userId, reason }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.supervision.all })
    },
  })
}

/**
 * Hook to resume the auto-approve system
 */
export function useResumeSystem() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: () => resumeSystem(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.supervision.all })
    },
  })
}

// =============================================================================
// WebSocket Hook for Real-Time Updates
// Requirements: 3.1, 3.3
// =============================================================================

const WS_BASE_URL = import.meta.env.VITE_WS_URL || 'ws://localhost:8081'
const INITIAL_RECONNECT_DELAY = 1000
const MAX_RECONNECT_DELAY = 30000
const MAX_FEED_ITEMS = 50

export interface UseSupervisionWebSocketOptions {
  onAutoApproved?: (event: AutoApproveEvent) => void
  onOverridden?: (event: AutoApproveOverrideEvent) => void
  onUndone?: (event: AutoApproveUndoEvent) => void
  onPaused?: (event: AutoApprovePauseEvent) => void
  onResumed?: () => void
  onQueuedForReview?: (event: QueuedForReviewEvent) => void
  onBlocked?: (event: AutoApproveBlockedEvent) => void
  onConnectionChange?: (connected: boolean) => void
  autoReconnect?: boolean
  maxReconnectAttempts?: number
}

export interface UseSupervisionWebSocketReturn {
  isConnected: boolean
  liveFeed: LiveFeedItem[]
  reconnect: () => void
  disconnect: () => void
  clearFeed: () => void
}

/**
 * Hook for WebSocket connection to receive real-time supervision updates
 * Requirements: 3.1, 3.3
 */
export function useSupervisionWebSocket(
  options: UseSupervisionWebSocketOptions = {},
): UseSupervisionWebSocketReturn {
  const {
    onAutoApproved,
    onOverridden,
    onUndone,
    onPaused,
    onResumed,
    onQueuedForReview,
    onBlocked,
    onConnectionChange,
    autoReconnect = true,
    maxReconnectAttempts = 10,
  } = options

  const queryClient = useQueryClient()
  const [isConnected, setIsConnected] = useState(false)
  const [liveFeed, setLiveFeed] = useState<LiveFeedItem[]>([])
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectAttemptRef = useRef(0)
  const reconnectTimeoutRef = useRef<NodeJS.Timeout | null>(null)
  const mountedRef = useRef(true)

  const getReconnectDelay = useCallback(() => {
    const delay = Math.min(
      INITIAL_RECONNECT_DELAY * Math.pow(2, reconnectAttemptRef.current),
      MAX_RECONNECT_DELAY,
    )
    return delay
  }, [])

  const addToFeed = useCallback((item: LiveFeedItem) => {
    setLiveFeed((prev) => {
      const newFeed = [item, ...prev]
      return newFeed.slice(0, MAX_FEED_ITEMS)
    })
  }, [])

  const handleMessage = useCallback(
    (event: MessageEvent) => {
      try {
        const message: SupervisionWebSocketMessage = JSON.parse(event.data)

        switch (message.type) {
          case 'AutoApproved': {
            const payload = message.payload as AutoApproveEvent
            onAutoApproved?.(payload)
            addToFeed({
              id: `${payload.matchId}-${Date.now()}`,
              matchId: payload.matchId,
              timestamp: payload.approvedAt,
              action: 'approved',
              aiConfidence: payload.aiConfidence,
              aiExplanation: payload.aiExplanation,
              offerMedication: payload.offerMedication,
              requestMedication: payload.requestMedication,
              isBorderline: payload.isBorderline,
            })
            queryClient.invalidateQueries({
              queryKey: queryKeys.supervision.stats(),
            })
            break
          }
          case 'AutoApproveOverridden': {
            const payload = message.payload as AutoApproveOverrideEvent
            onOverridden?.(payload)
            queryClient.invalidateQueries({
              queryKey: queryKeys.supervision.all,
            })
            break
          }
          case 'AutoApproveUndone': {
            const payload = message.payload as AutoApproveUndoEvent
            onUndone?.(payload)
            queryClient.invalidateQueries({
              queryKey: queryKeys.supervision.all,
            })
            break
          }
          case 'AutoApprovePaused': {
            const payload = message.payload as AutoApprovePauseEvent
            onPaused?.(payload)
            queryClient.invalidateQueries({
              queryKey: queryKeys.supervision.stats(),
            })
            break
          }
          case 'AutoApproveResumed': {
            onResumed?.()
            queryClient.invalidateQueries({
              queryKey: queryKeys.supervision.stats(),
            })
            break
          }
          case 'QueuedForReview': {
            const payload = message.payload as QueuedForReviewEvent
            onQueuedForReview?.(payload)
            addToFeed({
              id: `${payload.matchId}-${Date.now()}`,
              matchId: payload.matchId,
              timestamp: payload.queuedAt,
              action: 'queued',
              aiConfidence: payload.aiConfidence,
              aiExplanation: payload.aiExplanation,
              offerMedication: payload.offerMedication,
              requestMedication: payload.requestMedication,
              isBorderline: payload.isBorderline,
            })
            queryClient.invalidateQueries({
              queryKey: queryKeys.supervision.stats(),
            })
            queryClient.invalidateQueries({
              queryKey: queryKeys.matchReviews.all,
            })
            break
          }
          case 'AutoApproveBlocked': {
            const payload = message.payload as AutoApproveBlockedEvent
            onBlocked?.(payload)
            addToFeed({
              id: `${payload.matchId}-${Date.now()}`,
              matchId: payload.matchId,
              timestamp: payload.blockedAt,
              action: 'blocked',
              aiConfidence: 0,
              aiExplanation: payload.blockReason,
              offerMedication: payload.offerMedication,
              requestMedication: payload.requestMedication,
              isBorderline: false,
              blockReason: payload.blockReason,
            })
            queryClient.invalidateQueries({
              queryKey: queryKeys.supervision.stats(),
            })
            break
          }
          case 'Ping':
            // Heartbeat - no action needed
            break
        }
      } catch (error) {
        console.error('Failed to parse supervision WebSocket message:', error)
      }
    },
    [
      onAutoApproved,
      onOverridden,
      onUndone,
      onPaused,
      onResumed,
      onQueuedForReview,
      onBlocked,
      addToFeed,
      queryClient,
    ],
  )

  const connect = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      return
    }

    try {
      const ws = new WebSocket(`${WS_BASE_URL}/ws`)

      ws.onopen = () => {
        if (!mountedRef.current) return
        console.log('Supervision WebSocket connected')
        setIsConnected(true)
        onConnectionChange?.(true)
        reconnectAttemptRef.current = 0
      }

      ws.onclose = (event) => {
        if (!mountedRef.current) return
        setIsConnected(false)
        onConnectionChange?.(false)

        if (
          autoReconnect &&
          event.code !== 1000 &&
          reconnectAttemptRef.current < maxReconnectAttempts
        ) {
          const delay = getReconnectDelay()
          // Only log first few reconnection attempts
          if (reconnectAttemptRef.current < 3) {
            console.log(
              `Supervision WebSocket: reconnecting in ${delay}ms (attempt ${reconnectAttemptRef.current + 1}/${maxReconnectAttempts})`,
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
            'Supervision WebSocket: connection failed (server may be unavailable)',
          )
        }
      }

      ws.onmessage = handleMessage

      wsRef.current = ws
    } catch (error) {
      console.error('Failed to create supervision WebSocket connection:', error)
    }
  }, [
    autoReconnect,
    maxReconnectAttempts,
    getReconnectDelay,
    handleMessage,
    onConnectionChange,
  ])

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

  const reconnect = useCallback(() => {
    disconnect()
    reconnectAttemptRef.current = 0
    connect()
  }, [connect, disconnect])

  const clearFeed = useCallback(() => {
    setLiveFeed([])
  }, [])

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
    liveFeed,
    reconnect,
    disconnect,
    clearFeed,
  }
}
