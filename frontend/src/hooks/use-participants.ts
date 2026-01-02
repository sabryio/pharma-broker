import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiClient } from '@/api/client'
import { queryKeys } from './query-keys'

// ============================================================================
// Types
// ============================================================================

export interface ParticipantStats {
  participantId: string
  displayName: string | null
  phone: string | null
  jid: string | null
  totalOffers: number
  totalRequests: number
  confirmedMatches: number
  rejectedMatches: number
  approvalRate: number
  avgConfidence: number
  lastActivity: string | null
  reputation: 'new' | 'regular' | 'trusted'
}

// ============================================================================
// API Functions
// ============================================================================

async function getParticipantStats(id: string): Promise<ParticipantStats> {
  const response = await apiClient.get(`/api/participants/${id}/stats`)
  return response.data
}

async function getParticipantByJid(jid: string): Promise<ParticipantStats> {
  const response = await apiClient.get(
    `/api/participants/by-jid/${encodeURIComponent(jid)}`,
  )
  return response.data
}

async function updateMatchNotes(
  id: string,
  notes: string,
): Promise<{ success: boolean; id: string; notes: string }> {
  const response = await apiClient.put(`/api/match-reviews/${id}/notes`, {
    notes,
  })
  return response.data
}

// ============================================================================
// Hooks
// ============================================================================

/**
 * Hook to fetch participant statistics by ID
 */
export function useParticipantStats(id: string | undefined) {
  return useQuery({
    queryKey: queryKeys.participants.stats(id ?? ''),
    queryFn: () => getParticipantStats(id!),
    enabled: !!id,
    staleTime: 60 * 1000, // Cache for 1 minute
  })
}

/**
 * Hook to fetch participant statistics by JID
 */
export function useParticipantByJid(jid: string | undefined) {
  return useQuery({
    queryKey: queryKeys.participants.byJid(jid ?? ''),
    queryFn: () => getParticipantByJid(jid!),
    enabled: !!jid,
    staleTime: 60 * 1000,
  })
}

/**
 * Hook to update match notes
 */
export function useUpdateMatchNotes() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({ id, notes }: { id: string; notes: string }) =>
      updateMatchNotes(id, notes),

    onSuccess: (_, variables) => {
      // Invalidate the specific match review
      queryClient.invalidateQueries({
        queryKey: queryKeys.matchReviews.detail(variables.id),
      })
      // Also invalidate the list to update notes display
      queryClient.invalidateQueries({
        queryKey: queryKeys.matchReviews.lists(),
      })
    },
  })
}
