import {
  useQuery,
  useMutation,
  useQueryClient,
  keepPreviousData,
} from '@tanstack/react-query'
import {
  getMatchReviews,
  getMatchReview,
  updateMatchReviewStatus,
  bulkUpdateMatchReviews,
  getMatchReviewStats,
} from '@/api/match-reviews'
import type {
  MatchReviewParams,
  BulkUpdateRequest,
  MatchReviewItem,
  MatchReviewStats,
} from '@/schema/match-review'
import { useAppSelector, useMatchReviewsActions } from '@/store/hooks'
import { selectUserId } from '@/store/slices/sessionSlice'
import { selectMatchReviewsFilters } from '@/store/slices/filtersSlice'
import { queryKeys } from './query-keys'

/**
 * Hook to fetch paginated match reviews with Redux filter integration
 */
export function useMatchReviews(params: MatchReviewParams = {}) {
  const filters = useAppSelector(selectMatchReviewsFilters)
  const pageSize = useAppSelector(
    (state) => state.session.preferences.defaultPageSize,
  )

  const mergedParams = {
    limit: params.limit ?? pageSize,
    offset: params.offset ?? 0,
    status:
      params.status ?? (filters.status === 'all' ? undefined : filters.status),
    minScore:
      params.minScore ??
      (filters.minConfidence > 0 ? filters.minConfidence : undefined),
  }

  return useQuery({
    queryKey: queryKeys.matchReviews.list(mergedParams),
    queryFn: () => getMatchReviews(mergedParams),
    placeholderData: keepPreviousData,
    staleTime: 10 * 1000,
  })
}

/**
 * Hook to fetch match reviews without Redux filter integration (for manual control)
 */
export function useMatchReviewsManual(params: MatchReviewParams = {}) {
  const { limit = 20, offset = 0 } = params

  return useQuery({
    queryKey: queryKeys.matchReviews.list({ limit, offset }),
    queryFn: () => getMatchReviews({ limit, offset }),
    placeholderData: keepPreviousData,
    staleTime: 10 * 1000,
  })
}

/**
 * Hook to fetch a single match review
 */
export function useMatchReview(id: string | undefined) {
  return useQuery({
    queryKey: queryKeys.matchReviews.detail(id ?? ''),
    queryFn: () => getMatchReview(id!),
    enabled: !!id,
    staleTime: 10 * 1000,
  })
}

/**
 * Hook to fetch match review statistics
 */
export function useMatchReviewStats() {
  const autoRefreshInterval = useAppSelector(
    (state) => state.session.preferences.autoRefreshInterval,
  )

  return useQuery({
    queryKey: queryKeys.matchReviews.stats(),
    queryFn: getMatchReviewStats,
    staleTime: 30 * 1000,
    refetchInterval: autoRefreshInterval > 0 ? autoRefreshInterval : false,
  })
}

/**
 * Hook to update match review status with optimistic updates and Redux integration
 */
export function useUpdateMatchReviewStatus() {
  const queryClient = useQueryClient()
  const matchReviewsActions = useMatchReviewsActions()
  const userId = useAppSelector(selectUserId)

  return useMutation({
    mutationFn: ({
      id,
      action,
      notes,
    }: {
      id: string
      action: 'approved' | 'rejected'
      notes?: string
    }) => {
      return updateMatchReviewStatus(id, {
        action,
        reviewed_by: userId,
        reasoning: notes,
      })
    },

    onMutate: async (variables) => {
      await queryClient.cancelQueries({ queryKey: queryKeys.matchReviews.all })

      const previousItems = queryClient.getQueryData(
        queryKeys.matchReviews.lists(),
      )
      const previousStats = queryClient.getQueryData(
        queryKeys.matchReviews.stats(),
      )

      // Optimistically remove item from list
      queryClient.setQueriesData(
        { queryKey: queryKeys.matchReviews.lists() },
        (old: { items: MatchReviewItem[]; total: number } | undefined) => {
          if (!old) return old
          return {
            ...old,
            items: old.items.filter((item) => item.id !== variables.id),
            total: Math.max(0, old.total - 1),
          }
        },
      )

      // Optimistically update stats
      queryClient.setQueryData(
        queryKeys.matchReviews.stats(),
        (old: MatchReviewStats | undefined) => {
          if (!old) return old
          return {
            ...old,
            pending: Math.max(0, old.pending - 1),
            totalPending: Math.max(0, old.totalPending - 1),
            confirmedToday:
              variables.action === 'approved'
                ? old.confirmedToday + 1
                : old.confirmedToday,
            rejectedToday:
              variables.action === 'rejected'
                ? old.rejectedToday + 1
                : old.rejectedToday,
          }
        },
      )

      return { previousItems, previousStats }
    },

    onSuccess: (_, variables) => {
      // Record action in Redux for undo/history
      matchReviewsActions.recordAction({
        type: variables.action,
        matchId: variables.id,
      })
    },

    onError: (_err, _variables, context) => {
      if (context?.previousItems) {
        queryClient.setQueriesData(
          { queryKey: queryKeys.matchReviews.lists() },
          context.previousItems,
        )
      }
      if (context?.previousStats) {
        queryClient.setQueryData(
          queryKeys.matchReviews.stats(),
          context.previousStats,
        )
      }
    },

    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.matchReviews.all })
    },
  })
}

