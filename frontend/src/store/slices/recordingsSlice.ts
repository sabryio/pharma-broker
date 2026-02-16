// Recordings Slice
// State management for debug recordings
//
// Feature: debug-recording-enhancement
// Implements: Requirements 2.1, 2.2, 2.3, 2.4

import {
  createSelector,
  createSlice,
  type PayloadAction,
} from '@reduxjs/toolkit'
import type {
  MatchRecording,
  MatchRecordingSnapshot,
} from '@/components/debug-recordings/types'
import type {
  LivePipelineStage,
  LivePipelineState,
  MatchOutcome,
  AiOperation,
} from '@/hooks/use-pipeline-websocket'

type ViewMode = 'overview' | 'recordings' | 'pipeline' | 'analytics' | 'audit'

interface PlaybackState {
  recordingId: string | null
  currentIndex: number
  isPlaying: boolean
  speed: number
}

// Live pipeline state for real-time WebSocket updates
interface LivePipelineData {
  /** Current pipeline state by match ID */
  states: Record<string, LivePipelineState>
  /** Currently subscribed match ID */
  subscribedMatchId: string | null
  /** WebSocket connection status */
  connectionStatus: 'disconnected' | 'connecting' | 'connected' | 'error'
  /** Connection error message */
  connectionError: string | null
}

interface RecordingsState {
  recordings: Record<string, MatchRecording>
  activeRecordingId: string | null
  isRecording: boolean
  playback: PlaybackState
  viewMode: ViewMode
  selectedRecordingId: string | null
  maxRecordings: number
  maxSnapshots: number
  // New state for live pipeline data (Task 10.2)
  livePipeline: LivePipelineData
}

const initialState: RecordingsState = {
  recordings: {},
  activeRecordingId: null,
  isRecording: true,
  playback: {
    recordingId: null,
    currentIndex: 0,
    isPlaying: false,
    speed: 1,
  },
  viewMode: 'overview',
  selectedRecordingId: null,
  maxRecordings: 50,
  maxSnapshots: 100,
  // Initial live pipeline state
  livePipeline: {
    states: {},
    subscribedMatchId: null,
    connectionStatus: 'disconnected',
    connectionError: null,
  },
}

