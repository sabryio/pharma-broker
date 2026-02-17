import { useQuery } from '@tanstack/react-query'
import { getCommonGroups } from '@/api/participants'

export function useCommonGroups(jid1?: string | null, jid2?: string | null) {
  return useQuery({
    queryKey: ['common-groups', jid1, jid2],
    queryFn: () => {
      if (!jid1 || !jid2) {
        throw new Error('Both JIDs are required')
      }
      return getCommonGroups(jid1, jid2)
    },
    enabled: !!jid1 && !!jid2,
    staleTime: 5 * 60 * 1000, // 5 minutes
  })
}
