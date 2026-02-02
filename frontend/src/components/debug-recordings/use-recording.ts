// Recording Hook
// Manages recording state and snapshot creation with enhanced features

import { useCallback, useRef, useState } from 'react'
import type { MatchReviewItem } from '@/schema/match-review'
import type {
  AdjustmentSettings,
  MatchRecording,
  MatchRecordingSnapshot,
  RecordingEvent,
  RecordingMetadata,
  ScoreBreakdown,
  WeightConfig,
} from './types'

const generateId = () =>
  `${Date.now()}-${Math.random().toString(36).slice(2, 9)}`

const getSessionId = () => {
  let sessionId = sessionStorage.getItem('recording-session-id')
  if (!sessionId) {
    sessionId = generateId()
    sessionStorage.setItem('recording-session-id', sessionId)
  }
  return sessionId
}

export interface UseRecordingOptions {
  autoStart?: boolean
  maxSnapshots?: number
  persist?: boolean
  maxRecordings?: number
}

export interface UseRecordingReturn {
  recordings: Map<string, MatchRecording>
  recordingsArray: MatchRecording[]
  activeRecording: MatchRecording | null
  isRecording: boolean
  recordingCount: number
  startRecording: (matchId: string) => void
  stopRecording: () => void
  toggleRecording: () => void
  takeSnapshot: (
    matchReview: MatchReviewItem,
    event: RecordingEvent,
    adjustments: AdjustmentSettings,
    scoreBreakdown?: ScoreBreakdown,
    weights?: WeightConfig,
  ) => void
  getRecording: (matchId: string) => MatchRecording | undefined
  deleteRecording: (matchId: string) => void
  clearRecordings: () => void
  exportRecording: (matchId: string) => string | null
  exportAllRecordings: () => string
  importRecording: (json: string) => boolean
  importRecordings: (json: string) => number
}

const STORAGE_KEY = 'pharma-match-recordings'

function loadPersistedRecordings(): Map<string, MatchRecording> {
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    if (stored) {
      const parsed = JSON.parse(stored)
      const map = new Map<string, MatchRecording>()
      for (const [key, value] of Object.entries(parsed)) {
        const recording = value as MatchRecording
        recording.startedAt = new Date(recording.startedAt)
        if (recording.endedAt) recording.endedAt = new Date(recording.endedAt)
        recording.snapshots = recording.snapshots.map((s) => ({
          ...s,
          timestamp: new Date(s.timestamp),
        }))
        map.set(key, recording)
      }
      return map
    }
  } catch (e) {
    console.warn('Failed to load persisted recordings:', e)
  }
  return new Map()
}

function persistRecordings(recordings: Map<string, MatchRecording>) {
  try {
    const obj: Record<string, MatchRecording> = {}
    recordings.forEach((v, k) => {
      obj[k] = v
    })
    localStorage.setItem(STORAGE_KEY, JSON.stringify(obj))
  } catch (e) {
    console.warn('Failed to persist recordings:', e)
  }
}

