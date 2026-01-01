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
