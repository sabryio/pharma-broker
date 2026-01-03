// useMatchAction Hook
// Encapsulates match action logic with debouncing, processing state, and undo support

import { useState, useCallback, useRef, useEffect } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { updateMatchReviewStatus, undoMatchAction } from '@/api/match-reviews'
import { useAppSelector, useMatchReviewsActions } from '@/store/hooks'
import { selectUserId } from '@/store/slices/sessionSlice'
import { queryKeys } from './query-keys'
import type { MatchReviewItem, MatchReviewStats } from '@/schema/match-review'

const DEFAULT_UNDO_WINDOW_MS = 8000
const DEBOUNCE_MS = 300

export interface UndoState {
  matchId: string
  previousAction: 'approved' | 'rejected'
  timestamp: number
  expiresAt: number
  productName: string
}

export interface UseMatchActionOptions {
  onSuccess?: (matchId: string, action: 'approved' | 'rejected') => void
  onError?: (error: Error) => void
  undoWindowMs?: number
}

export interface UseMatchActionReturn {
  executeAction: (
    matchId: string,
    action: 'approved' | 'rejected',
    notes?: string,
  ) => Promise<void>
  undoAction: (matchId: string) => Promise<void>
  isProcessing: (matchId: string) => boolean
  canUndo: (matchId: string) => boolean
  pendingUndos: Map<string, UndoState>
  getUndoState: (matchId: string) => UndoState | undefined
}

/**
 * Hook for executing match approve/reject actions with:
 * - Debouncing to prevent rapid duplicate clicks
 * - Per-match processing state tracking
 * - Undo capability within a configurable time window
 * - Optimistic updates with rollback on error
 */
