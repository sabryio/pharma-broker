import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import {
  createPriorityMedication,
  deletePriorityMedication,
  getActivePriorityMedications,
  getPriorityMedications,
  updatePriorityMedication,
} from '@/api/priority-medications'
import type {
  CreatePriorityRequest,
  UpdatePriorityRequest,
} from '@/schema/priority-medications'

const QUERY_KEY = ['priority-medications']

/**
 * Hook to fetch all priority medications
 */
export function usePriorityMedications() {
  return useQuery({
    queryKey: QUERY_KEY,
    queryFn: getPriorityMedications,
    staleTime: 30_000, // 30 seconds
  })
}

/**
 * Hook to fetch only active priority medications
 */
export function useActivePriorityMedications() {
  return useQuery({
    queryKey: [...QUERY_KEY, 'active'],
    queryFn: getActivePriorityMedications,
    staleTime: 30_000,
  })
}

/**
 * Hook to create a new priority medication
 */
export function useCreatePriorityMedication() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (request: CreatePriorityRequest) =>
      createPriorityMedication(request),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEY })
      toast.success('Priority medication created', {
        description: 'The medication has been added to the priority list',
      })
    },
    onError: (error: Error) => {
      toast.error('Failed to create priority medication', {
        description: error.message,
      })
    },
  })
}

/**
 * Hook to update a priority medication
 */
export function useUpdatePriorityMedication() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({
      id,
      request,
    }: {
      id: string
      request: UpdatePriorityRequest
    }) => updatePriorityMedication(id, request),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEY })
      toast.success('Priority medication updated', {
        description: 'Changes have been saved successfully',
      })
    },
    onError: (error: Error) => {
      toast.error('Failed to update priority medication', {
        description: error.message,
      })
    },
  })
}

/**
 * Hook to delete a priority medication
 */
export function useDeletePriorityMedication() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => deletePriorityMedication(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEY })
      toast.success('Priority medication deleted', {
        description: 'The medication has been removed from the priority list',
      })
    },
    onError: (error: Error) => {
      toast.error('Failed to delete priority medication', {
        description: error.message,
      })
    },
  })
}