export const recordingsSlice = createSlice({
  name: 'recordings',
  initialState,
  reducers: {
    startRecording: (state, action: PayloadAction<string>) => {
      const matchId = action.payload
      if (!state.recordings[matchId]) {
        // Enforce max recordings limit
        const recordingIds = Object.keys(state.recordings)
        if (recordingIds.length >= state.maxRecordings) {
          const oldest = Object.values(state.recordings).sort(
            (a, b) =>
              new Date(a.startedAt).getTime() - new Date(b.startedAt).getTime(),
          )[0]
          if (oldest) {
            delete state.recordings[oldest.id]
          }
        }
        state.recordings[matchId] = {
          id: matchId,
          matchId,
          startedAt: new Date(),
          snapshots: [],
        }
      }
      state.activeRecordingId = matchId
    },
    stopRecording: (state) => {
      if (
        state.activeRecordingId &&
        state.recordings[state.activeRecordingId]
      ) {
        const recording = state.recordings[state.activeRecordingId]
        if (recording) {
          recording.endedAt = new Date()
          recording.duration =
            new Date(recording.endedAt).getTime() -
            new Date(recording.startedAt).getTime()
          // Determine outcome from last snapshot
          const lastSnapshot =
            recording.snapshots[recording.snapshots.length - 1]
          if (lastSnapshot) {
            recording.outcome =
              lastSnapshot.event.type === 'approve'
                ? 'approved'
                : lastSnapshot.event.type === 'reject'
                  ? 'rejected'
                  : 'pending'
          }
        }
      }
      state.activeRecordingId = null
    },
    addSnapshot: (
      state,
      action: PayloadAction<{
        matchId: string
        snapshot: MatchRecordingSnapshot
      }>,
    ) => {
      const { matchId, snapshot } = action.payload
      if (state.recordings[matchId]) {
        const recording = state.recordings[matchId]
        // Enforce max snapshots
        if (recording.snapshots.length >= state.maxSnapshots) {
          recording.snapshots.shift()
        }
        recording.snapshots.push(snapshot)
      }
    },
    deleteRecording: (state, action: PayloadAction<string>) => {
      delete state.recordings[action.payload]
      if (state.selectedRecordingId === action.payload) {
        state.selectedRecordingId = null
      }
      if (state.activeRecordingId === action.payload) {
        state.activeRecordingId = null
      }
      if (state.playback.recordingId === action.payload) {
        state.playback = initialState.playback
      }
    },
    clearAllRecordings: (state) => {
      state.recordings = {}
      state.activeRecordingId = null
      state.selectedRecordingId = null
      state.playback = initialState.playback
    },
    toggleRecording: (state) => {
      state.isRecording = !state.isRecording
    },
    setIsRecording: (state, action: PayloadAction<boolean>) => {
      state.isRecording = action.payload
    },
    setViewMode: (state, action: PayloadAction<ViewMode>) => {
      state.viewMode = action.payload
    },
    selectRecording: (state, action: PayloadAction<string | null>) => {
      state.selectedRecordingId = action.payload
    },
    // Playback controls
    startPlayback: (state, action: PayloadAction<string>) => {
      state.playback.recordingId = action.payload
      state.playback.currentIndex = 0
      state.playback.isPlaying = true
    },
    pausePlayback: (state) => {
      state.playback.isPlaying = false
    },
    resumePlayback: (state) => {
      state.playback.isPlaying = true
    },
    stopPlayback: (state) => {
      state.playback = initialState.playback
    },
    setPlaybackIndex: (state, action: PayloadAction<number>) => {
      state.playback.currentIndex = action.payload
    },
    nextSnapshot: (state) => {
      const recording = state.playback.recordingId
        ? state.recordings[state.playback.recordingId]
        : null
      if (
        recording &&
        state.playback.currentIndex < recording.snapshots.length - 1
      ) {
        state.playback.currentIndex += 1
      }
    },
    previousSnapshot: (state) => {
      if (state.playback.currentIndex > 0) {
        state.playback.currentIndex -= 1
      }
    },
    setPlaybackSpeed: (state, action: PayloadAction<number>) => {
      state.playback.speed = action.payload
    },
    // Import/Export
    importRecordings: (
      state,
      action: PayloadAction<Record<string, MatchRecording>>,
    ) => {
      state.recordings = { ...state.recordings, ...action.payload }
    },
    setMaxRecordings: (state, action: PayloadAction<number>) => {
      state.maxRecordings = action.payload
    },
    setMaxSnapshots: (state, action: PayloadAction<number>) => {
      state.maxSnapshots = action.payload
    },

    // ==========================================================================
    // Live Pipeline Actions (Task 10.2)
    // Feature: debug-recording-enhancement
    // Implements: Requirements 2.1, 2.2, 2.3, 2.4
    // ==========================================================================

    /** Set WebSocket connection status */
    setConnectionStatus: (
      state,
      action: PayloadAction<{
        status: 'disconnected' | 'connecting' | 'connected' | 'error'
        error?: string
      }>,
    ) => {
      state.livePipeline.connectionStatus = action.payload.status
      state.livePipeline.connectionError = action.payload.error ?? null
    },

    /** Subscribe to a match for live pipeline updates */
    subscribeToPipeline: (state, action: PayloadAction<string>) => {
      state.livePipeline.subscribedMatchId = action.payload
      state.livePipeline.connectionStatus = 'connecting'
    },

    /** Unsubscribe from pipeline updates */
    unsubscribeFromPipeline: (state) => {
      state.livePipeline.subscribedMatchId = null
    },

    /** Handle match started event */
    pipelineMatchStarted: (
      state,
      action: PayloadAction<{
        matchId: string
        offerId: string
        requestId: string
        timestamp: string
        sessionId?: string
      }>,
    ) => {
      const { matchId, offerId, requestId, timestamp } = action.payload
      state.livePipeline.states[matchId] = {
        matchId,
        offerId,
        requestId,
        status: 'running',
        stages: [],
        currentStage: null,
        aiProcessing: { active: false },
        startedAt: timestamp,
      }
    },

    /** Handle stage completed event */
    pipelineStageCompleted: (
      state,
      action: PayloadAction<{
        matchId: string
        stage: LivePipelineStage
      }>,
    ) => {
      const { matchId, stage } = action.payload
      const pipelineState = state.livePipeline.states[matchId]
      if (pipelineState) {
        pipelineState.stages.push(stage)
        pipelineState.currentStage = stage.stageName
      }
    },

    /** Handle AI processing started event */
    pipelineAiStarted: (
      state,
      action: PayloadAction<{
        matchId: string
        model: string
        operation: AiOperation
        estimatedDurationMs?: number
        timestamp: string
      }>,
    ) => {
      const { matchId, model, operation, estimatedDurationMs, timestamp } =
        action.payload
      const pipelineState = state.livePipeline.states[matchId]
      if (pipelineState) {
        pipelineState.aiProcessing = {
          active: true,
          model,
          operation,
          estimatedDurationMs,
          startedAt: timestamp,
        }
      }
    },

    /** Handle AI processing completed event */
    pipelineAiCompleted: (
      state,
      action: PayloadAction<{
        matchId: string
      }>,
    ) => {
      const { matchId } = action.payload
      const pipelineState = state.livePipeline.states[matchId]
      if (pipelineState) {
        pipelineState.aiProcessing = { active: false }
      }
    },

    /** Handle match completed event */
    pipelineMatchCompleted: (
      state,
      action: PayloadAction<{
        matchId: string
        auditRecordId: string
        finalScore: number
        outcome: MatchOutcome
        totalDurationMs: number
        timestamp: string
      }>,
    ) => {
      const {
        matchId,
        auditRecordId,
        finalScore,
        outcome,
        totalDurationMs,
        timestamp,
      } = action.payload
      const pipelineState = state.livePipeline.states[matchId]
      if (pipelineState) {
        pipelineState.status = 'completed'
        pipelineState.auditRecordId = auditRecordId
        pipelineState.finalScore = finalScore
        pipelineState.outcome = outcome
        pipelineState.totalDurationMs = totalDurationMs
        pipelineState.completedAt = timestamp
      }
    },

    /** Handle stage error event */
    pipelineStageError: (
      state,
      action: PayloadAction<{
        matchId: string
        stage: LivePipelineStage
        recoverable: boolean
      }>,
    ) => {
      const { matchId, stage, recoverable } = action.payload
      const pipelineState = state.livePipeline.states[matchId]
      if (pipelineState) {
        pipelineState.stages.push(stage)
        if (!recoverable) {
          pipelineState.status = 'error'
          pipelineState.error = stage.error
        }
      }
    },

    /** Handle match failed event */
    pipelineMatchFailed: (
      state,
      action: PayloadAction<{
        matchId: string
        error: string
        timestamp: string
      }>,
    ) => {
      const { matchId, error, timestamp } = action.payload
      const pipelineState = state.livePipeline.states[matchId]
      if (pipelineState) {
        pipelineState.status = 'error'
        pipelineState.error = error
        pipelineState.completedAt = timestamp
      }
    },

    /** Handle stage progress event */
    pipelineStageProgress: (
      state,
      action: PayloadAction<{
        matchId: string
        stageName: string
        progressPercent: number
        message: string
      }>,
    ) => {
      const { matchId, stageName, progressPercent, message } = action.payload
      const pipelineState = state.livePipeline.states[matchId]
      if (pipelineState) {
        // Update the last stage with progress info
        const lastStage = pipelineState.stages[pipelineState.stages.length - 1]
        if (lastStage && lastStage.stageName === stageName) {
          lastStage.progressPercent = progressPercent
          lastStage.progressMessage = message
        }
      }
    },

    /** Update entire pipeline state (for bulk updates from hook) */
    updateLivePipelineState: (
      state,
      action: PayloadAction<{
        matchId: string
        pipelineState: LivePipelineState
      }>,
    ) => {
      const { matchId, pipelineState } = action.payload
      state.livePipeline.states[matchId] = pipelineState
    },

    /** Clear live pipeline state for a match */
    clearLivePipelineState: (state, action: PayloadAction<string>) => {
      delete state.livePipeline.states[action.payload]
    },

    /** Clear all live pipeline states */
    clearAllLivePipelineStates: (state) => {
      state.livePipeline.states = {}
      state.livePipeline.subscribedMatchId = null
    },
  },
})

