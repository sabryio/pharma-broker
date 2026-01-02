import apiClient from './client'
import {
  MatchReviewItemSchema,
  MatchReviewListResponseSchema,
  MatchReviewStatsSchema,
  UpdateMatchReviewRequestSchema,
  UpdateMatchReviewResponseSchema,
  BulkUpdateResponseSchema,
  type MatchReviewItem,
  type MatchReviewListResponse,
  type MatchReviewParams,
  type MatchReviewStats,
  type UpdateMatchReviewRequest,
  type UpdateMatchReviewResponse,
  type BulkUpdateRequest,
  type BulkUpdateResponse,
} from '@/schema/match-review'

// ============================================================================
// API Functions
// ============================================================================

/**
 * Fetch paginated match reviews
 */
export async function getMatchReviews(
  params: MatchReviewParams = {},
): Promise<MatchReviewListResponse> {
  const { limit = 20, offset = 0, status, minScore } = params

  const response = await apiClient.get<MatchReviewListResponse>(
    '/api/match-reviews',
    { params: { limit, offset, status, min_score: minScore } },
  )

  return MatchReviewListResponseSchema.parse(response.data)
}

/**
 * Fetch a single match review by ID
 */
export async function getMatchReview(id: string): Promise<MatchReviewItem> {
  const response = await apiClient.get<MatchReviewItem>(
    `/api/match-reviews/${id}`,
  )
  return MatchReviewItemSchema.parse(response.data)
}

/**
 * Update match review status (approve/reject)
 */
export async function updateMatchReviewStatus(
  id: string,
  data: UpdateMatchReviewRequest,
): Promise<UpdateMatchReviewResponse> {
  console.log('[DEBUG] updateMatchReviewStatus called:', { id, data })
  const validated = UpdateMatchReviewRequestSchema.parse(data)
  console.log('[DEBUG] Validated request:', validated)

  try {
    const response = await apiClient.put<UpdateMatchReviewResponse>(
      `/api/match-reviews/${id}/status`,
      validated,
    )
    console.log('[DEBUG] API response:', response.data)
    return UpdateMatchReviewResponseSchema.parse(response.data)
  } catch (error) {
    console.error('[DEBUG] API error:', error)
    throw error
  }
}

/**
 * Bulk update match reviews
 */
export async function bulkUpdateMatchReviews(
  data: BulkUpdateRequest,
): Promise<BulkUpdateResponse> {
  const response = await apiClient.post<BulkUpdateResponse>(
    '/api/match-reviews/bulk',
    data,
  )
  return BulkUpdateResponseSchema.parse(response.data)
}

/**
 * Fetch match review statistics
 */
export async function getMatchReviewStats(): Promise<MatchReviewStats> {
  const response = await apiClient.get<MatchReviewStats>(
    '/api/match-reviews/stats',
  )
  return MatchReviewStatsSchema.parse(response.data)
}

/**
 * Re-audit response type
 */
export interface ReAuditResponse {
  success: boolean
  matchId: string
  aiStatus: string | null
  aiConfidence: number | null
  aiExplanation: string | null
  suggestedAction: string | null
}

/**
 * Re-trigger AI audit for a match
 */
export async function reAuditMatch(id: string): Promise<ReAuditResponse> {
  const response = await apiClient.post<ReAuditResponse>(
    `/api/match-reviews/${id}/re-audit`,
  )
  return response.data
}

/**
 * Recalculate confidence response type
 */
export interface RecalculateConfidenceResponse {
  success: boolean
  matchId: string
  oldScore: number
  newScore: number
  medicationSimilarity: number
  rawSimilarity: number
  embeddingSimilarity: number | null
  reasoning: string
}

/**
 * Recalculate match confidence score
 */
export async function recalculateConfidence(
  id: string,
): Promise<RecalculateConfidenceResponse> {
  const response = await apiClient.post<RecalculateConfidenceResponse>(
    `/api/match-reviews/${id}/recalculate`,
  )
  return response.data
}