export function useMatchAction(
  options: UseMatchActionOptions = {},
): UseMatchActionReturn {
  const { onSuccess, onError, undoWindowMs = DEFAULT_UNDO_WINDOW_MS } = options

  const queryClient = useQueryClient()
  const matchReviewsActions = useMatchReviewsActions()
  const userId = useAppSelector(selectUserId)

  // Track which matches are currently being processed
  const [processingIds, setProcessingIds] = useState<Set<string>>(new Set())

  // Track undoable actions with expiration
  const [pendingUndos, setPendingUndos] = useState<Map<string, UndoState>>(
    new Map(),
  )

  // Track last action time per match for debouncing
  const lastActionTimeRef = useRef<Map<string, number>>(new Map())

  // Track undo expiration timers
  const undoTimersRef = useRef<Map<string, NodeJS.Timeout>>(new Map())

  // Cleanup timers on unmount
  useEffect(() => {
    return () => {
      undoTimersRef.current.forEach((timer) => clearTimeout(timer))
      undoTimersRef.current.clear()
    }
  }, [])

  // Clear expired undo states periodically
  useEffect(() => {
    const interval = setInterval(() => {
      const now = Date.now()
      setPendingUndos((prev) => {
        const updated = new Map(prev)
        let changed = false
        for (const [matchId, state] of updated) {
          if (state.expiresAt <= now) {
            updated.delete(matchId)
            changed = true
          }
        }
        return changed ? updated : prev
      })
    }, 1000)

    return () => clearInterval(interval)
  }, [])

  const isProcessing = useCallback(
    (matchId: string) => processingIds.has(matchId),
    [processingIds],
  )

  const canUndo = useCallback(
    (matchId: string) => {
      const undoState = pendingUndos.get(matchId)
      if (!undoState) return false
      return Date.now() < undoState.expiresAt
    },
    [pendingUndos],
  )

  const getUndoState = useCallback(
    (matchId: string) => pendingUndos.get(matchId),
    [pendingUndos],
  )

  const executeAction = useCallback(
    async (
      matchId: string,
      action: 'approved' | 'rejected',
      notes?: string,
    ): Promise<void> => {
      // Debounce: ignore if action was triggered too recently
      const now = Date.now()
      const lastTime = lastActionTimeRef.current.get(matchId) ?? 0
      if (now - lastTime < DEBOUNCE_MS) {
        return
      }
      lastActionTimeRef.current.set(matchId, now)

      // Ignore if already processing this match
      if (processingIds.has(matchId)) {
        return
      }

      // Mark as processing
      setProcessingIds((prev) => new Set(prev).add(matchId))

      // Cancel any existing undo timer for this match
      const existingTimer = undoTimersRef.current.get(matchId)
      if (existingTimer) {
        clearTimeout(existingTimer)
        undoTimersRef.current.delete(matchId)
      }

      // Remove from pending undos if exists
      setPendingUndos((prev) => {
        if (prev.has(matchId)) {
          const updated = new Map(prev)
          updated.delete(matchId)
          return updated
        }
        return prev
      })

      // Store previous data for optimistic update rollback
      const previousItems = queryClient.getQueryData(
        queryKeys.matchReviews.lists(),
      )
      const previousStats = queryClient.getQueryData<MatchReviewStats>(
        queryKeys.matchReviews.stats(),
      )

      // Get product name for undo toast before optimistic update removes it
      let productName = 'Unknown Product'
      queryClient
        .getQueriesData<{ items: MatchReviewItem[] }>({
          queryKey: queryKeys.matchReviews.lists(),
        })
        .forEach(([, data]) => {
          const match = data?.items?.find((item) => item.id === matchId)
          if (match) {
            productName = match.offer.product
          }
        })

      // Optimistic update: remove from list
      await queryClient.cancelQueries({ queryKey: queryKeys.matchReviews.all })

      queryClient.setQueriesData<{ items: MatchReviewItem[]; total: number }>(
        { queryKey: queryKeys.matchReviews.lists() },
        (old) => {
          if (!old) return old
          return {
            ...old,
            items: old.items.filter((item) => item.id !== matchId),
            total: Math.max(0, old.total - 1),
          }
        },
      )

      // Optimistic update: update stats
      queryClient.setQueryData<MatchReviewStats>(
        queryKeys.matchReviews.stats(),
        (old) => {
          if (!old) return old
          return {
            ...old,
            pending: Math.max(0, old.pending - 1),
            totalPending: Math.max(0, old.totalPending - 1),
            confirmedToday:
              action === 'approved'
                ? old.confirmedToday + 1
                : old.confirmedToday,
            rejectedToday:
              action === 'rejected' ? old.rejectedToday + 1 : old.rejectedToday,
          }
        },
      )

      try {
        await updateMatchReviewStatus(matchId, {
          action,
          reviewed_by: userId,
          notes,
        })

        // Record action in Redux
        matchReviewsActions.recordAction({
          type: action,
          matchId,
        })

        // Set up undo state
        const timestamp = Date.now()
        const expiresAt = timestamp + undoWindowMs

        setPendingUndos((prev) => {
          const updated = new Map(prev)
          updated.set(matchId, {
            matchId,
            previousAction: action,
            timestamp,
            expiresAt,
            productName,
          })
          return updated
        })

        // Set up timer to clear undo state
        const timer = setTimeout(() => {
          setPendingUndos((prev) => {
            const updated = new Map(prev)
            updated.delete(matchId)
            return updated
          })
          undoTimersRef.current.delete(matchId)
        }, undoWindowMs)

        undoTimersRef.current.set(matchId, timer)

        onSuccess?.(matchId, action)
      } catch (error) {
        // Rollback optimistic updates
        if (previousItems) {
          queryClient.setQueriesData(
            { queryKey: queryKeys.matchReviews.lists() },
            previousItems,
          )
        }
        if (previousStats) {
          queryClient.setQueryData(
            queryKeys.matchReviews.stats(),
            previousStats,
          )
        }

        onError?.(error instanceof Error ? error : new Error(String(error)))
      } finally {
        // Clear processing state
        setProcessingIds((prev) => {
          const updated = new Set(prev)
          updated.delete(matchId)
          return updated
        })

        // Invalidate queries to ensure fresh data
        queryClient.invalidateQueries({ queryKey: queryKeys.matchReviews.all })
      }
    },
    [
      processingIds,
      queryClient,
      userId,
      matchReviewsActions,
      undoWindowMs,
      onSuccess,
      onError,
    ],
  )

  const undoAction = useCallback(
    async (matchId: string): Promise<void> => {
      const undoState = pendingUndos.get(matchId)
      if (!undoState || Date.now() >= undoState.expiresAt) {
        onError?.(new Error('Undo window expired'))
        return
      }

      // Clear the undo timer
      const timer = undoTimersRef.current.get(matchId)
      if (timer) {
        clearTimeout(timer)
        undoTimersRef.current.delete(matchId)
      }

      // Remove from pending undos immediately
      setPendingUndos((prev) => {
        const updated = new Map(prev)
        updated.delete(matchId)
        return updated
      })

      // Mark as processing
      setProcessingIds((prev) => new Set(prev).add(matchId))

      try {
        // Call the undo API endpoint
        await undoMatchAction(matchId, {
          userId,
          originalAction: undoState.previousAction,
        })

        // Invalidate queries to refresh data
        await queryClient.invalidateQueries({
          queryKey: queryKeys.matchReviews.all,
        })

        onSuccess?.(matchId, undoState.previousAction)
      } catch (error) {
        onError?.(error instanceof Error ? error : new Error(String(error)))
      } finally {
        setProcessingIds((prev) => {
          const updated = new Set(prev)
          updated.delete(matchId)
          return updated
        })
      }
    },
    [pendingUndos, queryClient, userId, onSuccess, onError],
  )

  return {
    executeAction,
    undoAction,
    isProcessing,
    canUndo,
    pendingUndos,
    getUndoState,
  }
}
