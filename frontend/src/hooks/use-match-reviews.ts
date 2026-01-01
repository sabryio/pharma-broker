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
  transformToReviewWithMapping,
} from '@/api/match-reviews'
import type {
  MatchReviewParams,
  BulkUpdateRequest,
} from '@/schema/match-review'

/**
 * Hook to fetch paginated match reviews
 */
export function useMatchReviews(params: MatchReviewParams = {}) {
  const { limit = 20, offset = 0 } = params

  return useQuery({
    queryKey: ['match-reviews', 'items', { limit, offset }],
    queryFn: () => getMatchReviews({ limit, offset }),
    placeholderData: keepPreviousData,
    staleTime: 10 * 1000,
    select: (data) => ({
      ...data,
      items: data.items.map(transformToReviewWithMapping),
    }),
  })
}

/**
 * Hook to fetch a single match review
 */
export function useMatchReview(id: string | undefined) {
  return useQuery({
    queryKey: ['match-reviews', 'item', id],
    queryFn: () => getMatchReview(id!),
    enabled: !!id,
    staleTime: 10 * 1000,
    select: transformToReviewWithMapping,
  })
}

/**
 * Hook to fetch match review statistics
 */
export function useMatchReviewStats() {
  return useQuery({
    queryKey: ['match-reviews', 'stats'],
    queryFn: getMatchReviewStats,
    staleTime: 30 * 1000,
    refetchInterval: 30 * 1000,
  })
}

/**
 * Hook to update match review status with optimistic updates
 */
export function useUpdateMatchReviewStatus() {
  const queryClient = useQueryClient()

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
        reviewed_by: 'current-user', // TODO: Get from auth context
        notes,
      })
    },

    onMutate: async (variables) => {
      await queryClient.cancelQueries({ queryKey: ['match-reviews'] })

      const previousItems = queryClient.getQueryData(['match-reviews', 'items'])
      const previousStats = queryClient.getQueryData(['match-reviews', 'stats'])

      // Optimistically remove item from list
      queryClient.setQueriesData(
        { queryKey: ['match-reviews', 'items'] },
        (
          old: { items: Array<{ uuid: string }>; total: number } | undefined,
        ) => {
          if (!old) return old
          return {
            ...old,
            items: old.items.filter((item) => item.uuid !== variables.id),
            total: Math.max(0, old.total - 1),
          }
        },
      )

      // Optimistically update stats
      queryClient.setQueryData(
        ['match-reviews', 'stats'],
        (old: { pending: number; totalPending: number } | undefined) => {
          if (!old) return old
          return {
            ...old,
            pending: Math.max(0, old.pending - 1),
            totalPending: Math.max(0, old.totalPending - 1),
          }
        },
      )

      return { previousItems, previousStats }
    },

    onError: (_err, _variables, context) => {
      if (context?.previousItems) {
        queryClient.setQueriesData(
          { queryKey: ['match-reviews', 'items'] },
          context.previousItems,
        )
      }
      if (context?.previousStats) {
        queryClient.setQueryData(
          ['match-reviews', 'stats'],
          context.previousStats,
        )
      }
    },

    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['match-reviews'] })
    },
  })
}

/**
 * Hook to bulk update match reviews
 */
export function useBulkUpdateMatchReviews() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: BulkUpdateRequest) => bulkUpdateMatchReviews(data),

    onMutate: async (variables) => {
      await queryClient.cancelQueries({ queryKey: ['match-reviews'] })

      const previousItems = queryClient.getQueryData(['match-reviews', 'items'])

      // Optimistically remove items from list
      queryClient.setQueriesData(
        { queryKey: ['match-reviews', 'items'] },
        (
          old: { items: Array<{ uuid: string }>; total: number } | undefined,
        ) => {
          if (!old) return old
          const idsSet = new Set(variables.ids)
          return {
            ...old,
            items: old.items.filter((item) => !idsSet.has(item.uuid)),
            total: Math.max(0, old.total - variables.ids.length),
          }
        },
      )

      return { previousItems }
    },

    onError: (_err, _variables, context) => {
      if (context?.previousItems) {
        queryClient.setQueriesData(
          { queryKey: ['match-reviews', 'items'] },
          context.previousItems,
        )
      }
    },

    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['match-reviews'] })
    },
  })
}

/**
 * Hook to prefetch match reviews
 */
export function usePrefetchMatchReviews() {
  const queryClient = useQueryClient()

  return (params: MatchReviewParams) => {
    queryClient.prefetchQuery({
      queryKey: ['match-reviews', 'items', params],
      queryFn: () => getMatchReviews(params),
      staleTime: 10 * 1000,
    })
  }
}
