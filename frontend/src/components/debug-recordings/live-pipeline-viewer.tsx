// Live Pipeline Viewer Component
// Wrapper component that connects PipelineViewer to live WebSocket data
//
// Feature: debug-recording-enhancement
// Implements: Requirements 2.1, 2.2, 2.3, 2.4

import { useEffect, useMemo, useCallback } from 'react'
import { cn } from '@/lib/utils'
import { Wifi, WifiOff, RefreshCw, Radio } from 'lucide-react'
import { PipelineViewer } from './pipeline-viewer'
import type { PipelineRecording, PipelineStep } from './pipeline-types'
import {
  usePipelineWebSocket,
  type LivePipelineState,
  type LivePipelineStage,
  type StageCompletedEvent,
  type MatchCompletedEvent,
  type StageErrorEvent,
  type MatchFailedEvent,
} from '@/hooks/use-pipeline-websocket'
import { useAppDispatch, useAppSelector } from '@/store/hooks'
import { recordingsActions } from '@/store/slices/recordingsSlice'
import {
  selectLivePipelineState,
  selectPipelineConnectionStatus,
} from '@/store/slices'

interface LivePipelineViewerProps {
  /** Match ID to subscribe to for live updates */
  matchId: string
  /** Session ID for correlation */
  sessionId?: string
  /** Authentication token */
  token?: string
  /** Callback when match completes */
  onMatchComplete?: (event: MatchCompletedEvent) => void
  /** Callback when an error occurs */
  onError?: (event: StageErrorEvent | MatchFailedEvent) => void
  /** Close handler */
  onClose?: () => void
  /** Export handler */
  onExport?: () => void
  /** Whether to show connection status indicator */
  showConnectionStatus?: boolean
}

/**
 * Convert LivePipelineState to PipelineRecording format for the viewer
 */
function convertToPipelineRecording(
  state: LivePipelineState,
): PipelineRecording {
  const steps: PipelineStep[] = state.stages.map((stage) => ({
    id: stage.id,
    stage: stage.stage,
    status: stage.status,
    startedAt: stage.startedAt,
    completedAt: stage.completedAt,
    durationMs: stage.durationMs,
    error: stage.error,
    metadata: {
      candidatesIn: stage.candidatesIn,
      candidatesOut: stage.candidatesOut,
      summary: stage.summary,
      progressPercent: stage.progressPercent,
      progressMessage: stage.progressMessage,
    },
  }))

  // Calculate total duration
  let totalDurationMs: number | undefined
  if (state.startedAt) {
    const startTime = new Date(state.startedAt).getTime()
    const endTime = state.completedAt
      ? new Date(state.completedAt).getTime()
      : Date.now()
    totalDurationMs = endTime - startTime
  }

  // Map outcome to finalStatus
  let finalStatus:
    | 'auto_approved'
    | 'needs_review'
    | 'auto_rejected'
    | undefined
  if (state.outcome) {
    switch (state.outcome) {
      case 'auto_approved':
      case 'approved':
        finalStatus = 'auto_approved'
        break
      case 'pending_review':
      case 'flagged':
        finalStatus = 'needs_review'
        break
      case 'rejected':
      case 'no_match':
        finalStatus = 'auto_rejected'
        break
    }
  }

  return {
    id: `live-${state.matchId}`,
    matchId: state.matchId,
    offerId: state.offerId ?? '',
    requestId: state.requestId ?? '',
    startedAt: state.startedAt ?? new Date().toISOString(),
    completedAt: state.completedAt,
    totalDurationMs,
    status:
      state.status === 'completed'
        ? 'completed'
        : state.status === 'error'
          ? 'error'
          : 'running',
    steps,
    finalScore: state.finalScore,
    finalStatus,
  }
}

/**
 * Connection status indicator component
 */
function ConnectionStatusIndicator({
  status,
  onReconnect,
}: {
  status: 'disconnected' | 'connecting' | 'connected' | 'error'
  onReconnect?: () => void
}) {
  return (
    <div
      className={cn(
        'flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium transition-all',
        status === 'connected'
          ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30'
          : status === 'connecting'
            ? 'bg-amber-500/20 text-amber-400 border border-amber-500/30 animate-pulse'
            : status === 'error'
              ? 'bg-red-500/20 text-red-400 border border-red-500/30'
              : 'bg-slate-500/20 text-slate-400 border border-slate-500/30',
      )}
    >
      {status === 'connected' ? (
        <>
          <Wifi className="w-3 h-3" />
          <span>Live</span>
          <Radio className="w-3 h-3 animate-pulse" />
        </>
      ) : status === 'connecting' ? (
        <>
          <RefreshCw className="w-3 h-3 animate-spin" />
          <span>Connecting...</span>
        </>
      ) : status === 'error' ? (
        <>
          <WifiOff className="w-3 h-3" />
          <span>Disconnected</span>
          {onReconnect && (
            <button
              onClick={onReconnect}
              className="ml-1 underline hover:no-underline"
            >
              Retry
            </button>
          )}
        </>
      ) : (
        <>
          <WifiOff className="w-3 h-3" />
          <span>Offline</span>
        </>
      )}
    </div>
  )
}

/**
 * AI Processing indicator component
 */
function AiProcessingIndicator({
  model,
  operation,
  estimatedDurationMs,
}: {
  model?: string
  operation?: string
  estimatedDurationMs?: number
}) {
  return (
    <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-purple-500/20 text-purple-400 border border-purple-500/30 text-xs font-medium animate-pulse">
      <span className="text-base">🤖</span>
      <span>AI Processing</span>
      {model && <span className="text-purple-300">({model})</span>}
      {operation && <span className="text-purple-300">• {operation}</span>}
      {estimatedDurationMs && (
        <span className="text-purple-300">
          ~{Math.round(estimatedDurationMs / 1000)}s
        </span>
      )}
    </div>
  )
}

