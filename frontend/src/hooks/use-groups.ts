import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import type { CreateGroupRequest, UpdateGroupRequest } from '@/schema/groups'
import {
  createGroup,
  deleteGroup,
  getGroup,
  getGroups,
  updateGroup,
} from '@/api/groups'

/**
 * Hook to fetch all groups
 */
export function useGroups() {
  return useQuery({
    queryKey: ['groups'],
    queryFn: getGroups,
    staleTime: 30 * 1000, // Consider data fresh for 30 seconds
  })
}

/**
 * Hook to fetch a single group by JID
 */
export function useGroup(jid: string) {
  return useQuery({
    queryKey: ['groups', jid],
    queryFn: () => getGroup(jid),
    enabled: !!jid, // Only fetch if jid is provided
  })
}

/**
 * Hook to create a new group
 */
export function useCreateGroup() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (request: CreateGroupRequest) => createGroup(request),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['groups'] })
    },
  })
}

/**
 * Hook to update a group
 */
export function useUpdateGroup() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({
      jid,
      request,
    }: {
      jid: string
      request: UpdateGroupRequest
    }) => updateGroup(jid, request),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['groups'] })
    },
  })
}

/**
 * Hook to delete a group
 */
export function useDeleteGroup() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (jid: string) => deleteGroup(jid),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['groups'] })
    },
  })
}