export function useRecording(
  options: UseRecordingOptions = {},
): UseRecordingReturn {
  const {
    autoStart = true,
    maxSnapshots = 100,
    persist = true,
    maxRecordings = 50,
  } = options

  const [recordings, setRecordings] = useState<Map<string, MatchRecording>>(
    () => (persist ? loadPersistedRecordings() : new Map()),
  )
  const [activeRecordingId, setActiveRecordingId] = useState<string | null>(
    null,
  )
  const [isRecording, setIsRecording] = useState(autoStart)
  const lastSnapshotIdRef = useRef<string | null>(null)

  const activeRecording = activeRecordingId
    ? (recordings.get(activeRecordingId) ?? null)
    : null

  const recordingsArray = Array.from(recordings.values()).sort(
    (a, b) => b.startedAt.getTime() - a.startedAt.getTime(),
  )

  const startRecording = useCallback(
    (matchId: string) => {
      if (!isRecording) return

      setRecordings((prev) => {
        const next = new Map(prev)
        if (!next.has(matchId)) {
          // Enforce max recordings limit
          if (next.size >= maxRecordings) {
            const oldest = Array.from(next.entries()).sort(
              ([, a], [, b]) => a.startedAt.getTime() - b.startedAt.getTime(),
            )[0]
            next.delete(oldest[0])
          }
          next.set(matchId, {
            id: matchId,
            matchId,
            startedAt: new Date(),
            snapshots: [],
          })
        }
        if (persist) persistRecordings(next)
        return next
      })
      setActiveRecordingId(matchId)
      lastSnapshotIdRef.current = null
    },
    [isRecording, persist, maxRecordings],
  )

  const stopRecording = useCallback(() => {
    if (!activeRecordingId) return

    setRecordings((prev) => {
      const next = new Map(prev)
      const recording = next.get(activeRecordingId)
      if (recording) {
        recording.endedAt = new Date()
        recording.duration =
          recording.endedAt.getTime() - recording.startedAt.getTime()
        const lastSnapshot = recording.snapshots[recording.snapshots.length - 1]
        if (lastSnapshot) {
          if (lastSnapshot.event.type === 'approve')
            recording.outcome = 'approved'
          else if (lastSnapshot.event.type === 'reject')
            recording.outcome = 'rejected'
          else recording.outcome = 'pending'
        }
      }
      if (persist) persistRecordings(next)
      return next
    })
    setActiveRecordingId(null)
  }, [activeRecordingId, persist])

  const toggleRecording = useCallback(() => {
    setIsRecording((prev) => !prev)
    if (isRecording && activeRecordingId) {
      stopRecording()
    }
  }, [isRecording, activeRecordingId, stopRecording])

  const takeSnapshot = useCallback(
    (
      matchReview: MatchReviewItem,
      event: RecordingEvent,
      adjustments: AdjustmentSettings,
      scoreBreakdown?: ScoreBreakdown,
      weights?: WeightConfig,
    ) => {
      if (!isRecording) return

      const matchId = matchReview.id

      setRecordings((prev) => {
        const next = new Map(prev)
        let recording = next.get(matchId)

        if (!recording) {
          recording = {
            id: matchId,
            matchId,
            startedAt: new Date(),
            snapshots: [],
          }
          next.set(matchId, recording)
          setActiveRecordingId(matchId)
        }

        if (recording.snapshots.length >= maxSnapshots) {
          recording.snapshots.shift()
        }

        const snapshotId = generateId()
        const metadata: RecordingMetadata = {
          userAgent:
            typeof navigator !== 'undefined' ? navigator.userAgent : undefined,
          sessionId: getSessionId(),
          previousSnapshotId: lastSnapshotIdRef.current,
          scoreBreakdown,
          weights,
        }

        const snapshot: MatchRecordingSnapshot = {
          id: snapshotId,
          timestamp: new Date(),
          matchReview,
          offer: matchReview.offer,
          request: matchReview.request,
          confidence: matchReview.confidence,
          aiConfidence: matchReview.aiConfidence ?? null,
          issues: matchReview.issues,
          reasoning: matchReview.reasoning,
          adjustments: { ...adjustments },
          event,
          metadata,
        }

        recording.snapshots.push(snapshot)
        lastSnapshotIdRef.current = snapshotId

        if (persist) persistRecordings(next)
        return next
      })
    },
    [isRecording, maxSnapshots, persist],
  )

  const getRecording = useCallback(
    (matchId: string) => recordings.get(matchId),
    [recordings],
  )

  const deleteRecording = useCallback(
    (matchId: string) => {
      setRecordings((prev) => {
        const next = new Map(prev)
        next.delete(matchId)
        if (persist) persistRecordings(next)
        return next
      })
      if (activeRecordingId === matchId) {
        setActiveRecordingId(null)
      }
    },
    [persist, activeRecordingId],
  )

  const clearRecordings = useCallback(() => {
    setRecordings(new Map())
    setActiveRecordingId(null)
    lastSnapshotIdRef.current = null
    if (persist) localStorage.removeItem(STORAGE_KEY)
  }, [persist])

  const exportRecording = useCallback(
    (matchId: string): string | null => {
      const recording = recordings.get(matchId)
      if (!recording) return null
      return JSON.stringify(recording, null, 2)
    },
    [recordings],
  )

  const exportAllRecordings = useCallback((): string => {
    const obj: Record<string, MatchRecording> = {}
    recordings.forEach((v, k) => {
      obj[k] = v
    })
    return JSON.stringify(obj, null, 2)
  }, [recordings])

  const importRecording = useCallback(
    (json: string): boolean => {
      try {
        const recording = JSON.parse(json) as MatchRecording
        recording.startedAt = new Date(recording.startedAt)
        if (recording.endedAt) recording.endedAt = new Date(recording.endedAt)
        recording.snapshots = recording.snapshots.map((s) => ({
          ...s,
          timestamp: new Date(s.timestamp),
        }))

        setRecordings((prev) => {
          const next = new Map(prev)
          next.set(recording.matchId, recording)
          if (persist) persistRecordings(next)
          return next
        })
        return true
      } catch (e) {
        console.error('Failed to import recording:', e)
        return false
      }
    },
    [persist],
  )

  const importRecordings = useCallback(
    (json: string): number => {
      try {
        const data = JSON.parse(json) as Record<string, MatchRecording>
        let count = 0

        setRecordings((prev) => {
          const next = new Map(prev)
          for (const [key, value] of Object.entries(data)) {
            const recording = value as MatchRecording
            recording.startedAt = new Date(recording.startedAt)
            if (recording.endedAt)
              recording.endedAt = new Date(recording.endedAt)
            recording.snapshots = recording.snapshots.map((s) => ({
              ...s,
              timestamp: new Date(s.timestamp),
            }))
            next.set(key, recording)
            count++
          }
          if (persist) persistRecordings(next)
          return next
        })
        return count
      } catch (e) {
        console.error('Failed to import recordings:', e)
        return 0
      }
    },
    [persist],
  )

  return {
    recordings,
    recordingsArray,
    activeRecording,
    isRecording,
    recordingCount: recordings.size,
    startRecording,
    stopRecording,
    toggleRecording,
    takeSnapshot,
    getRecording,
    deleteRecording,
    clearRecordings,
    exportRecording,
    exportAllRecordings,
    importRecording,
    importRecordings,
  }
}
