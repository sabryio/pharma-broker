// Root Reducer
// Combines all slices into a single reducer

import { combineReducers } from '@reduxjs/toolkit'
import uiReducer from './slices/uiSlice'
import filtersReducer from './slices/filtersSlice'
import sessionReducer from './slices/sessionSlice'
import reviewQueueReducer from './slices/reviewQueueSlice'
import matchReviewsReducer from './slices/matchReviewsSlice'
import recordingsReducer from './slices/recordingsSlice'

export const rootReducer = combineReducers({
  ui: uiReducer,
  filters: filtersReducer,
  session: sessionReducer,
  reviewQueue: reviewQueueReducer,
  matchReviews: matchReviewsReducer,
  recordings: recordingsReducer,
})
