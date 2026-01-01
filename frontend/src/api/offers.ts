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

// ============================================================================
// Reclassification API
// ============================================================================

export type ItemType = 'offer' | 'request'

export interface ReclassifyRequest {
  sourceId: string
  sourceType: ItemType
  targetType: ItemType
  reclassifiedBy: string
  notes?: string
}

export interface ReclassifyResponse {
  success: boolean
  sourceId: string
  newId: string
  newType: ItemType
  message: string
}

export interface ItemSummary {
  id: string
  itemType: ItemType
  medication: string
  medicationRaw: string
  quantity: string | null
  price: string | null
  status: string
  createdAt: string
}

/**
 * Reclassify an item (offer to request or vice versa)
 */
export async function reclassifyItem(
  request: ReclassifyRequest,
): Promise<ReclassifyResponse> {
  const response = await apiClient.post<ReclassifyResponse>(
    '/api/reclassify',
    request,
  )
  return response.data
}

/**
 * Get an item by type and ID
 */
export async function getItem(
  itemType: ItemType,
  id: string,
): Promise<ItemSummary> {
  const response = await apiClient.get<ItemSummary>(
    `/api/items/${itemType}/${id}`,
  )
  return response.data
}