export const recordingsActions = recordingsSlice.actions

export default recordingsSlice.reducer

// Selectors
export const selectRecordingsMap = (state: { recordings: RecordingsState }) =>
  state.recordings.recordings

export const selectRecordingsArray = createSelector(
  [selectRecordingsMap],
  (recordingsMap) =>
    Object.values(recordingsMap).sort(
      (a, b) =>
        new Date(b.startedAt).getTime() - new Date(a.startedAt).getTime(),
    ),
)

export const selectRecordingCount = (state: { recordings: RecordingsState }) =>
  Object.keys(state.recordings.recordings).length

export const selectActiveRecording = (state: {
  recordings: RecordingsState
}) => {
  const id = state.recordings.activeRecordingId
  return id ? (state.recordings.recordings[id] ?? null) : null
}

export const selectIsRecording = (state: { recordings: RecordingsState }) =>
  state.recordings.isRecording

export const selectViewMode = (state: { recordings: RecordingsState }) =>
  state.recordings.viewMode

export const selectSelectedRecordingId = (state: {
  recordings: RecordingsState
}) => state.recordings.selectedRecordingId

export const selectSelectedRecording = (state: {
  recordings: RecordingsState
}) => {
  const id = state.recordings.selectedRecordingId
  return id ? (state.recordings.recordings[id] ?? null) : null
}

