import { useMutation, useQueryClient } from '@tanstack/react-query'
import { sendMessage, type SendMessageRequest } from '@/api/messaging'
import { toast } from 'sonner'

export interface UseSendMessageOptions {
  onSuccess?: (messageId: string) => void
  onError?: (error: Error) => void
}

/**
 * Hook for sending WhatsApp messages via the API
 * Handles success/error callbacks and invalidates audit queries on success
 */
export function useSendMessage(options: UseSendMessageOptions = {}) {
  const queryClient = useQueryClient()
  const { onSuccess, onError } = options

  return useMutation({
    mutationFn: (request: SendMessageRequest) => sendMessage(request),

    onSuccess: (data) => {
      if (data.success && data.message_id) {
        toast.success('Message sent successfully')
        onSuccess?.(data.message_id)
        // Invalidate audit queries to show the new message in audit trail
        queryClient.invalidateQueries({ queryKey: ['audit'] })
        queryClient.invalidateQueries({ queryKey: ['audit-trail'] })
      } else {
        // API returned success: false
        const errorMessage = data.error || 'Failed to send message'
        toast.error('Failed to send message', { description: errorMessage })
        onError?.(new Error(errorMessage))
      }
    },

    onError: (error: Error) => {
      toast.error('Failed to send message', { description: error.message })
      onError?.(error)
    },
  })
}
