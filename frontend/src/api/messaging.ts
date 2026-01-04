import apiClient from './client'

/**
 * Request body for sending a WhatsApp message
 */
export interface SendMessageRequest {
  /** WhatsApp JID of the recipient */
  recipient_jid: string
  /** Message content to send */
  content: string
  /** Optional reference ID for tracking */
  reference_id?: string
}

/**
 * Response from send message operation
 */
export interface SendMessageResponse {
  /** Whether the message was sent successfully */
  success: boolean
  /** Message ID on success */
  message_id?: string
  /** Error message on failure */
  error?: string
  /** Error code for programmatic handling */
  code?: string
}

/**
 * Send a WhatsApp message via the API
 */
export async function sendMessage(
  request: SendMessageRequest,
): Promise<SendMessageResponse> {
  const response = await apiClient.post<SendMessageResponse>(
    '/api/messages/send',
    request,
  )
  return response.data
}
