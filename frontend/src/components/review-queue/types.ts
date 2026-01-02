// Review Queue Types
// Re-export Zod types and add helper functions

import type {
  MatchReviewItem,
  OfferSummary,
  RequestSummary,
} from '@/schema/match-review'

// Re-export types from schema
export type ReviewOffer = OfferSummary
export type ReviewRequest = RequestSummary
export type Review = MatchReviewItem

export interface AdjustmentSettings {
  priceFlexibility: number
  quantityTolerance: number
  dosageStrictness: number
}

export interface HistoryEntry {
  id: string
  reviewId: string
  product: string
  action: 'approved' | 'rejected'
  timestamp: Date
  confidence: number
  adjustments: AdjustmentSettings
  originalReview: Review
}

// ============================================
// Anchor-based Carousel Types
// ============================================

/** A match entry for carousel navigation */
export interface MatchEntry {
  matchId: string
  confidence: number
  issues: string[]
  notes?: string | null
  aiStatus?: string | null
  aiConfidence?: number | null
  aiExplanation?: string | null
}

/** An offer with all its related request matches */
export interface OfferWithMatches {
  offer: ReviewOffer
  matches: Array<MatchEntry & { request: ReviewRequest }>
}

/** A request with all its related offer matches */
export interface RequestWithMatches {
  request: ReviewRequest
  matches: Array<MatchEntry & { offer: ReviewOffer }>
}

/** Group reviews by offer - returns array of offers with their matched requests */
export function groupByOffer(reviews: Review[]): OfferWithMatches[] {
  const map = new Map<string, OfferWithMatches>()

  for (const review of reviews) {
    const key = review.offer.id

    if (!map.has(key)) {
      map.set(key, {
        offer: review.offer,
        matches: [],
      })
    }

    map.get(key)!.matches.push({
      matchId: review.id,
      confidence: review.confidence,
      issues: review.issues,
      notes: review.notes,
      request: review.request,
      aiStatus: review.aiStatus,
      aiConfidence: review.aiConfidence,
      aiExplanation: review.aiExplanation,
    })
  }

  // Sort matches within each offer by confidence (highest first)
  for (const group of map.values()) {
    group.matches.sort((a, b) => b.confidence - a.confidence)
  }

  return Array.from(map.values())
}

/** Group reviews by request - returns array of requests with their matched offers */
export function groupByRequest(reviews: Review[]): RequestWithMatches[] {
  const map = new Map<string, RequestWithMatches>()

  for (const review of reviews) {
    const key = review.request.id

    if (!map.has(key)) {
      map.set(key, {
        request: review.request,
        matches: [],
      })
    }

    map.get(key)!.matches.push({
      matchId: review.id,
      confidence: review.confidence,
      issues: review.issues,
      notes: review.notes,
      offer: review.offer,
      aiStatus: review.aiStatus,
      aiConfidence: review.aiConfidence,
      aiExplanation: review.aiExplanation,
    })
  }

  // Sort matches within each request by confidence (highest first)
  for (const group of map.values()) {
    group.matches.sort((a, b) => b.confidence - a.confidence)
  }

  return Array.from(map.values())
}

// Helper to get confidence color class
export function getConfidenceColor(confidence: number): string {
  if (confidence >= 80) return 'text-emerald'
  if (confidence >= 60) return 'text-amber'
  return 'text-destructive'
}

// Default adjustment settings
export const defaultAdjustments: AdjustmentSettings = {
  priceFlexibility: 10,
  quantityTolerance: 15,
  dosageStrictness: 80,
}
