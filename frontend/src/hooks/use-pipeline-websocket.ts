// usePipelineWebSocket Hook
// Provides real-time WebSocket connection for pipeline execution updates
//
// Feature: debug-recording-enhancement
// Implements: Requirements 2.1, 2.2, 2.3, 2.4, 2.5

import { useEffect, useRef, useState, useCallback } from 'react'
import type {
  PipelineStage,
  PipelineStepStatus,
} from '@/components/debug-recordings/pipeline-types'

// =============================================================================
// Pipeline Event Types (matching backend PipelineEvent enum)
// =============================================================================

export type PipelineStageType =
  | 'message_received'
  | 'ai_parsing'
  | 'parsing_complete'
  | 'medication_resolution'
  | 'offer_created'
  | 'request_created'
  | 'match_candidate_search'
  | { hierarchical_stage: { stage_number: number } }
  | 'score_calculation'
  | 'ai_review'
  | 'consensus_check'
  | 'contrastive_validation'
  | 'calibration'
  | 'match_created'
  | 'queue_added'
  | 'notification_sent'

export type AiOperation =
  | 'parsing'
  | 'review'
  | 'consensus_audit'
  | 'contrastive_validation'

export type MatchOutcome =
  | 'approved'
  | 'rejected'
  | 'pending_review'
  | 'auto_approved'
  | 'flagged'
  | 'no_match'

// Pipeline event types from backend
export interface MatchStartedEvent {
  type: 'match_started'
  match_id: string
  offer_id: string
  request_id: string
  timestamp: string
  session_id?: string
}

export interface StageCompletedEvent {
  type: 'stage_completed'
  match_id: string
  stage: PipelineStageType
  stage_name: string
  duration_ms: number
  candidates_in: number
  candidates_out: number
  summary: string
  timestamp: string
}

export interface AiProcessingStartedEvent {
  type: 'ai_processing_started'
  match_id: string
  model: string
  operation: AiOperation
  estimated_duration_ms?: number
  timestamp: string
}

export interface AiProcessingCompletedEvent {
  type: 'ai_processing_completed'
  match_id: string
  model: string
  operation: AiOperation
  duration_ms: number
  success: boolean
  timestamp: string
}

export interface MatchCompletedEvent {
  type: 'match_completed'
  match_id: string
  audit_record_id: string
  final_score: number
  outcome: MatchOutcome
  total_duration_ms: number
  stages_completed: number
  timestamp: string
}

export interface StageErrorEvent {
  type: 'stage_error'
  match_id: string
  stage: PipelineStageType
  stage_name: string
  error: string
  partial_results?: unknown
  recoverable: boolean
  timestamp: string
}

export interface MatchFailedEvent {
  type: 'match_failed'
  match_id: string
  error: string
  last_completed_stage?: PipelineStageType
  partial_audit_record_id?: string
  timestamp: string
}

export interface StageProgressEvent {
  type: 'stage_progress'
  match_id: string
  stage: PipelineStageType
  stage_name: string
  progress_percent: number
  message: string
  timestamp: string
}

export interface ConnectionEvent {
  type: 'connected'
  match_id?: string
  filter?: string
  session_id?: string
}

export type PipelineEvent =
  | MatchStartedEvent
  | StageCompletedEvent
  | AiProcessingStartedEvent
  | AiProcessingCompletedEvent
  | MatchCompletedEvent
  | StageErrorEvent
  | MatchFailedEvent
  | StageProgressEvent
  | ConnectionEvent

// =============================================================================
// Live Stage State
// =============================================================================

export interface LivePipelineStage {
  id: string
  stage: PipelineStage
  stageName: string
  status: PipelineStepStatus
  startedAt: string
  completedAt?: string
  durationMs?: number
  candidatesIn?: number
  candidatesOut?: number
  summary?: string
  error?: string
  progressPercent?: number
  progressMessage?: string
}

