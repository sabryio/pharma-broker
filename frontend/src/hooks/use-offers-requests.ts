import { useQuery, keepPreviousData } from '@tanstack/react-query'
import { getOffers, getRequests, getStats } from '@/api/offers'
import type { PaginationParams } from '@/schema/api'

/**
 * Hook to fetch paginated offers with React Query
 */
export function useOffers(params: PaginationParams = {}) {
  const { limit = 20, offset = 0 } = params

  return useQuery({
    queryKey: ['offers', { limit, offset }],
    queryFn: () => getOffers({ limit, offset }),
    placeholderData: keepPreviousData, // Keep previous data while fetching new page
    staleTime: 30 * 1000, // Consider data fresh for 30 seconds
  })
}

/**
 * Hook to fetch paginated requests with React Query
 */
export function useRequests(params: PaginationParams = {}) {
  const { limit = 20, offset = 0 } = params

  return useQuery({
    queryKey: ['requests', { limit, offset }],
    queryFn: () => getRequests({ limit, offset }),
    placeholderData: keepPreviousData,
    staleTime: 30 * 1000,
  })
}

/**
 * Hook to fetch dashboard stats with React Query
 */
export function useStats() {
  return useQuery({
    queryKey: ['stats'],
    queryFn: getStats,
    staleTime: 60 * 1000, // Consider data fresh for 1 minute
    refetchInterval: 60 * 1000, // Refetch every minute
  })
}
