import { z } from 'zod'

/**
 * New medication parsing schema matching the updated backend prompt structure
 */

// Urgency levels (lowercase to match new backend format)
export const UrgencyLevelSchema = z.enum([
  'normal',
  'soon',
  'urgent',
  'critical',
])
export type UrgencyLevel = z.infer<typeof UrgencyLevelSchema>

// Intent types (lowercase to match new backend format)
export const IntentSchema = z.enum(['offer', 'request'])
export type Intent = z.infer<typeof IntentSchema>

// Individual medication entry from new prompt structure
export const MedicationSchema = z.object({
  // Exact medication name from message (preserves original language)
  name: z.string(),

  // Dosage/strength (e.g., "36", "1mg", "150") - can be null
  concentration: z.string().nullable().optional(),

  // Physical form (امبول, فايل, اقراص, etc.) - can be null
  form: z.string().nullable().optional(),

  // Expiration date (MM/YY format or description) - can be null
  expiry: z.string().nullable().optional(),

  // AI confidence score (0.0 to 1.0)
  confidence: z.number().min(0).max(1),

  // Extraction accuracy explanation
  reason: z.string(),
})

export type Medication = z.infer<typeof MedicationSchema>

// Parse result from new prompt structure
export const ParseResultSchema = z.object({
  // Intent: "offer" or "request"
  intent: IntentSchema,

  // Urgency level: "critical", "urgent", "soon", or "normal"
  urgency: UrgencyLevelSchema,

  // Brief explanation including urgency assessment
  reason: z.string(),

  // List of extracted medications
  medications: z.array(MedicationSchema),
})

export type ParseResult = z.infer<typeof ParseResultSchema>

// Helper functions for urgency level
export const urgencyLevelToDisplay = (level: UrgencyLevel): string => {
  const map: Record<UrgencyLevel, string> = {
    normal: 'Normal',
    soon: 'Soon',
    urgent: 'Urgent',
    critical: 'Critical',
  }
  return map[level]
}

export const urgencyLevelToColor = (level: UrgencyLevel): string => {
  const map: Record<UrgencyLevel, string> = {
    normal: 'gray',
    soon: 'blue',
    urgent: 'orange',
    critical: 'red',
  }
  return map[level]
}

export const urgencyLevelToPriority = (level: UrgencyLevel): number => {
  const map: Record<UrgencyLevel, number> = {
    normal: 0,
    soon: 1,
    urgent: 2,
    critical: 3,
  }
  return map[level]
}

// Helper to convert legacy urgency to new format
export const legacyUrgencyToNew = (
  urgentFlag: boolean,
  urgencyLevel?: string,
): UrgencyLevel => {
  if (urgencyLevel) {
    const normalized = urgencyLevel.toLowerCase()
    if (
      normalized === 'critical' ||
      normalized === 'urgent' ||
      normalized === 'soon' ||
      normalized === 'normal'
    ) {
      return normalized
    }
  }
  return urgentFlag ? 'urgent' : 'normal'
}

// Helper to build display name from medication
export const buildMedicationDisplayName = (med: Medication): string => {
  let name = med.name
  if (med.concentration) {
    name += ` ${med.concentration}`
  }
  if (med.form) {
    name += ` (${med.form})`
  }
  return name
}
