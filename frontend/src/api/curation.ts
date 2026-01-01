import apiClient from './client'
import { z } from 'zod'

export const CurationStatusSchema = z.enum(['Pending', 'Approved', 'Rejected'])

export const MedicationMasterSchema = z.object({
  id: z.string().uuid(),
  name: z.string(),
  canonicalNameAr: z.string().nullable(),
  activeIngredient: z.string().nullable(),
  strength: z.string().nullable(),
  manufacturer: z.string().nullable(),
})

export const MedicationAliasSchema = z.object({
  id: z.string().uuid(),
  aliasName: z.string(),
  masterMedicationId: z.string().uuid().nullable(),
  curationStatus: CurationStatusSchema,
})

export const SuggestionSchema = z.object({
  master: MedicationMasterSchema,
  score: z.number(),
  method: z.string(),
})

export type MedicationMaster = z.infer<typeof MedicationMasterSchema>
export type MedicationAlias = z.infer<typeof MedicationAliasSchema>
export type Suggestion = z.infer<typeof SuggestionSchema>

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
  return z.array(SuggestionSchema).parse(response.data)
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
      master_id: masterId,
    },
  )
  return { success: !!response.data }
}

/**
 * Create a new master record and link the alias to it
 */
export async function createMasterAndLink(
  aliasId: string,
  masterData: Partial<MedicationMaster>,
): Promise<{ success: boolean; master: MedicationMaster }> {
  const response = await apiClient.post<MedicationMaster>(
    '/api/curation/master',
    {
      ...masterData,
      alias_id: aliasId,
    },
  )
  return { success: true, master: MedicationMasterSchema.parse(response.data) }
}
