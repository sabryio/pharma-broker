import { z } from 'zod'

// ============================================================================
// Zod Schemas
// ============================================================================

export const ReviewStatusSchema = z.enum([
  'Pending',
  'Approved',
  'Rejected',
  'Skipped',
])

export const ApiReviewItemSchema = z.object({
  id: z.uuid(),
  rawMessageId: z.uuid(),
  aiResult: z.record(z.string(), z.unknown()),
  confidence: z.number().min(0).max(1),
  reason: z.string(),
  status: ReviewStatusSchema,
  reviewedBy: z.string().nullable(),
  reviewNotes: z.string().nullable(),
  createdAt: z.string(),
  reviewedAt: z.string().nullable(),
  originalText: z.string(),
  messageTimestamp: z.string(),
  senderName: z.string().nullable(),
  senderPhone: z.string().nullable(),
  groupName: z.string().nullable(),
})

export const ReviewQueueListResponseSchema = z.object({
  items: z.array(ApiReviewItemSchema),
  total: z.number(),
  limit: z.number(),
  offset: z.number(),
})

export const ReviewQueueStatsSchema = z.object({
  pending: z.number(),
  approved: z.number(),
  rejected: z.number(),
  skipped: z.number(),
})

export const UpdateReviewStatusRequestSchema = z.object({
  status: ReviewStatusSchema.exclude(['Pending']),
  reviewed_by: z.string(),
  notes: z.string().optional(),
})

export const UpdateReviewStatusResponseSchema = z.object({
  success: z.boolean(),
  id: z.string().uuid(),
  new_status: z.string(),
})

export const ReviewQueueParamsSchema = z.object({
  limit: z.number().optional(),
  offset: z.number().optional(),
  status: z.string().optional(),
})

// ============================================================================
// Inferred Types
// ============================================================================

export type ReviewStatus = z.infer<typeof ReviewStatusSchema>
export type ApiReviewItem = z.infer<typeof ApiReviewItemSchema>
export type ReviewQueueListResponse = z.infer<
  typeof ReviewQueueListResponseSchema
>
export type ReviewQueueStats = z.infer<typeof ReviewQueueStatsSchema>
export type UpdateReviewStatusRequest = z.infer<
  typeof UpdateReviewStatusRequestSchema
>
export type UpdateReviewStatusResponse = z.infer<
  typeof UpdateReviewStatusResponseSchema
>
export type ReviewQueueParams = z.infer<typeof ReviewQueueParamsSchema>
