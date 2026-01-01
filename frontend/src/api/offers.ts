import apiClient from './client'
import type { ApiResponse, PaginationParams } from '@/schema/api'
import type { Offer, Stats, Request } from '@/schema/offer-request'

/**
 * Fetch paginated offers from the API
 */
export async function getOffers(
  params: PaginationParams = {},
): Promise<ApiResponse<Offer[]>> {
  const { limit = 20, offset = 0 } = params

  const response = await apiClient.get<ApiResponse<Offer[]>>('/api/offers', {
    params: { limit, offset },
  })

  return response.data
}

/**
 * Fetch paginated requests from the API
 */
export async function getRequests(
  params: PaginationParams = {},
): Promise<ApiResponse<Request[]>> {
  const { limit = 20, offset = 0 } = params

  const response = await apiClient.get<ApiResponse<Request[]>>(
    '/api/requests',
    {
      params: { limit, offset },
    },
  )

  return response.data
}

/**
 * Fetch dashboard stats from the API
 */
export async function getStats(): Promise<Stats> {
  const response = await apiClient.get<Stats>('/api/stats')
  return response.data
}
