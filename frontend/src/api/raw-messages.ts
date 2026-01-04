import apiClient from './client'
import type { ApiResponse } from '@/schema/api'
import type { RawMessage, RawMessageParams } from '@/schema/raw-message'

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
