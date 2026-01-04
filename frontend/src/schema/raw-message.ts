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

// Raw message schema matching Rust's RawMessageResponse
export const RawMessageSchema = z.object({
  id: z.string().uuid(),
  external_id: z.string().nullable(),
  content: z.string(),
  timestamp: z.string().datetime({ offset: true }),
  processed_at: z.string().datetime({ offset: true }).nullable(),
  error: z.string().nullable(),
  reply_to_id: z.string().nullable(),
  reply_to_content: z.string().nullable(),
  reply_to_sender: z.string().nullable(),
  created_at: z.string().datetime({ offset: true }),
  // Denormalized relations
  participant_id: z.string().uuid(),
  participant_name: z.string().nullable(),
  participant_jid: z.string(),
  group_id: z.string().uuid(),
  group_name: z.string().nullable(),
  group_jid: z.string(),
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