/**
 * Hook to bulk update match reviews with Redux integration
 */
export function useBulkUpdateMatchReviews() {
  const queryClient = useQueryClient()
  const userId = useAppSelector(selectUserId)
  const matchReviewsActions = useMatchReviewsActions()
  return useMutation({
    mutationFn: (
      data: Omit<BulkUpdateRequest, 'reviewed_by'> & { reviewed_by?: string },
    ) =>
      bulkUpdateMatchReviews({
        ...data,
        reviewed_by: data.reviewed_by ?? userId,
      }),

    onMutate: async (variables) => {
      await queryClient.cancelQueries({ queryKey: queryKeys.matchReviews.all })

      const previousItems = queryClient.getQueryData(
        queryKeys.matchReviews.lists(),
      )

      // Optimistically remove items from list
      queryClient.setQueriesData(
        { queryKey: queryKeys.matchReviews.lists() },
        (old: { items: MatchReviewItem[]; total: number } | undefined) => {
          if (!old) return old
          const idsSet = new Set(variables.ids)
          return {
            ...old,
            items: old.items.filter((item) => !idsSet.has(item.id)),
            total: Math.max(0, old.total - variables.ids.length),
          }
        },
      )

      // Optimistically update stats
      queryClient.setQueryData(
        queryKeys.matchReviews.stats(),
        (old: MatchReviewStats | undefined) => {
          if (!old) return old
          return {
            ...old,
            pending: Math.max(0, old.pending - variables.ids.length),
            totalPending: Math.max(0, old.totalPending - variables.ids.length),
            confirmedToday:
              variables.action === 'approved'
                ? old.confirmedToday + variables.ids.length
                : old.confirmedToday,
            rejectedToday:
              variables.action === 'rejected'
                ? old.rejectedToday + variables.ids.length
                : old.rejectedToday,
          }
        },
      )

      return { previousItems }
    },

    onSuccess: (_, variables) => {
      // Record actions in Redux
      for (const id of variables.ids) {
        matchReviewsActions.recordAction({
          type: variables.action,
          matchId: id,
        })
      }
      // Clear bulk selection after successful operation
      matchReviewsActions.clearBulkSelection()
    },

    onError: (_err, _variables, context) => {
      if (context?.previousItems) {
        queryClient.setQueriesData(
          { queryKey: queryKeys.matchReviews.lists() },
          context.previousItems,
        )
      }
    },

    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.matchReviews.all })
    },
  })
}

/**
 * Hook to bulk update using current Redux selection
 */
export function useBulkUpdateFromSelection() {
  const bulkSelectionIds = useAppSelector(
    (state) => state.matchReviews.bulkSelectionIds,
  )
  const bulkUpdate = useBulkUpdateMatchReviews()

  return {
    ...bulkUpdate,
    selectedCount: bulkSelectionIds.length,
    mutate: (action: 'approved' | 'rejected') => {
      if (bulkSelectionIds.length === 0) return
      bulkUpdate.mutate({ ids: bulkSelectionIds, action })
    },
    mutateAsync: async (action: 'approved' | 'rejected') => {
      if (bulkSelectionIds.length === 0) return
      return bulkUpdate.mutateAsync({ ids: bulkSelectionIds, action })
    },
  }
}

/**
 * Hook to prefetch match reviews
 */
export function usePrefetchMatchReviews() {
  const queryClient = useQueryClient()

  return (params: MatchReviewParams) => {
    queryClient.prefetchQuery({
      queryKey: queryKeys.matchReviews.list(params),
      queryFn: () => getMatchReviews(params),
      staleTime: 10 * 1000,
    })
  }
}
