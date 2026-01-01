import { z } from 'zod'

// ============================================================================
// Zod Schemas
// ============================================================================

export const MatchStatusSchema = z.enum([
  'PENDING',
  'CONFIRMED',
  'REJECTED',
  'EXPIRED',
])

export const OfferSummarySchema = z.object({
  id: z.uuid(),
  product: z.string(),
  medicationRaw: z.string().nullable(),
  source: z.string(),
  sourceGroup: z.string().nullable(),
  senderName: z.string().nullable(),
  senderJid: z.string().nullable(),
  rawMessage: z.string().nullable(),
  quantity: z.string().nullable(),
  price: z.string().nullable(),
  expiry: z.string().nullable(),
  masterId: z.string().uuid().nullable().optional(),
  medicationAliasId: z.string().uuid().nullable().optional(),
  curationStatus: z.string().nullable().optional(),
})

export const RequestSummarySchema = z.object({
  id: z.uuid(),
  product: z.string(),
  medicationRaw: z.string().nullable(),
  source: z.string(),
  sourceGroup: z.string().nullable(),
  senderName: z.string().nullable(),
  senderJid: z.string().nullable(),
  rawMessage: z.string().nullable(),
  quantity: z.string().nullable(),
  maxPrice: z.string().nullable(),
  urgency: z.string(),
  masterId: z.string().uuid().nullable().optional(),
  medicationAliasId: z.string().uuid().nullable().optional(),
  curationStatus: z.string().nullable().optional(),
})

export const MatchReviewItemSchema = z.object({
  id: z.uuid(),
  confidence: z.number(),
  status: MatchStatusSchema,
  reasoning: z.string().nullable(),
  issues: z.array(z.string()),
  offer: OfferSummarySchema,
  request: RequestSummarySchema,
  createdAt: z.string(),
  confirmedAt: z.string().nullable(),
  notes: z.string().nullable(),
})

export const MatchReviewListResponseSchema = z.object({
  items: z.array(MatchReviewItemSchema),
  total: z.number(),
  limit: z.number(),
  offset: z.number(),
})

export const MatchReviewStatsSchema = z.object({
  pending: z.number(),
  confirmedToday: z.number(),
  rejectedToday: z.number(),
  totalPending: z.number(),
  avgConfidence: z.number(),
})

export const UpdateMatchReviewRequestSchema = z.object({
  action: z.enum(['approved', 'rejected']),
  reviewed_by: z.string(),
  notes: z.string().optional(),
})

export const UpdateMatchReviewResponseSchema = z.object({
  success: z.boolean(),
  id: z.uuid(),
  newStatus: z.string(),
  reviewedAt: z.string().nullable(),
})

export const BulkUpdateRequestSchema = z.object({
  ids: z.array(z.uuid()),
  action: z.enum(['approved', 'rejected']),
  reviewed_by: z.string(),
})

export const BulkUpdateResponseSchema = z.object({
  success: z.boolean(),
  updatedCount: z.number(),
})

export const MatchReviewParamsSchema = z.object({
  limit: z.number().optional(),
  offset: z.number().optional(),
  status: z.string().optional(),
  minScore: z.number().optional(),
})

// ============================================================================
// Inferred Types
// ============================================================================

export type MatchStatus = z.infer<typeof MatchStatusSchema>
export type OfferSummary = z.infer<typeof OfferSummarySchema>
export type RequestSummary = z.infer<typeof RequestSummarySchema>
export type MatchReviewItem = z.infer<typeof MatchReviewItemSchema>
export type MatchReviewListResponse = z.infer<
  typeof MatchReviewListResponseSchema
>
export type MatchReviewStats = z.infer<typeof MatchReviewStatsSchema>
export type UpdateMatchReviewRequest = z.infer<
  typeof UpdateMatchReviewRequestSchema
>
export type UpdateMatchReviewResponse = z.infer<
  typeof UpdateMatchReviewResponseSchema
>
export type BulkUpdateRequest = z.infer<typeof BulkUpdateRequestSchema>
export type BulkUpdateResponse = z.infer<typeof BulkUpdateResponseSchema>
export type MatchReviewParams = z.infer<typeof MatchReviewParamsSchema>