export interface LivePipelineState {
  matchId: string
  offerId?: string
  requestId?: string
  status: 'pending' | 'running' | 'completed' | 'error'
  stages: LivePipelineStage[]
  currentStage: string | null
  aiProcessing: {
    active: boolean
    model?: string
    operation?: AiOperation
    estimatedDurationMs?: number
    startedAt?: string
  }
  finalScore?: number
  outcome?: MatchOutcome
  auditRecordId?: string
  totalDurationMs?: number
  error?: string
  startedAt?: string
  completedAt?: string
}

// =============================================================================
// Hook Options and Return Types
// =============================================================================

export interface UsePipelineWebSocketOptions {
  /** Match ID to subscribe to (optional - if not provided, subscribes to all) */
  matchId?: string
  /** Session ID for correlation */
  sessionId?: string
  /** Authentication token */
  token?: string
  /** Callback when a stage completes */
  onStageComplete?: (event: StageCompletedEvent) => void
  /** Callback when match completes */
  onMatchComplete?: (event: MatchCompletedEvent) => void
  /** Callback when an error occurs */
  onError?: (event: StageErrorEvent | MatchFailedEvent) => void
  /** Callback when connection status changes */
  onConnectionChange?: (connected: boolean) => void
  /** Enable auto-reconnect (default: true) */
  autoReconnect?: boolean
  /** Maximum reconnect attempts (default: 10) */
  maxReconnectAttempts?: number
  /** Enable the WebSocket connection (default: true) */
  enabled?: boolean
}

export interface UsePipelineWebSocketReturn {
  /** Whether the WebSocket is connected */
  isConnected: boolean
  /** Current live pipeline state */
  pipelineState: LivePipelineState | null
  /** All received stages */
  stages: LivePipelineStage[]
  /** Current stage name */
  currentStage: string | null
  /** Subscribe to a specific match */
  subscribe: (matchId: string) => void
  /** Unsubscribe from current match */
  unsubscribe: () => void
  /** Manually reconnect */
  reconnect: () => void
  /** Disconnect */
  disconnect: () => void
  /** Connection error message */
  error: string | null
}

// =============================================================================
// Constants
// =============================================================================

const WS_BASE_URL = import.meta.env.VITE_WS_URL || 'ws://localhost:8081'
const INITIAL_RECONNECT_DELAY = 1000
const MAX_RECONNECT_DELAY = 30000

// =============================================================================
// Helper Functions
// =============================================================================

/** Convert backend stage type to frontend PipelineStage */
function convertStageType(stage: PipelineStageType): PipelineStage {
  if (typeof stage === 'string') {
    return stage as PipelineStage
  }
  if (typeof stage === 'object' && 'hierarchical_stage' in stage) {
    const num = stage.hierarchical_stage.stage_number
    return `hierarchical_stage_${num}` as PipelineStage
  }
  return 'message_received'
}

/** Generate a unique stage ID */
function generateStageId(matchId: string, stageName: string): string {
  return `${matchId}-${stageName}-${Date.now()}`
}

// =============================================================================
// Hook Implementation
// =============================================================================

/**
 * Hook for WebSocket connection to receive real-time pipeline updates.
 * Implements auto-reconnect with exponential backoff.
 *
 * @example
 * ```tsx
 * const { isConnected, pipelineState, stages } = usePipelineWebSocket({
 *   matchId: 'abc-123',
 *   onStageComplete: (stage) => console.log('Stage completed:', stage),
 *   onMatchComplete: (result) => console.log('Match completed:', result),
 * })
 * ```
 */
