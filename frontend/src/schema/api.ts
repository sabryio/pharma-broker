import { z } from 'zod'

// Pagination meta from API response
export const PaginationMetaSchema = z.object({
  total: z.number(),
  limit: z.number(),
  offset: z.number(),
})

export type PaginationMeta = z.infer<typeof PaginationMetaSchema>

// Generic API response wrapper matching Rust's ApiResponse<T>
export const createApiResponseSchema = <T extends z.ZodTypeAny>(
  dataSchema: T,
) =>
  z.object({
    success: z.boolean(),
    data: dataSchema.nullable(),
    error: z.string().nullable().optional(),
    meta: PaginationMetaSchema.nullable().optional(),
  })

export type ApiResponse<T> = {
  success: boolean
  data: T | null
  error?: string | null
  meta?: PaginationMeta | null
}

// Pagination query params
export interface PaginationParams {
  limit?: number
  offset?: number
}
