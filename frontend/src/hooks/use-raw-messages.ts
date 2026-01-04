import { useQuery, keepPreviousData } from '@tanstack/react-query'
import { getRawMessages, getRawMessage } from '@/api/raw-messages'
import { queryKeys } from './query-keys'
import type { RawMessageParams } from '@/schema/raw-message'

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
