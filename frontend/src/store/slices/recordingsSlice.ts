// Recordings Slice
// State management for debug recordings

import { createSlice, type PayloadAction } from '@reduxjs/toolkit'
import type {
  MatchRecording,
  MatchRecordingSnapshot,
} from '@/components/debug-recordings/types'

type ViewMode = 'overview' | 'recordings' | 'pipeline' | 'analytics'

interface PlaybackState {
  recordingId: string | null
  currentIndex: number
  isPlaying: boolean
  speed: number
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
        recording.endedAt = new Date()
        recording.duration =
          new Date(recording.endedAt).getTime() -
          new Date(recording.startedAt).getTime()
        // Determine outcome from last snapshot
        const lastSnapshot = recording.snapshots[recording.snapshots.length - 1]
        if (lastSnapshot) {
          recording.outcome =
            lastSnapshot.event.type === 'approve'
              ? 'approved'
              : lastSnapshot.event.type === 'reject'
                ? 'rejected'
                : 'pending'
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
  },
})

export const recordingsActions = recordingsSlice.actions

export default recordingsSlice.reducer

// Selectors
export const selectRecordingsMap = (state: { recordings: RecordingsState }) =>
  state.recordings.recordings

export const selectRecordingsArray = (state: { recordings: RecordingsState }) =>
  Object.values(state.recordings.recordings).sort(
    (a, b) => new Date(b.startedAt).getTime() - new Date(a.startedAt).getTime(),
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
