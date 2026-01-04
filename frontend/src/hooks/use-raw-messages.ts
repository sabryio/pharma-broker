import {
  useQuery,
  useMutation,
  useQueryClient,
  keepPreviousData,
} from '@tanstack/react-query'
import {
  getRawMessages,
  getRawMessage,
  reprocessMessage,
  deleteMessage,
  updateMessageStatus,
  bulkReprocessMessages,
  bulkDeleteMessages,
  bulkMarkProcessed,
} from '@/api/raw-messages'
import type { BulkOperationResult } from '@/api/raw-messages'
import { queryKeys } from './query-keys'
import type { RawMessage, RawMessageParams } from '@/schema/raw-message'
import type { ApiResponse } from '@/schema/api'

/**
 * Hook to fetch paginated raw messages with filtering and sorting
 */
export function useRawMessages(params: RawMessageParams = {}) {
  const {
    limit = 20,
    offset = 0,
    search,
    status,
    sort_by = 'timestamp',
    sort_order = 'desc',
    start_date,
    end_date,
  } = params

  return useQuery({
    queryKey: queryKeys.rawMessages.list({
      limit,
      offset,
      search,
      status,
      sort_by,
      sort_order,
      start_date,
      end_date,
    }),
    queryFn: () =>
      getRawMessages({
        limit,
        offset,
        search,
        status,
        sort_by,
        sort_order,
        start_date,
        end_date,
      }),
    placeholderData: keepPreviousData,
    staleTime: 30 * 1000, // 30 seconds
  })
}

/**
 * Hook to fetch a single raw message by ID
 */
export function useRawMessage(id: string | undefined) {
  return useQuery({
    queryKey: queryKeys.rawMessages.detail(id ?? ''),
    queryFn: () => getRawMessage(id!),
    enabled: !!id,
    staleTime: 30 * 1000,
  })
}

// =============================================================================
// Single Message Mutation Hooks
// =============================================================================

/**
 * Hook to reprocess a single raw message
 * Includes optimistic update to show "processing" status
 */
export function useReprocessMessage() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => reprocessMessage(id),

    // Optimistic update
    onMutate: async (id) => {
      // Cancel outgoing refetches
      await queryClient.cancelQueries({ queryKey: queryKeys.rawMessages.all })

      // Snapshot previous values
      const previousLists = queryClient.getQueriesData({
        queryKey: queryKeys.rawMessages.lists(),
      })

      // Optimistically update the message status to "unprocessed" (queued for reprocessing)
      queryClient.setQueriesData(
        { queryKey: queryKeys.rawMessages.lists() },
        (
          old: ApiResponse<RawMessage[]> | undefined,
        ): ApiResponse<RawMessage[]> | undefined => {
          if (!old?.data) return old
          return {
            ...old,
            data: old.data.map((msg) =>
              msg.id === id
                ? {
                    ...msg,
                    status: 'unprocessed',
                    processedAt: null,
                    error: null,
                  }
                : msg,
            ),
          }
        },
      )

      return { previousLists }
    },

    // Rollback on error
    onError: (_err, _id, context) => {
      if (context?.previousLists) {
        for (const [queryKey, data] of context.previousLists) {
          queryClient.setQueryData(queryKey, data)
        }
      }
    },

    // Refetch after mutation
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.rawMessages.all })
    },
  })
}

/**
 * Hook to delete a single raw message
 * Includes optimistic update to hide the row immediately
 */
export function useDeleteMessage() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => deleteMessage(id),

    // Optimistic update
    onMutate: async (id) => {
      // Cancel outgoing refetches
      await queryClient.cancelQueries({ queryKey: queryKeys.rawMessages.all })

      // Snapshot previous values
      const previousLists = queryClient.getQueriesData({
        queryKey: queryKeys.rawMessages.lists(),
      })

      // Optimistically remove the message from lists
      queryClient.setQueriesData(
        { queryKey: queryKeys.rawMessages.lists() },
        (
          old: ApiResponse<RawMessage[]> | undefined,
        ): ApiResponse<RawMessage[]> | undefined => {
          if (!old?.data) return old
          return {
            ...old,
            data: old.data.filter((msg) => msg.id !== id),
            meta: old.meta
              ? { ...old.meta, total: Math.max(0, old.meta.total - 1) }
              : old.meta,
          }
        },
      )

      return { previousLists }
    },

    // Rollback on error
    onError: (_err, _id, context) => {
      if (context?.previousLists) {
        for (const [queryKey, data] of context.previousLists) {
          queryClient.setQueryData(queryKey, data)
        }
      }
    },

    // Refetch after mutation
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.rawMessages.all })
    },
  })
}

/**
 * Hook to update message status (mark as processed)
 * Includes optimistic update to show new status immediately
 */
