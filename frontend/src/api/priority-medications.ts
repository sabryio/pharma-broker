import apiClient from './client'
import type {
  CreatePriorityRequest,
  PriorityCheckResponse,
  PriorityListResponse,
  PriorityResponse,
  UpdatePriorityRequest,
} from '@/schema/priority-medications'

/**
 * Fetch all priority medications from the API
 */
export async function getPriorityMedications(): Promise<PriorityListResponse> {
  const response = await apiClient.get<PriorityListResponse>(
    '/api/priority-medications',
  )
  return response.data
}

/**
 * Fetch only active priority medications
 */
export async function getActivePriorityMedications(): Promise<PriorityListResponse> {
  const response = await apiClient.get<PriorityListResponse>(
    '/api/priority-medications/active',
  )
  return response.data
}

/**
 * Fetch a single priority medication by ID
 */
export async function getPriorityMedication(
  id: string,
): Promise<PriorityResponse> {
  const response = await apiClient.get<PriorityResponse>(
    `/api/priority-medications/${id}`,
  )
  return response.data
}

/**
 * Check if a medication is priority
 */
export async function checkMedicationPriority(
  medication: string,
): Promise<PriorityCheckResponse> {
  const response = await apiClient.get<PriorityCheckResponse>(
    `/api/priority-medications/check/${encodeURIComponent(medication)}`,
  )
  return response.data
}

/**
 * Create a new priority medication
 */
export async function createPriorityMedication(
  request: CreatePriorityRequest,
): Promise<PriorityResponse> {
  const response = await apiClient.post<PriorityResponse>(
    '/api/priority-medications',
    request,
  )
  return response.data
}

/**
 * Update a priority medication by ID
 */
export async function updatePriorityMedication(
  id: string,
  request: UpdatePriorityRequest,
): Promise<PriorityResponse> {
  const response = await apiClient.put<PriorityResponse>(
    `/api/priority-medications/${id}`,
    request,
  )
  return response.data
}

/**
 * Delete a priority medication by ID
 */
export async function deletePriorityMedication(
  id: string,
): Promise<{ success: boolean; message: string }> {
  const response = await apiClient.delete<{
    success: boolean
    message: string
  }>(`/api/priority-medications/${id}`)
  return response.data
}
