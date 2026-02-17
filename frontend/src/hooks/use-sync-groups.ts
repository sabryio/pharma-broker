import { useMutation, useQueryClient } from '@tanstack/react-query'
import { syncGroups } from '@/api/participants'

export function useSyncGroups() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: syncGroups,
    onSuccess: () => {
      // Invalidate groups query to refresh the list
      queryClient.invalidateQueries({ queryKey: ['groups'] })
    },
  })
}