/**
 * Live Pipeline Viewer - connects to WebSocket for real-time pipeline updates
 *
 * @example
 * ```tsx
 * <LivePipelineViewer
 *   matchId="abc-123"
 *   onMatchComplete={(result) => console.log('Match completed:', result)}
 *   onClose={() => setShowViewer(false)}
 * />
 * ```
 */
export function LivePipelineViewer({
  matchId,
  sessionId,
  token,
  onMatchComplete,
  onError,
  onClose,
  onExport,
  showConnectionStatus = true,
}: LivePipelineViewerProps) {
  const dispatch = useAppDispatch()

  // Get live pipeline state from Redux
  const livePipelineState = useAppSelector((state) =>
    selectLivePipelineState(state, matchId),
  )
  const connectionStatus = useAppSelector(selectPipelineConnectionStatus)

  // Handle stage completion - dispatch to Redux
  const handleStageComplete = useCallback(
    (event: StageCompletedEvent) => {
      const stage: LivePipelineStage = {
        id: `${event.match_id}-${event.stage_name}-${Date.now()}`,
        stage: event.stage_name as any,
        stageName: event.stage_name,
        status: 'success',
        startedAt: event.timestamp,
        completedAt: event.timestamp,
        durationMs: event.duration_ms,
        candidatesIn: event.candidates_in,
        candidatesOut: event.candidates_out,
        summary: event.summary,
      }
      dispatch(
        recordingsActions.pipelineStageCompleted({
          matchId: event.match_id,
          stage,
        }),
      )
    },
    [dispatch],
  )

  // Handle match completion
  const handleMatchComplete = useCallback(
    (event: MatchCompletedEvent) => {
      dispatch(
        recordingsActions.pipelineMatchCompleted({
          matchId: event.match_id,
          auditRecordId: event.audit_record_id,
          finalScore: event.final_score,
          outcome: event.outcome,
          totalDurationMs: event.total_duration_ms,
          timestamp: event.timestamp,
        }),
      )
      onMatchComplete?.(event)
    },
    [dispatch, onMatchComplete],
  )

  // Handle errors
  const handleError = useCallback(
    (event: StageErrorEvent | MatchFailedEvent) => {
      if (event.type === 'stage_error') {
        const stageEvent = event as StageErrorEvent
        const stage: LivePipelineStage = {
          id: `${stageEvent.match_id}-${stageEvent.stage_name}-${Date.now()}`,
          stage: stageEvent.stage_name as any,
          stageName: stageEvent.stage_name,
          status: 'error',
          startedAt: stageEvent.timestamp,
          error: stageEvent.error,
        }
        dispatch(
          recordingsActions.pipelineStageError({
            matchId: stageEvent.match_id,
            stage,
            recoverable: stageEvent.recoverable,
          }),
        )
      } else {
        const failedEvent = event as MatchFailedEvent
        dispatch(
          recordingsActions.pipelineMatchFailed({
            matchId: failedEvent.match_id,
            error: failedEvent.error,
            timestamp: failedEvent.timestamp,
          }),
        )
      }
      onError?.(event)
    },
    [dispatch, onError],
  )

  // Handle connection status changes
  const handleConnectionChange = useCallback(
    (connected: boolean) => {
      dispatch(
        recordingsActions.setConnectionStatus({
          status: connected ? 'connected' : 'disconnected',
        }),
      )
    },
    [dispatch],
  )

  // Connect to WebSocket
  const {
    isConnected,
    pipelineState: hookPipelineState,
    reconnect,
  } = usePipelineWebSocket({
    matchId,
    sessionId,
    token,
    onStageComplete: handleStageComplete,
    onMatchComplete: handleMatchComplete,
    onError: handleError,
    onConnectionChange: handleConnectionChange,
    enabled: true,
  })

  // Initialize pipeline state when match starts
  useEffect(() => {
    if (hookPipelineState && !livePipelineState) {
      dispatch(
        recordingsActions.updateLivePipelineState({
          matchId,
          pipelineState: hookPipelineState,
        }),
      )
    }
  }, [hookPipelineState, livePipelineState, matchId, dispatch])

  // Subscribe to pipeline on mount
  useEffect(() => {
    dispatch(recordingsActions.subscribeToPipeline(matchId))
    return () => {
      dispatch(recordingsActions.unsubscribeFromPipeline())
    }
  }, [matchId, dispatch])

  // Convert live state to PipelineRecording format
  const recording = useMemo(() => {
    const state = livePipelineState ?? hookPipelineState
    if (!state) return null
    return convertToPipelineRecording(state)
  }, [livePipelineState, hookPipelineState])

  // Get AI processing state
  const aiProcessing =
    livePipelineState?.aiProcessing ?? hookPipelineState?.aiProcessing

  return (
    <div className="relative">
      {/* Connection and AI status indicators */}
      {(showConnectionStatus || aiProcessing?.active) && (
        <div className="absolute top-4 right-4 z-10 flex items-center gap-2">
          {aiProcessing?.active && (
            <AiProcessingIndicator
              model={aiProcessing.model}
              operation={aiProcessing.operation}
              estimatedDurationMs={aiProcessing.estimatedDurationMs}
            />
          )}
          {showConnectionStatus && (
            <ConnectionStatusIndicator
              status={isConnected ? 'connected' : connectionStatus}
              onReconnect={reconnect}
            />
          )}
        </div>
      )}

      {/* Pipeline Viewer */}
      <PipelineViewer
        recording={recording}
        onClose={onClose}
        onRefresh={reconnect}
        onExport={onExport}
      />
    </div>
  )
}

export default LivePipelineViewer
