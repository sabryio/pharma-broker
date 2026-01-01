import apiClient from './client'
import type {
  Review,
  ReviewOffer,
  ReviewRequest,
} from '@/components/review-queue/types'
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
  const validated = UpdateMatchReviewRequestSchema.parse(data)
  const response = await apiClient.put<UpdateMatchReviewResponse>(
    `/api/match-reviews/${id}/status`,
    validated,
  )
  return UpdateMatchReviewResponseSchema.parse(response.data)
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

// ============================================================================
// Transform Functions
// ============================================================================

/**
 * Transform API MatchReviewItem to frontend Review format
 */
export function transformToReview(item: MatchReviewItem): Review {
  const offer: ReviewOffer = {
    product: item.offer.product,
    medicationRaw: item.offer.medicationRaw,
    source: item.offer.source,
    sourceGroup: item.offer.sourceGroup,
    senderName: item.offer.senderName,
    senderJid: item.offer.senderJid,
    rawMessage: item.offer.rawMessage,
    quantity: item.offer.quantity ?? 'N/A',
    price: item.offer.price ?? 'N/A',
    expiry: item.offer.expiry ?? 'N/A',
  }

  const request: ReviewRequest = {
    product: item.request.product,
    medicationRaw: item.request.medicationRaw,
    source: item.request.source,
    sourceGroup: item.request.sourceGroup,
    senderName: item.request.senderName,
    senderJid: item.request.senderJid,
    rawMessage: item.request.rawMessage,
    quantity: item.request.quantity ?? 'N/A',
    maxPrice: item.request.maxPrice ?? 'N/A',
    urgency: mapUrgency(item.request.urgency),
  }

  return {
    id: hashUuid(item.id), // Convert UUID to number for frontend compatibility
    confidence: Math.round(item.confidence),
    offer,
    request,
    issues: item.issues.length > 0 ? item.issues : ['No issues detected'],
  }
}

/**
 * Map urgency string to expected format
 */
function mapUrgency(urgency: string): 'Low' | 'Medium' | 'High' {
  const lower = urgency.toLowerCase()
  if (
    lower.includes('critical') ||
    lower.includes('urgent') ||
    lower.includes('high')
  ) {
    return 'High'
  }
  if (lower.includes('normal') || lower.includes('medium')) {
    return 'Medium'
  }
  return 'Low'
}

/**
 * Simple hash function to convert UUID to number
 */
function hashUuid(uuid: string): number {
  let hash = 0
  for (let i = 0; i < uuid.length; i++) {
    const char = uuid.charCodeAt(i)
    hash = (hash << 5) - hash + char
    hash = hash & hash // Convert to 32bit integer
  }
  return Math.abs(hash)
}

/**
 * Store UUID mapping for reverse lookup
 */
const uuidMap = new Map<number, string>()

export function transformToReviewWithMapping(
  item: MatchReviewItem,
): Review & { uuid: string } {
  const review = transformToReview(item)
  uuidMap.set(review.id, item.id)
  return { ...review, uuid: item.id }
}

export function getUuidFromReviewId(reviewId: number): string | undefined {
  return uuidMap.get(reviewId)
}
