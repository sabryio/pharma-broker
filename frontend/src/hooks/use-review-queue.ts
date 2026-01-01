import {
  useQuery,
  useMutation,
  useQueryClient,
  keepPreviousData,
} from '@tanstack/react-query'
import {
  getReviewQueueItems,
  getReviewQueueItem,
  updateReviewStatus,
  getReviewQueueStats,
  transformToParsingReviewItem,
  transformToParsingStats,
} from '@/api/review-queue'
import type {
  ReviewQueueParams,
  UpdateReviewStatusRequest,
} from '@/schema/review-queue'

/**
 * Hook to fetch paginated review queue items
 */
export function useReviewQueueItems(params: ReviewQueueParams = {}) {
  const { limit = 20, offset = 0, status = 'pending' } = params

  return useQuery({
    queryKey: ['review-queue', 'items', { limit, offset, status }],
    queryFn: () => getReviewQueueItems({ limit, offset, status }),
    placeholderData: keepPreviousData,
    staleTime: 10 * 1000, // 10 seconds
    select: (data) => ({
      ...data,
      items: data.items.map(transformToParsingReviewItem),
    }),
  })
}

/**
 * Hook to fetch a single review queue item
 */
export function useReviewQueueItem(id: string | undefined) {
  return useQuery({
    queryKey: ['review-queue', 'item', id],
    queryFn: () => getReviewQueueItem(id!),
    enabled: !!id,
    staleTime: 10 * 1000,
    select: transformToParsingReviewItem,
  })
}

/**
 * Hook to fetch review queue statistics
 */
export function useReviewQueueStats() {
  return useQuery({
    queryKey: ['review-queue', 'stats'],
    queryFn: getReviewQueueStats,
    staleTime: 30 * 1000, // 30 seconds
    refetchInterval: 30 * 1000,
    select: (data) => transformToParsingStats(data),
  })
}

/**
 * Hook to update review queue item status with optimistic updates
 */
export function useUpdateReviewStatus() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({
      id,
      status,
      notes,
    }: {
      id: string
      status: 'approved' | 'rejected' | 'skipped'
      notes?: string
    }) => {
      const request: UpdateReviewStatusRequest = {
        status,
        reviewed_by: 'current-user', // TODO: Get from auth context
        notes,
      }
      return updateReviewStatus(id, request)
    },

    // Optimistic update
    onMutate: async (variables) => {
      // Cancel outgoing refetches
      await queryClient.cancelQueries({ queryKey: ['review-queue'] })

      // Snapshot previous value
      const previousItems = queryClient.getQueryData(['review-queue', 'items'])
      const previousStats = queryClient.getQueryData(['review-queue', 'stats'])

      // Optimistically remove item from list
      queryClient.setQueriesData(
        { queryKey: ['review-queue', 'items'] },
        (old: { items: Array<{ id: string }>; total: number } | undefined) => {
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
        ['review-queue', 'stats'],
        (
          old:
            | {
                pending: number
                approved: number
                rejected: number
                skipped: number
              }
            | undefined,
        ) => {
          if (!old) return old
          return {
            ...old,
            pending: Math.max(0, old.pending - 1),
            [variables.status]: old[variables.status] + 1,
          }
        },
      )

      return { previousItems, previousStats }
    },

    // Rollback on error
    onError: (_err, _variables, context) => {
      if (context?.previousItems) {
        queryClient.setQueriesData(
          { queryKey: ['review-queue', 'items'] },
          context.previousItems,
        )
      }
      if (context?.previousStats) {
        queryClient.setQueryData(
          ['review-queue', 'stats'],
          context.previousStats,
        )
      }
    },

    // Refetch after mutation
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['review-queue'] })
    },
  })
}

/**
 * Hook to prefetch the next page of review items
 */
export function usePrefetchReviewItems() {
  const queryClient = useQueryClient()

  return (params: ReviewQueueParams) => {
    queryClient.prefetchQuery({
      queryKey: ['review-queue', 'items', params],
      queryFn: () => getReviewQueueItems(params),
      staleTime: 10 * 1000,
    })
  }
}
