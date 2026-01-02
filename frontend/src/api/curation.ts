import apiClient from './client'
import {
  CurationStatsSchema,
  MedicationAliasSchema,
  MasterSuggestionSchema,
  MedicationMasterSchema,
  type CurationStats,
  type MedicationAlias,
  type Suggestion,
  type MedicationMaster,
} from '@/schema/curation'

export type { Suggestion, MedicationAlias, MedicationMaster, CurationStats }
import { z } from 'zod'

/**
 * Fetch overall curation statistics
 */
export async function getCurationStats(): Promise<CurationStats> {
  const response = await apiClient.get('/api/curation/stats')
  return CurationStatsSchema.parse(response.data)
}

/**
 * Fetch medication aliases with pagination and filtering
 */
export async function getAliases(params: {
  limit?: number
  offset?: number
  status?: string
}): Promise<{ aliases: MedicationAlias[]; total: number }> {
  const response = await apiClient.get('/api/curation/aliases', { params })
  return z
    .object({
      aliases: z.array(MedicationAliasSchema),
      total: z.number(),
    })
    .parse(response.data)
}

/**
 * Fetch AI suggestions for a medication name
 */
export async function getCurationSuggestions(
  name: string,
): Promise<Suggestion[]> {
  const response = await apiClient.get<Suggestion[]>(
    '/api/curation/suggestions',
    {
      params: { name },
    },
  )
  return z.array(MasterSuggestionSchema).parse(response.data)
}

/**
 * Link an alias to a master medication.
 * If aliasId is provided, updates that alias.
 * If aliasId is null but aliasName is provided, creates a new alias and links it.
 */
export async function linkAliasToMaster(
  masterId: string,
  aliasId: string | null,
  aliasName?: string,
): Promise<{ success: boolean }> {
  if (aliasId) {
    // Update existing alias
    const response = await apiClient.put(
      `/api/curation/aliases/${aliasId}/approve`,
      { masterId },
    )
    return { success: !!response.data }
  } else if (aliasName) {
    // Create new alias and link it
    const response = await apiClient.post('/api/curation/link', {
      masterId,
      aliasName,
    })
    return { success: !!response.data?.success }
  }
  return { success: false }
}

/**
 * Approve an alias by linking it to a master medication
 */
export async function approveMedicationAlias(
  aliasId: string,
  masterId: string,
): Promise<{ success: boolean }> {
  const response = await apiClient.put(
    `/api/curation/aliases/${aliasId}/approve`,
    {
      masterId: masterId,
    },
  )
  return { success: !!response.data }
}

/**
 * Create a new master record and link the alias to it
 */
export async function createMasterAndLink(
  aliasId: string | null,
  masterData: Partial<MedicationMaster>,
  aliasName?: string,
): Promise<{ success: boolean; master: MedicationMaster }> {
  const response = await apiClient.post('/api/curation/master', {
    ...masterData,
    aliasId: aliasId || undefined,
    aliasName: aliasId ? undefined : aliasName, // Only send aliasName if no aliasId
  })
  const data = response.data as { success: boolean; master: MedicationMaster }
  return {
    success: data.success,
    master: MedicationMasterSchema.parse(data.master),
  }
}

/**
 * Bulk approve multiple aliases by linking them to a master
 */
export async function bulkApproveAliases(
  aliasIds: string[],
  masterId: string,
): Promise<{ success: boolean; approvedCount: number; failedCount: number }> {
  const response = await apiClient.post('/api/curation/aliases/bulk-approve', {
    aliasIds,
    masterId,
  })
  return response.data as {
    success: boolean
    approvedCount: number
    failedCount: number
  }
}

/**
 * Update a master medication record
 * Regenerates embedding if canonical names change
 */
export async function updateMaster(
  masterId: string,
  data: {
    name?: string
    nameAr?: string
    activeIngredient?: string
    strength?: string
    manufacturer?: string
  },
): Promise<{ success: boolean; master: MedicationMaster }> {
  const response = await apiClient.put(`/api/curation/master/${masterId}`, data)
  const result = response.data as { success: boolean; master: MedicationMaster }
  return {
    success: result.success,
    master: MedicationMasterSchema.parse(result.master),
  }
}
