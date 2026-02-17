import apiClient from './client'
import type { ApiResponse } from '@/schema/api'
import type {
  RawMessage,
  RawMessageParams,
  ReprocessResponse,
} from '@/schema/raw-message'

// =============================================================================
// Types for Bulk Operations
// =============================================================================

/**
 * Individual failure in a bulk operation
 */
export interface BulkOperationFailure {
  id: string
  error: string
}

/**
 * Response from bulk operations containing succeeded and failed items
 */
export interface BulkOperationResult {
  succeeded: string[]
  failed: BulkOperationFailure[]
}

// =============================================================================
// Query Functions
// =============================================================================

/**
 * Fetch paginated raw messages from the API with optional filters
 */
export async function getRawMessages(
  params: RawMessageParams = {},
): Promise<ApiResponse<RawMessage[]>> {
  const {
    limit = 20,
    offset = 0,
    search,
    status,
    sort_by,
    sort_order,
    start_date,
    end_date,
  } = params

  const queryParams: Record<string, string | number> = { limit, offset }

  if (search) queryParams.search = search
  if (status && status !== 'all') queryParams.status = status
  if (sort_by) queryParams.sort_by = sort_by
  if (sort_order) queryParams.sort_order = sort_order
  if (start_date) queryParams.start_date = start_date
  if (end_date) queryParams.end_date = end_date

  const response = await apiClient.get<ApiResponse<RawMessage[]>>(
    '/api/raw-messages',
    { params: queryParams },
  )

  return response.data
}

/**
 * Fetch a single raw message by ID for detail view
 */
export async function getRawMessage(
  id: string,
): Promise<ApiResponse<RawMessage>> {
  const response = await apiClient.get<ApiResponse<RawMessage>>(
    `/api/raw-messages/${id}`,
  )

  return response.data
}

// =============================================================================
// Single Message Operations
// =============================================================================

/**
 * Reprocess a single raw message through the parsing pipeline
 * Triggers AI parsing inline and returns the result with item counts
 */
export async function reprocessMessage(
  id: string,
): Promise<ApiResponse<ReprocessResponse>> {
  const response = await apiClient.post<ApiResponse<ReprocessResponse>>(
    `/api/raw-messages/${id}/reprocess`,
  )

  return response.data
}

/**
 * Delete a single raw message
 * Will fail with 409 Conflict if the message has associated offers or requests
 */
export async function deleteMessage(id: string): Promise<ApiResponse<void>> {
  const response = await apiClient.delete<ApiResponse<void>>(
    `/api/raw-messages/${id}`,
  )

  return response.data
}

/**
 * Update message status (mark as processed)
 * Only 'processed' status is currently supported
 */
export async function updateMessageStatus(
  id: string,
  status: 'processed',
): Promise<ApiResponse<RawMessage>> {
  const response = await apiClient.patch<ApiResponse<RawMessage>>(
    `/api/raw-messages/${id}/status`,
    { status },
  )

  return response.data
}

// =============================================================================
// Bulk Operations
// =============================================================================

/**
 * Bulk reprocess multiple raw messages
 * Returns per-item status for each message
 */
export async function bulkReprocessMessages(
  ids: string[],
): Promise<ApiResponse<BulkOperationResult>> {
  const response = await apiClient.post<ApiResponse<BulkOperationResult>>(
    '/api/raw-messages/bulk/reprocess',
    { ids },
  )

  return response.data
}

/**
 * Bulk delete multiple raw messages
 * Messages with associated offers/requests will fail individually
 * Returns per-item status for each message
 */
export async function bulkDeleteMessages(
  ids: string[],
): Promise<ApiResponse<BulkOperationResult>> {
  const response = await apiClient.post<ApiResponse<BulkOperationResult>>(
    '/api/raw-messages/bulk/delete',
    { ids },
  )

  return response.data
}

/**
 * Bulk mark multiple messages as processed
 * Already processed messages will fail individually
 * Returns per-item status for each message
 */
export async function bulkMarkProcessed(
  ids: string[],
): Promise<ApiResponse<BulkOperationResult>> {
  const response = await apiClient.post<ApiResponse<BulkOperationResult>>(
    '/api/raw-messages/bulk/mark-processed',
    { ids },
  )

  return response.data
}

/**
 * Get all failed message IDs for auto-reprocess
 * Returns array of message IDs with error status
 */
export async function getFailedMessageIds(): Promise<ApiResponse<string[]>> {
  const response = await apiClient.get<ApiResponse<string[]>>(
    '/api/raw-messages/failed-ids',
  )

  return response.data
}
