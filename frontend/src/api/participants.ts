import apiClient from './client'
import type { Group } from '@/schema/groups'

export interface CommonGroupsResponse {
  success: boolean
  common_groups: Array<Group>
  total: number
}

export interface SyncGroupsResponse {
  success: boolean
  message: string
}

/**
 * Get common WhatsApp groups between two participants
 */
export async function getCommonGroups(
  jid1: string,
  jid2: string,
): Promise<CommonGroupsResponse> {
  const response = await apiClient.get<CommonGroupsResponse>(
    `/api/participants/common-groups/${encodeURIComponent(jid1)}/${encodeURIComponent(jid2)}`,
  )
  return response.data
}

/**
 * Trigger a manual group sync from WhatsApp
 */
export async function syncGroups(): Promise<SyncGroupsResponse> {
  const response = await apiClient.post<SyncGroupsResponse>(
    'http://localhost:8081/sync-groups',
  )
  return response.data
}
