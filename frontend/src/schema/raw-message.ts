import { z } from 'zod'

// Processing status enum for filtering
export const ProcessingStatusSchema = z.enum([
  'all',
  'processed',
  'unprocessed',
  'error',
])

export type ProcessingStatus = z.infer<typeof ProcessingStatusSchema>

// Sort field enum
export const SortFieldSchema = z.enum([
  'timestamp',
  'processed_at',
  'created_at',
])

export type SortField = z.infer<typeof SortFieldSchema>

// Sort order enum
export const SortOrderSchema = z.enum(['asc', 'desc'])

export type SortOrder = z.infer<typeof SortOrderSchema>

// Raw message schema matching Rust's RawMessageResponse (uses camelCase)
export const RawMessageSchema = z.object({
  id: z.string().uuid(),
  externalId: z.string().nullable(),
  content: z.string(),
  timestamp: z.string().datetime({ offset: true }),
  processedAt: z.string().datetime({ offset: true }).nullable(),
  error: z.string().nullable(),
  replyToId: z.string().nullable(),
  replyToContent: z.string().nullable(),
  replyToSender: z.string().nullable(),
  createdAt: z.string().datetime({ offset: true }),
  // Denormalized relations
  participantId: z.string().uuid(),
  participantName: z.string().nullable(),
  participantJid: z.string().nullable(),
  groupId: z.string().uuid(),
  groupName: z.string().nullable(),
  groupJid: z.string().nullable(),
  // Computed fields
  status: z.string(),
})

export type RawMessage = z.infer<typeof RawMessageSchema>

// Query parameters for raw messages API
export interface RawMessageParams {
  limit?: number
  offset?: number
  search?: string
  status?: ProcessingStatus
  sort_by?: SortField
  sort_order?: SortOrder
  start_date?: string
  end_date?: string
}