export const selectPlayback = (state: { recordings: RecordingsState }) =>
  state.recordings.playback

export const selectCurrentSnapshot = (state: {
  recordings: RecordingsState
}) => {
  const { recordingId, currentIndex } = state.recordings.playback
  if (!recordingId) return null
  const recording = state.recordings.recordings[recordingId]
  return recording?.snapshots[currentIndex] ?? null
}

// =============================================================================
// Live Pipeline Selectors (Task 10.2)
// Feature: debug-recording-enhancement
// Implements: Requirements 2.1, 2.2, 2.3, 2.4
// =============================================================================

/** Select all live pipeline states */
export const selectLivePipelineStates = (state: {
  recordings: RecordingsState
}) => state.recordings.livePipeline.states

/** Select live pipeline state for a specific match */
export const selectLivePipelineState = (
  state: { recordings: RecordingsState },
  matchId: string,
) => state.recordings.livePipeline.states[matchId] ?? null

/** Select the currently subscribed match ID */
export const selectSubscribedMatchId = (state: {
  recordings: RecordingsState
}) => state.recordings.livePipeline.subscribedMatchId

/** Select WebSocket connection status */
export const selectPipelineConnectionStatus = (state: {
  recordings: RecordingsState
}) => state.recordings.livePipeline.connectionStatus

/** Select WebSocket connection error */
export const selectPipelineConnectionError = (state: {
  recordings: RecordingsState
}) => state.recordings.livePipeline.connectionError

/** Select live stages for a specific match */
export const selectLiveStages = (
  state: { recordings: RecordingsState },
  matchId: string,
) => state.recordings.livePipeline.states[matchId]?.stages ?? []

/** Select current stage name for a specific match */
export const selectCurrentLiveStage = (
  state: { recordings: RecordingsState },
  matchId: string,
) => state.recordings.livePipeline.states[matchId]?.currentStage ?? null

/** Select AI processing state for a specific match */
export const selectAiProcessingState = (
  state: { recordings: RecordingsState },
  matchId: string,
) =>
  state.recordings.livePipeline.states[matchId]?.aiProcessing ?? {
    active: false,
  }

/** Select whether a match pipeline is currently running */
export const selectIsPipelineRunning = (
  state: { recordings: RecordingsState },
  matchId: string,
) => state.recordings.livePipeline.states[matchId]?.status === 'running'

/** Select the subscribed pipeline state (convenience selector) */
export const selectSubscribedPipelineState = (state: {
  recordings: RecordingsState
}) => {
  const matchId = state.recordings.livePipeline.subscribedMatchId
  return matchId
    ? (state.recordings.livePipeline.states[matchId] ?? null)
    : null
}
