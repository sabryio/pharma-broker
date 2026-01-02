// Persistence Middleware
// LocalStorage persistence for user preferences and recordings

import type { Middleware } from '@reduxjs/toolkit'

const STORAGE_KEYS = {
  preferences: 'pharma-user-preferences',
  recordings: 'pharma-match-recordings',
  ui: 'pharma-ui-state',
} as const

// Debounce helper to avoid excessive writes
function debounce<T extends (...args: unknown[]) => void>(
  fn: T,
  delay: number,
): T {
  let timeoutId: ReturnType<typeof setTimeout> | null = null
  return ((...args: unknown[]) => {
    if (timeoutId) clearTimeout(timeoutId)
    timeoutId = setTimeout(() => fn(...args), delay)
  }) as T
}

// Debounced save functions
const savePreferences = debounce((preferences: unknown) => {
  try {
    localStorage.setItem(STORAGE_KEYS.preferences, JSON.stringify(preferences))
  } catch (e) {
    console.warn('Failed to persist preferences:', e)
  }
}, 500)

const saveRecordings = debounce((recordings: unknown) => {
  try {
    localStorage.setItem(STORAGE_KEYS.recordings, JSON.stringify(recordings))
  } catch (e) {
    console.warn('Failed to persist recordings:', e)
  }
}, 1000)

const saveUiState = debounce((ui: unknown) => {
  try {
    const uiState = ui as { sidebarCollapsed: boolean; theme: string }
    localStorage.setItem(
      STORAGE_KEYS.ui,
      JSON.stringify({
        sidebarCollapsed: uiState.sidebarCollapsed,
        theme: uiState.theme,
      }),
    )
  } catch (e) {
    console.warn('Failed to persist UI state:', e)
  }
}, 500)

// Define state shape interface locally to avoid circular imports
interface PersistedStateShape {
  session?: {
    preferences: unknown
  }
  recordings?: {
    recordings: unknown
  }
  ui?: {
    sidebarCollapsed: boolean
    theme: string
  }
}

export const persistenceMiddleware: Middleware<object, PersistedStateShape> =
  (store) => (next) => (action) => {
    const result = next(action)
    const state = store.getState()

    const actionType = (action as { type?: string }).type ?? ''

    // Persist preferences on session changes
    if (actionType.startsWith('session/') && state.session) {
      savePreferences(state.session.preferences)
    }

    // Persist recordings on recording changes
    if (actionType.startsWith('recordings/') && state.recordings) {
      saveRecordings(state.recordings.recordings)
    }

    // Persist UI state on UI changes
    if (actionType.startsWith('ui/') && state.ui) {
      saveUiState({
        sidebarCollapsed: state.ui.sidebarCollapsed,
        theme: state.ui.theme,
      })
    }

    return result
  }

// Hydration function for initial state - returns generic object
export function loadPersistedState(): Record<string, unknown> {
  const result: Record<string, unknown> = {}

  try {
    // Load preferences
    const preferencesJson = localStorage.getItem(STORAGE_KEYS.preferences)
    if (preferencesJson) {
      const preferences = JSON.parse(preferencesJson)
      result.session = {
        userId: '00000000-0000-0000-0000-000000000001',
        userName: null,
        isAuthenticated: false,
        preferences,
      }
    }
  } catch (e) {
    console.warn('Failed to load persisted preferences:', e)
  }

  try {
    // Load recordings
    const recordingsJson = localStorage.getItem(STORAGE_KEYS.recordings)
    if (recordingsJson) {
      const recordings = JSON.parse(recordingsJson)
      // Convert date strings back to Date objects
      for (const key of Object.keys(recordings)) {
        const recording = recordings[key]
        recording.startedAt = new Date(recording.startedAt)
        if (recording.endedAt) {
          recording.endedAt = new Date(recording.endedAt)
        }
        recording.snapshots = recording.snapshots.map(
          (s: { timestamp: string | Date }) => ({
            ...s,
            timestamp: new Date(s.timestamp),
          }),
        )
      }
      result.recordings = {
        recordings,
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
    }
  } catch (e) {
    console.warn('Failed to load persisted recordings:', e)
  }

  try {
    // Load UI state
    const uiJson = localStorage.getItem(STORAGE_KEYS.ui)
    if (uiJson) {
      const ui = JSON.parse(uiJson)
      result.ui = {
        sidebarCollapsed: ui.sidebarCollapsed ?? false,
        activeModal: null,
        theme: ui.theme ?? 'system',
        toastQueue: [],
      }
    }
  } catch (e) {
    console.warn('Failed to load persisted UI state:', e)
  }

  return result
}

// Clear all persisted state
export function clearPersistedState(): void {
  Object.values(STORAGE_KEYS).forEach((key) => {
    localStorage.removeItem(key)
  })
}
