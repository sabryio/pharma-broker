import apiClient from './client'
import type {
  ParsingReviewItem,
  ParsingStats,
} from '@/components/parsing-review/types'
import {
  ApiReviewItemSchema,
  ReviewQueueListResponseSchema,
  ReviewQueueStatsSchema,
  UpdateReviewStatusRequestSchema,
  UpdateReviewStatusResponseSchema,
  type ApiReviewItem,
  type ReviewQueueListResponse,
  type ReviewQueueParams,
  type ReviewQueueStats,
  type UpdateReviewStatusRequest,
  type UpdateReviewStatusResponse,
} from '@/schema/review-queue'

// ============================================================================
// API Functions
// ============================================================================

/**
 * Fetch paginated review queue items
 */
export async function getReviewQueueItems(
  params: ReviewQueueParams = {},
): Promise<ReviewQueueListResponse> {
  const { limit = 20, offset = 0, status = 'pending' } = params

  const response = await apiClient.get<ReviewQueueListResponse>(
    '/api/review-queue',
    { params: { limit, offset, status } },
  )

  return ReviewQueueListResponseSchema.parse(response.data)
}

/**
 * Fetch a single review queue item by ID
 */
export async function getReviewQueueItem(id: string): Promise<ApiReviewItem> {
  const response = await apiClient.get<ApiReviewItem>(`/api/review-queue/${id}`)
  return ApiReviewItemSchema.parse(response.data)
}

/**
 * Update review queue item status
 */
export async function updateReviewStatus(
  id: string,
  data: UpdateReviewStatusRequest,
): Promise<UpdateReviewStatusResponse> {
  const validated = UpdateReviewStatusRequestSchema.parse(data)
  const response = await apiClient.put<UpdateReviewStatusResponse>(
    `/api/review-queue/${id}/status`,
    validated,
  )
  return UpdateReviewStatusResponseSchema.parse(response.data)
}

/**
 * Fetch review queue statistics
 */
export async function getReviewQueueStats(): Promise<ReviewQueueStats> {
  const response = await apiClient.get<ReviewQueueStats>(
    '/api/review-queue/stats',
  )
  return ReviewQueueStatsSchema.parse(response.data)
}

// ============================================================================
// Transform Functions
// ============================================================================

/**
 * Transform API response to frontend ParsingReviewItem format
 */
export function transformToParsingReviewItem(
  item: ApiReviewItem,
): ParsingReviewItem {
  const aiResult = item.aiResult as Record<string, unknown>

  // Determine if it's an offer or request based on aiResult
  const isOffer =
    aiResult.type === 'offer' || 'price' in aiResult || 'expiry' in aiResult

  return {
    id: item.id,
    rawMessageId: item.rawMessageId,
    originalText: item.originalText,
    senderName: item.senderName ?? 'Unknown Sender',
    senderPhone: item.senderPhone ?? undefined,
    groupName: item.groupName ?? 'Unknown Group',
    timestamp: new Date(item.messageTimestamp),
    aiResult: isOffer
      ? {
          type: 'offer',
          medication: (aiResult.medication as string) ?? '',
          quantity: aiResult.quantity as string | undefined,
          price: aiResult.price as string | undefined,
          expiry: aiResult.expiry as string | undefined,
          batchNumber: aiResult.batchNumber as string | undefined,
          notes: aiResult.notes as string | undefined,
        }
      : {
          type: 'request',
          medication: (aiResult.medication as string) ?? '',
          quantity: aiResult.quantity as string | undefined,
          maxPrice: aiResult.maxPrice as string | undefined,
          urgency: aiResult.urgency as 'low' | 'medium' | 'high' | undefined,
          notes: aiResult.notes as string | undefined,
        },
    confidence: item.confidence,
    reason: item.reason,
    status: item.status,
    reviewedBy: item.reviewedBy ?? undefined,
    reviewNotes: item.reviewNotes ?? undefined,
    reviewedAt: item.reviewedAt ? new Date(item.reviewedAt) : undefined,
  }
}

/**
 * Transform API stats to frontend ParsingStats format
 */
export function transformToParsingStats(
  stats: ReviewQueueStats,
  avgConfidence: number = 0.68,
  todayReviewed: number = 0,
): ParsingStats {
  return {
    pending: stats.pending,
    approved: stats.approved,
    rejected: stats.rejected,
    skipped: stats.skipped,
    avgConfidence,
    todayReviewed,
  }
}
