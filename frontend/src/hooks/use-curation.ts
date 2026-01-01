import {
  useQuery,
  useMutation,
  useQueryClient,
  keepPreviousData,
} from '@tanstack/react-query'
import {
  getCurationStats,
  getAliases,
  getCurationSuggestions,
  approveMedicationAlias,
  createMasterAndLink,
} from '@/api/curation'
import type { MedicationMaster } from '@/schema/curation'
import { toast } from 'sonner'

/**
 * Hook to fetch overall curation statistics
 */
export function useCurationStats() {
  return useQuery({
    queryKey: ['curation', 'stats'],
    queryFn: getCurationStats,
    staleTime: 30 * 1000,
    refetchInterval: 30 * 1000,
  })
}

/**
 * Hook to fetch paginated medication aliases
 */
export function useAliases(params: {
  limit?: number
  offset?: number
  status?: string
}) {
  return useQuery({
    queryKey: ['curation', 'aliases', params],
    queryFn: () => getAliases(params),
    placeholderData: keepPreviousData,
    staleTime: 10 * 1000,
  })
}

/**
 * Hook to fetch AI suggestions for a medication
 */
export function useSuggestions(name: string | null) {
  return useQuery({
    queryKey: ['curation', 'suggestions', name],
    queryFn: () => getCurationSuggestions(name!),
    enabled: !!name,
    staleTime: 5 * 60 * 1000, // Suggestions are relatively stable
  })
}

/**
 * Hook to approve a medication alias with optimistic updates
 */
export function useApproveAlias() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({
      aliasId,
      masterId,
    }: {
      aliasId: string
      masterId: string
    }) => approveMedicationAlias(aliasId, masterId),

    onMutate: async (variables) => {
      await queryClient.cancelQueries({ queryKey: ['curation'] })
      const previousAliases = queryClient.getQueryData(['curation', 'aliases'])
      const previousStats = queryClient.getQueryData(['curation', 'stats'])

      // Optimistic update for stats
      queryClient.setQueryData(['curation', 'stats'], (old: any) => {
        if (!old) return old
        return {
          ...old,
          pendingCount: Math.max(0, old.pendingCount - 1),
          approvedCount: old.approvedCount + 1,
          curationPercentage:
            ((old.approvedCount + 1) / old.totalAliases) * 100,
        }
      })

      // Optimistic update for aliases list (remove approved item)
      queryClient.setQueriesData(
        { queryKey: ['curation', 'aliases'] },
        (old: any) => {
          if (!old) return old
          return {
            ...old,
            aliases: old.aliases.filter((a: any) => a.id !== variables.aliasId),
            total: Math.max(0, old.total - 1),
          }
        },
      )

      return { previousAliases, previousStats }
    },

    onError: (_err, _variables, context) => {
      if (context?.previousAliases) {
        queryClient.setQueryData(
          ['curation', 'aliases'],
          context.previousAliases,
        )
      }
      if (context?.previousStats) {
        queryClient.setQueryData(['curation', 'stats'], context.previousStats)
      }
    },

    onSuccess: () => {
      toast.success('Medication successfully curated')
    },

    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['curation'] })
      queryClient.invalidateQueries({ queryKey: ['match-reviews'] })
    },
  })
}

/**
 * Hook to create a new master medication and link it
 */
export function useCreateMaster() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({
      aliasId,
      aliasName,
      data,
    }: {
      aliasId: string | null
      aliasName?: string
      data: Partial<MedicationMaster>
    }) => createMasterAndLink(aliasId, data, aliasName),

    onSuccess: () => {
      toast.success('Master medication created and linked')
      queryClient.invalidateQueries({ queryKey: ['curation'] })
      queryClient.invalidateQueries({ queryKey: ['match-reviews'] })
    },
    onError: (err: any) => {
      toast.error('Failed to create master medication', {
        description: err.message,
      })
    },
  })
}
