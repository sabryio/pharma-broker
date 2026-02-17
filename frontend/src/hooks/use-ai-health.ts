import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  getAiHealth,
  getCircuitBreaker,
  getRetryQueue,
  testConnection,
} from '@/api/ai-health'

/**
 * Hook to fetch comprehensive AI health status
 * Refreshes every 10 seconds
 */
export function useAiHealth() {
  return useQuery({
    queryKey: ['ai-health'],
    queryFn: getAiHealth,
    refetchInterval: 10000, // Refresh every 10 seconds
    staleTime: 5000, // Consider data fresh for 5 seconds
  })
}

/**
 * Hook to fetch circuit breaker status only
 * Refreshes every 5 seconds
 */
export function useCircuitBreaker() {
  return useQuery({
    queryKey: ['ai-health', 'circuit-breaker'],
    queryFn: getCircuitBreaker,
    refetchInterval: 5000,
    staleTime: 2000,
  })
}

/**
 * Hook to fetch retry queue statistics
 * Refreshes every 10 seconds
 */
export function useRetryQueue() {
  return useQuery({
    queryKey: ['ai-health', 'retry-queue'],
    queryFn: getRetryQueue,
    refetchInterval: 10000,
    staleTime: 5000,
  })
}

/**
 * Hook to test connection to AI gateway
 */
export function useTestConnection() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: testConnection,
    onSuccess: () => {
      // Invalidate AI health queries to refresh data
      queryClient.invalidateQueries({ queryKey: ['ai-health'] })
    },
  })
}
