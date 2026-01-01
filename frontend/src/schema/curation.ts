import { z } from 'zod'

export const CurationStatusSchema = z.enum(['Pending', 'Approved', 'Rejected'])

export const CurationStatsSchema = z.object({
  totalAliases: z.number(),
  pendingCount: z.number(),
  approvedCount: z.number(),
  rejectedCount: z.number(),
  curationPercentage: z.number(),
})

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
  occurrenceCount: z.number().default(1),
  firstSeenAt: z.string().optional(),
  aiSuggestionConfidence: z.number().nullable().optional(),
})

export const MasterSuggestionSchema = z.object({
  master: MedicationMasterSchema,
  score: z.number(),
  method: z.string(),
})

export const AliasListResponseSchema = z.object({
  aliases: z.array(MedicationAliasSchema),
  total: z.number(),
})

export const SuggestionResponseSchema = z.object({
  suggestions: z.array(MasterSuggestionSchema),
})

export const CreateMasterRequestSchema = z
  .object({
    name: z.string().optional(),
    nameAr: z.string().optional(),
    activeIngredient: z.string().optional(),
    strength: z.string().optional(),
    manufacturer: z.string().optional(),
  })
  .refine((data) => data.name || data.nameAr, {
    message: 'At least one name (English or Arabic) is required',
    path: ['name'],
  })

// Infer types
export type CurationStats = z.infer<typeof CurationStatsSchema>
export type MedicationMaster = z.infer<typeof MedicationMasterSchema>
export type MedicationAlias = z.infer<typeof MedicationAliasSchema>
export type Suggestion = z.infer<typeof MasterSuggestionSchema>
export type AliasListResponse = z.infer<typeof AliasListResponseSchema>
