import { z } from 'zod'

// Offer status enum matching Rust's Status
export const OfferStatusSchema = z.enum([
  'Active',
  'Matched',
  'Expired',
  'Cancelled',
])

export type OfferStatus = z.infer<typeof OfferStatusSchema>

// Urgency level enum matching Rust's UrgencyLevel
export const UrgencyLevelSchema = z.enum(['Normal', 'Urgent', 'Critical'])

export type UrgencyLevel = z.infer<typeof UrgencyLevelSchema>

// Offer schema matching Rust's Offer entity
export const OfferSchema = z.object({
  id: z.string().uuid(),
  raw_message_id: z.string().uuid(),
  participant_id: z.string().uuid(),
  group_id: z.string().uuid(),
  medication: z.string(),
  medication_raw: z.string(),
  quantity: z.string().nullable(), // Decimal comes as string from Rust
  unit: z.string().nullable(),
  price: z.string().nullable(), // Decimal comes as string from Rust
  currency: z.string().nullable(),
  expiry_date: z.string().nullable(),
  batch_number: z.string().nullable(),
  notes: z.string().nullable(),
  status: OfferStatusSchema,
  urgency_level: UrgencyLevelSchema,
  expiry_info: z.string().nullable(),
  ai_confidence: z.number(),
  master_medication_id: z.string().uuid().nullable(),
  medication_curated: z.boolean(),
  confirmed_match_count: z.number().default(0),
  created_at: z.string(), // ISO date string
  updated_at: z.string(), // ISO date string
})

export type Offer = z.infer<typeof OfferSchema>

// Stats schema matching Rust's Stats
export const StatsSchema = z.object({
  active_offers: z.number(),
  active_requests: z.number(),
  pending_matches: z.number(),
  confirmed_today: z.number(),
  processed_today: z.number(),
  avg_match_score: z.number(),
  monitored_groups: z.number(),
  connected_clients: z.number(),
})

export type Stats = z.infer<typeof StatsSchema>

// Request status (same as Offer status)
export const RequestStatusSchema = OfferStatusSchema

export type RequestStatus = z.infer<typeof RequestStatusSchema>

// Request schema matching Rust's Request entity
export const RequestSchema = z.object({
  id: z.string().uuid(),
  raw_message_id: z.string().uuid(),
  participant_id: z.string().uuid(),
  group_id: z.string().uuid(),
  medication: z.string(),
  medication_raw: z.string(),
  quantity: z.string().nullable(), // Decimal comes as string from Rust
  unit: z.string().nullable(),
  max_price: z.string().nullable(), // Decimal comes as string from Rust
  currency: z.string().nullable(),
  urgency_level: UrgencyLevelSchema,
  expiry_requirement: z.string().nullable(),
  ai_confidence: z.number(),
  notes: z.string().nullable(),
  status: RequestStatusSchema,
  master_medication_id: z.string().uuid().nullable(),
  medication_curated: z.boolean(),
  confirmed_match_count: z.number().default(0),
  created_at: z.string(), // ISO date string
  updated_at: z.string(), // ISO date string
})

export type Request = z.infer<typeof RequestSchema>
