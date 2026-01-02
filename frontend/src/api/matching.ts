import apiClient from './client'
import type { ItemType } from './offers'

export interface RematchRequest {
  item_id: string
  item_type: ItemType
}

export interface RematchResponse {
  success: boolean
  message: string
  matches_cleared: number
  items_queued: number
}

export async function rematchItem(
  req: RematchRequest,
): Promise<RematchResponse> {
  const response = await apiClient.post<RematchResponse>(
    '/api/match/rematch',
    req,
  )
  return response.data
}
