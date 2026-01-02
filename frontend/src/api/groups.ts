import apiClient from './client'
import type {
  CreateGroupRequest,
  GroupListResponse,
  GroupResponse,
  UpdateGroupRequest,
} from '@/schema/groups'

/**
 * Fetch all groups from the API
 */
export async function getGroups(): Promise<GroupListResponse> {
  console.log('getGroups called')
  const response = await apiClient.get<GroupListResponse>('/api/groups')
  console.log(
    'getGroups response',
    response.data.groups.map((g) => ({ jid: g.jid, monitoring: g.monitoring })),
  )
  return response.data
}

/**
 * Fetch a single group by JID
 */
export async function getGroup(jid: string): Promise<GroupResponse> {
  const response = await apiClient.get<GroupResponse>(
    `/api/groups/${encodeURIComponent(jid)}`,
  )
  return response.data
}

/**
 * Create a new group
 */
export async function createGroup(
  request: CreateGroupRequest,
): Promise<GroupResponse> {
  const response = await apiClient.post<GroupResponse>('/api/groups', request)
  return response.data
}

/**
 * Update a group by JID
 */
export async function updateGroup(
  jid: string,
  request: UpdateGroupRequest,
): Promise<GroupResponse> {
  const response = await apiClient.put<GroupResponse>(
    `/api/groups/${encodeURIComponent(jid)}`,
    request,
  )
  return response.data
}

/**
 * Delete a group by JID
 */
export async function deleteGroup(
  jid: string,
): Promise<{ success: boolean; message: string }> {
  const response = await apiClient.delete<{
    success: boolean
    message: string
  }>(`/api/groups/${encodeURIComponent(jid)}`)
  return response.data
}