export function useUpdateMessageStatus() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => updateMessageStatus(id, 'processed'),

    // Optimistic update
    onMutate: async (id) => {
      // Cancel outgoing refetches
      await queryClient.cancelQueries({ queryKey: queryKeys.rawMessages.all })

      // Snapshot previous values
      const previousLists = queryClient.getQueriesData({
        queryKey: queryKeys.rawMessages.lists(),
      })

      // Optimistically update the message status
      queryClient.setQueriesData(
        { queryKey: queryKeys.rawMessages.lists() },
        (
          old: ApiResponse<RawMessage[]> | undefined,
        ): ApiResponse<RawMessage[]> | undefined => {
          if (!old?.data) return old
          return {
            ...old,
            data: old.data.map((msg) =>
              msg.id === id
                ? {
                    ...msg,
                    status: 'processed',
                    processedAt: new Date().toISOString(),
                  }
                : msg,
            ),
          }
        },
      )

      return { previousLists }
    },

    // Rollback on error
    onError: (_err, _id, context) => {
      if (context?.previousLists) {
        for (const [queryKey, data] of context.previousLists) {
          queryClient.setQueryData(queryKey, data)
        }
      }
    },

    // Refetch after mutation
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.rawMessages.all })
    },
  })
}

// =============================================================================
// Bulk Mutation Hooks
// =============================================================================

/**
 * Hook to bulk reprocess multiple raw messages
 * Returns per-item status for success/failure handling
 */
export function useBulkReprocessMessages() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (ids: string[]) => bulkReprocessMessages(ids),

    // Optimistic update
    onMutate: async (ids) => {
      await queryClient.cancelQueries({ queryKey: queryKeys.rawMessages.all })

      const previousLists = queryClient.getQueriesData({
        queryKey: queryKeys.rawMessages.lists(),
      })

      // Optimistically update all selected messages
      queryClient.setQueriesData(
        { queryKey: queryKeys.rawMessages.lists() },
        (
          old: ApiResponse<RawMessage[]> | undefined,
        ): ApiResponse<RawMessage[]> | undefined => {
          if (!old?.data) return old
          const idSet = new Set(ids)
          return {
            ...old,
            data: old.data.map((msg) =>
              idSet.has(msg.id)
                ? {
                    ...msg,
                    status: 'unprocessed',
                    processedAt: null,
                    error: null,
                  }
                : msg,
            ),
          }
        },
      )

      return { previousLists }
    },

    onError: (_err, _ids, context) => {
      if (context?.previousLists) {
        for (const [queryKey, data] of context.previousLists) {
          queryClient.setQueryData(queryKey, data)
        }
      }
    },

    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.rawMessages.all })
    },
  })
}

/**
 * Hook to bulk delete multiple raw messages
 * Returns per-item status for success/failure handling
 */
export function useBulkDeleteMessages() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (ids: string[]) => bulkDeleteMessages(ids),

    // Optimistic update
    onMutate: async (ids) => {
      await queryClient.cancelQueries({ queryKey: queryKeys.rawMessages.all })

      const previousLists = queryClient.getQueriesData({
        queryKey: queryKeys.rawMessages.lists(),
      })

      // Optimistically remove all selected messages
      queryClient.setQueriesData(
        { queryKey: queryKeys.rawMessages.lists() },
        (
          old: ApiResponse<RawMessage[]> | undefined,
        ): ApiResponse<RawMessage[]> | undefined => {
          if (!old?.data) return old
          const idSet = new Set(ids)
          const filteredData = old.data.filter((msg) => !idSet.has(msg.id))
          return {
            ...old,
            data: filteredData,
            meta: old.meta
              ? {
                  ...old.meta,
                  total: Math.max(0, old.meta.total - ids.length),
                }
              : old.meta,
          }
        },
      )

      return { previousLists }
    },

    onError: (_err, _ids, context) => {
      if (context?.previousLists) {
        for (const [queryKey, data] of context.previousLists) {
          queryClient.setQueryData(queryKey, data)
        }
      }
    },

    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.rawMessages.all })
    },
  })
}

/**
 * Hook to bulk mark multiple messages as processed
 * Returns per-item status for success/failure handling
 */
export function useBulkMarkProcessed() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (ids: string[]) => bulkMarkProcessed(ids),

    // Optimistic update
    onMutate: async (ids) => {
      await queryClient.cancelQueries({ queryKey: queryKeys.rawMessages.all })

      const previousLists = queryClient.getQueriesData({
        queryKey: queryKeys.rawMessages.lists(),
      })

      // Optimistically update all selected messages
      queryClient.setQueriesData(
        { queryKey: queryKeys.rawMessages.lists() },
        (
          old: ApiResponse<RawMessage[]> | undefined,
        ): ApiResponse<RawMessage[]> | undefined => {
          if (!old?.data) return old
          const idSet = new Set(ids)
          return {
            ...old,
            data: old.data.map((msg) =>
              idSet.has(msg.id)
                ? {
                    ...msg,
                    status: 'processed',
                    processedAt: new Date().toISOString(),
                  }
                : msg,
            ),
          }
        },
      )

      return { previousLists }
    },

    onError: (_err, _ids, context) => {
      if (context?.previousLists) {
        for (const [queryKey, data] of context.previousLists) {
          queryClient.setQueryData(queryKey, data)
        }
      }
    },

    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.rawMessages.all })
    },
  })
}

// Re-export types for convenience
export type { BulkOperationResult }
