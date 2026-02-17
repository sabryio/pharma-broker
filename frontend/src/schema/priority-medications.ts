import { z } from 'zod'

// Priority level enum matching Rust's PriorityLevel
export enum PriorityLevel {
  LOW = 'LOW',
  NORMAL = 'NORMAL',
  HIGH = 'HIGH',
  URGENT = 'URGENT',
  CRITICAL = 'CRITICAL',
}

// Priority level display configuration
export const PRIORITY_CONFIG = {
  LOW: {
    label: 'Low',
    score: 1,
    color: 'text-gray-400',
    bg: 'bg-gray-500/20',
    border: 'border-gray-500/30',
  },
  NORMAL: {
    label: 'Normal',
    score: 3,
    color: 'text-blue-400',
    bg: 'bg-blue-500/20',
    border: 'border-blue-500/30',
  },
  HIGH: {
    label: 'High',
    score: 5,
    color: 'text-yellow-400',
    bg: 'bg-yellow-500/20',
    border: 'border-yellow-500/30',
  },
  URGENT: {
    label: 'Urgent',
    score: 8,
    color: 'text-orange-400',
    bg: 'bg-orange-500/20',
    border: 'border-orange-500/30',
  },
  CRITICAL: {
    label: 'Critical',
    score: 10,
    color: 'text-red-400',
    bg: 'bg-red-500/20',
    border: 'border-red-500/30',
  },
} as const

// Priority medication schema matching Rust's Model
export const PriorityMedicationSchema = z.object({
  id: z.string().uuid(),
  medicationName: z.string(),
  medicationNameAr: z.string().nullable(),
  priorityLevel: z.nativeEnum(PriorityLevel),
  reason: z.string().nullable(),
  active: z.boolean(),
  activeFrom: z.string(), // ISO date string
  activeUntil: z.string().nullable(), // ISO date string
  createdBy: z.string().uuid().nullable(),
  createdAt: z.string(), // ISO date string
  updatedAt: z.string(), // ISO date string
})

export type PriorityMedication = z.infer<typeof PriorityMedicationSchema>

// Response for list operations
export const PriorityListResponseSchema = z.object({
  success: z.boolean(),
  priorities: z.array(PriorityMedicationSchema),
  total: z.number(),
})

export type PriorityListResponse = z.infer<typeof PriorityListResponseSchema>

// Response for single priority operations
export const PriorityResponseSchema = z.object({
  success: z.boolean(),
  priority: PriorityMedicationSchema.nullable(),
  error: z.string().nullable().optional(),
})

export type PriorityResponse = z.infer<typeof PriorityResponseSchema>

// Response for priority check
export const PriorityCheckResponseSchema = z.object({
  isPriority: z.boolean(),
  priorityScore: z.number(),
  medication: PriorityMedicationSchema.nullable(),
})

export type PriorityCheckResponse = z.infer<typeof PriorityCheckResponseSchema>

// Request body for creating a priority medication
export interface CreatePriorityRequest {
  medicationName: string
  medicationNameAr?: string
  priorityLevel: PriorityLevel
  reason?: string
  active?: boolean
  activeFrom?: string // ISO date string
  activeUntil?: string // ISO date string
}

// Request body for updating a priority medication
export interface UpdatePriorityRequest {
  medicationName?: string
  medicationNameAr?: string
  priorityLevel?: PriorityLevel
  reason?: string
  active?: boolean
  activeFrom?: string // ISO date string
  activeUntil?: string // ISO date string
}
