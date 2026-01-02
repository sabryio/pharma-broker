import { z } from 'zod'

// Group schema matching Rust's Group entity
export const GroupSchema = z.object({
  id: z.string().uuid(),
  jid: z.string(),
  name: z.string(),
  description: z.string().nullable(),
  monitoring: z.boolean(),
  parsing: z.boolean(),
  added_at: z.string(), // ISO date string
  last_message: z.string().nullable(), // ISO date string
  message_count: z.number(),
  member_count: z.number(),
})

export type Group = z.infer<typeof GroupSchema>

// Response for list operations
export const GroupListResponseSchema = z.object({
  success: z.boolean(),
  groups: z.array(GroupSchema),
  total: z.number(),
})

export type GroupListResponse = z.infer<typeof GroupListResponseSchema>

// Response for single group operations
export const GroupResponseSchema = z.object({
  success: z.boolean(),
  group: GroupSchema.nullable(),
  error: z.string().nullable().optional(),
})

export type GroupResponse = z.infer<typeof GroupResponseSchema>

// Request body for creating a group
export interface CreateGroupRequest {
  jid: string
  name: string
  description?: string
  monitoring?: boolean
  parsing?: boolean
}

// Request body for updating a group
export interface UpdateGroupRequest {
  name?: string
  description?: string
  monitoring?: boolean
  parsing?: boolean
}