export function usePipelineWebSocket(
  options: UsePipelineWebSocketOptions = {},
): UsePipelineWebSocketReturn {
  const {
    matchId: initialMatchId,
    sessionId,
    token,
    onStageComplete,
    onMatchComplete,
    onError,
    onConnectionChange,
    autoReconnect = true,
    maxReconnectAttempts = 10,
    enabled = true,
  } = options

  // State
  const [isConnected, setIsConnected] = useState(false)
  const [pipelineState, setPipelineState] = useState<LivePipelineState | null>(
    null,
  )
  const [stages, setStages] = useState<LivePipelineStage[]>([])
  const [currentStage, setCurrentStage] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  // Refs
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectAttemptRef = useRef(0)
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const mountedRef = useRef(true)
  const matchIdRef = useRef<string | undefined>(initialMatchId)

  // Update matchId ref when prop changes
  useEffect(() => {
    matchIdRef.current = initialMatchId
  }, [initialMatchId])

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
        const message: PipelineEvent = JSON.parse(event.data)

        switch (message.type) {
          case 'connected': {
            // Connection confirmed
            setError(null)
            break
          }

          case 'match_started': {
            const evt = message as MatchStartedEvent
            setPipelineState({
              matchId: evt.match_id,
              offerId: evt.offer_id,
              requestId: evt.request_id,
              status: 'running',
              stages: [],
              currentStage: null,
              aiProcessing: { active: false },
              startedAt: evt.timestamp,
            })
            setStages([])
            setCurrentStage(null)
            break
          }

          case 'stage_completed': {
            const evt = message as StageCompletedEvent
            const newStage: LivePipelineStage = {
              id: generateStageId(evt.match_id, evt.stage_name),
              stage: convertStageType(evt.stage),
              stageName: evt.stage_name,
              status: 'success',
              startedAt: evt.timestamp,
              completedAt: evt.timestamp,
              durationMs: evt.duration_ms,
              candidatesIn: evt.candidates_in,
              candidatesOut: evt.candidates_out,
              summary: evt.summary,
            }

            setStages((prev) => [...prev, newStage])
            setCurrentStage(evt.stage_name)
            setPipelineState((prev) =>
              prev
                ? {
                    ...prev,
                    stages: [...prev.stages, newStage],
                    currentStage: evt.stage_name,
                  }
                : null,
            )

            onStageComplete?.(evt)
            break
          }

          case 'ai_processing_started': {
            const evt = message as AiProcessingStartedEvent
            setPipelineState((prev) =>
              prev
                ? {
                    ...prev,
                    aiProcessing: {
                      active: true,
                      model: evt.model,
                      operation: evt.operation,
                      estimatedDurationMs: evt.estimated_duration_ms,
                      startedAt: evt.timestamp,
                    },
                  }
                : null,
            )
            break
          }

          case 'ai_processing_completed': {
            // AiProcessingCompletedEvent received - update state
            setPipelineState((prev) =>
              prev
                ? {
                    ...prev,
                    aiProcessing: { active: false },
                  }
                : null,
            )
            break
          }

          case 'match_completed': {
            const evt = message as MatchCompletedEvent
            setPipelineState((prev) =>
              prev
                ? {
                    ...prev,
                    status: 'completed',
                    finalScore: evt.final_score,
                    outcome: evt.outcome,
                    auditRecordId: evt.audit_record_id,
                    totalDurationMs: evt.total_duration_ms,
                    completedAt: evt.timestamp,
                  }
                : null,
            )

            onMatchComplete?.(evt)
            break
          }

          case 'stage_error': {
            const evt = message as StageErrorEvent
            const errorStage: LivePipelineStage = {
              id: generateStageId(evt.match_id, evt.stage_name),
              stage: convertStageType(evt.stage),
              stageName: evt.stage_name,
              status: 'error',
              startedAt: evt.timestamp,
              error: evt.error,
            }

            setStages((prev) => [...prev, errorStage])
            setPipelineState((prev) =>
              prev
                ? {
                    ...prev,
                    stages: [...prev.stages, errorStage],
                    status: evt.recoverable ? 'running' : 'error',
                    error: evt.error,
                  }
                : null,
            )

            onError?.(evt)
            break
          }

          case 'match_failed': {
            const evt = message as MatchFailedEvent
            setPipelineState((prev) =>
              prev
                ? {
                    ...prev,
                    status: 'error',
                    error: evt.error,
                    completedAt: evt.timestamp,
                  }
                : null,
            )

            onError?.(evt)
            break
          }

          case 'stage_progress': {
            const evt = message as StageProgressEvent
            // Update the current stage with progress info
            setStages((prev) => {
              const lastIndex = prev.length - 1
              if (
                lastIndex >= 0 &&
                prev[lastIndex].stageName === evt.stage_name
              ) {
                const updated = [...prev]
                updated[lastIndex] = {
                  ...updated[lastIndex],
                  progressPercent: evt.progress_percent,
                  progressMessage: evt.message,
                }
                return updated
              }
              return prev
            })
            break
          }

          default:
            // Unknown event type - ignore
            break
        }
      } catch (err) {
        console.error('Failed to parse pipeline WebSocket message:', err)
      }
    },
    [onStageComplete, onMatchComplete, onError],
  )

  // Connect to WebSocket
  const connect = useCallback(() => {
    if (!enabled) return
    if (wsRef.current?.readyState === WebSocket.OPEN) return

    try {
      // Build WebSocket URL
      const currentMatchId = matchIdRef.current
      const wsPath = currentMatchId
        ? `${WS_BASE_URL}/ws/pipeline/${currentMatchId}`
        : `${WS_BASE_URL}/ws/pipeline`

      const params = new URLSearchParams()
      if (token) params.set('token', token)
      if (sessionId) params.set('session_id', sessionId)

      const wsUrl = params.toString()
        ? `${wsPath}?${params.toString()}`
        : wsPath

      const ws = new WebSocket(wsUrl)

      ws.onopen = () => {
        if (!mountedRef.current) return
        setIsConnected(true)
        setError(null)
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
          if (reconnectAttemptRef.current < 3) {
            console.log(
              `Pipeline WebSocket: reconnecting in ${delay}ms (attempt ${reconnectAttemptRef.current + 1}/${maxReconnectAttempts})`,
            )
          }
          reconnectTimeoutRef.current = setTimeout(() => {
            reconnectAttemptRef.current++
            connect()
          }, delay)
        }
      }

      ws.onerror = () => {
        if (import.meta.env.DEV && reconnectAttemptRef.current === 0) {
          console.warn(
            'Pipeline WebSocket: connection failed (server may be unavailable)',
          )
        }
        setError('Connection failed')
      }

      ws.onmessage = handleMessage

      wsRef.current = ws
    } catch (err) {
      console.error('Failed to create pipeline WebSocket connection:', err)
      setError('Failed to connect')
    }
  }, [
    enabled,
    token,
    sessionId,
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

  // Subscribe to a specific match
  const subscribe = useCallback(
    (newMatchId: string) => {
      matchIdRef.current = newMatchId
      // Reset state for new subscription
      setPipelineState(null)
      setStages([])
      setCurrentStage(null)
      setError(null)
      // Reconnect with new match ID
      disconnect()
      reconnectAttemptRef.current = 0
      connect()
    },
    [connect, disconnect],
  )

  // Unsubscribe from current match
  const unsubscribe = useCallback(() => {
    matchIdRef.current = undefined
    setPipelineState(null)
    setStages([])
    setCurrentStage(null)
    disconnect()
  }, [disconnect])

  // Manual reconnect
  const reconnect = useCallback(() => {
    disconnect()
    reconnectAttemptRef.current = 0
    connect()
  }, [connect, disconnect])

  // Connect on mount, disconnect on unmount
  useEffect(() => {
    mountedRef.current = true

    if (enabled && initialMatchId) {
      connect()
    }

    return () => {
      mountedRef.current = false
      disconnect()
    }
  }, [enabled, initialMatchId, connect, disconnect])

  return {
    isConnected,
    pipelineState,
    stages,
    currentStage,
    subscribe,
    unsubscribe,
    reconnect,
    disconnect,
    error,
  }
}
