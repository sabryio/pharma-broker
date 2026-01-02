// Typed Redux Hooks
// Pre-typed hooks for use throughout the application

import { useMemo } from 'react'
import { useDispatch, useSelector } from 'react-redux'
import { bindActionCreators } from '@reduxjs/toolkit'
import type { RootState, AppDispatch } from './index'

import {
  filterActions,
  matchReviewsActions,
  reviewQueueActions,
  sessionActions,
  uiActions,
} from './slices'
import { recordingsActions } from './slices/recordingsSlice'

// Use these hooks instead of plain `useDispatch` and `useSelector`
export const useAppDispatch = useDispatch.withTypes<AppDispatch>()
export const useAppSelector = useSelector.withTypes<RootState>()

// ============================================================================
// Bound Action Hooks - No need to import individual actions or dispatch
// ============================================================================

/** All actions bound to dispatch */
export const useActions = () => {
  const dispatch = useAppDispatch()
  return useMemo(
    () => ({
      ui: bindActionCreators(uiActions, dispatch),
      filters: bindActionCreators(filterActions, dispatch),
      session: bindActionCreators(sessionActions, dispatch),
      reviewQueue: bindActionCreators(reviewQueueActions, dispatch),
      matchReviews: bindActionCreators(matchReviewsActions, dispatch),
      recordings: bindActionCreators(recordingsActions, dispatch),
    }),
    [dispatch],
  )
}

/** UI actions only */
export const useUiActions = () => {
  const dispatch = useAppDispatch()
  return useMemo(() => bindActionCreators(uiActions, dispatch), [dispatch])
}

/** Filter actions only */
export const useFilterActions = () => {
  const dispatch = useAppDispatch()
  return useMemo(() => bindActionCreators(filterActions, dispatch), [dispatch])
}

/** Session actions only */
export const useSessionActions = () => {
  const dispatch = useAppDispatch()
  return useMemo(() => bindActionCreators(sessionActions, dispatch), [dispatch])
}

/** Review queue actions only */
export const useReviewQueueActions = () => {
  const dispatch = useAppDispatch()
  return useMemo(
    () => bindActionCreators(reviewQueueActions, dispatch),
    [dispatch],
  )
}

/** Match reviews actions only */
export const useMatchReviewsActions = () => {
  const dispatch = useAppDispatch()
  return useMemo(
    () => bindActionCreators(matchReviewsActions, dispatch),
    [dispatch],
  )
}

/** Recordings actions only */
export const useRecordingsActions = () => {
  const dispatch = useAppDispatch()
  return useMemo(
    () => bindActionCreators(recordingsActions, dispatch),
    [dispatch],
  )
}
