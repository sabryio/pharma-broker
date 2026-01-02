// Slices Barrel Export
// Re-export all slices for convenient imports

export * from './uiSlice'
export * from './filtersSlice'
export * from './sessionSlice'
export * from './reviewQueueSlice'
export * from './matchReviewsSlice'
export {
  // Selectors (renamed to avoid conflicts)
  selectRecordingsMap,
  selectRecordingsArray,
  selectRecordingCount,
  selectActiveRecording,
  selectIsRecording,
  selectSelectedRecordingId,
  selectSelectedRecording,
  selectPlayback,
  selectCurrentSnapshot,
  selectViewMode as selectRecordingsViewMode,
  // Reducer
  default as recordingsReducer,
} from './recordingsSlice'
