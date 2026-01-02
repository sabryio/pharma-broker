// Redux Store Configuration
// Centralized store setup with TypeScript support

import { configureStore } from '@reduxjs/toolkit'
import { rootReducer } from './rootReducer'
import {
  persistenceMiddleware,
  loadPersistedState,
} from './middleware/persistence'

// Infer RootState from rootReducer
export type RootState = ReturnType<typeof rootReducer>

// Load persisted state for hydration
const preloadedState = loadPersistedState()

export const store = configureStore({
  reducer: rootReducer,
  preloadedState,
  middleware: (getDefaultMiddleware) =>
    getDefaultMiddleware({
      serializableCheck: {
        // Ignore Date objects in recordings
        ignoredPaths: ['recordings.recordings'],
        ignoredActions: ['recordings/addSnapshot', 'recordings/startRecording'],
      },
    }).concat(persistenceMiddleware),
  devTools: import.meta.env.DEV,
})

export type AppDispatch = typeof store.dispatch

// Re-export hooks and actions for convenience
export {
  useAppDispatch,
  useAppSelector,
  useActions,
  useUiActions,
  useFilterActions,
  useSessionActions,
  useReviewQueueActions,
  useMatchReviewsActions,
  useRecordingsActions,
} from './hooks'
