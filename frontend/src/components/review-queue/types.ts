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
  reasoning?: string | null
  aiConfidence?: number | null
  matchDetails?: MatchDetails | null
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
      reasoning: review.reasoning,
      request: review.request,
      aiConfidence: review.aiConfidence,
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
      reasoning: review.reasoning,
      offer: review.offer,
      aiConfidence: review.aiConfidence,
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

// ============================================
// Structured AI Match Details Types
// ============================================

/** Comparison result for a single field (brand name, Arabic name, etc.) */
export interface MatchField {
  /** Value from the offer */
  offerValue: string
  /** Value from the request */
  requestValue: string
  /** Whether the fields match */
  matches: boolean
  /** Type of match: "exact", "transliteration", "fuzzy", "partial", "no_match" */
  matchType: string
  /** Similarity score (0.0 - 1.0) */
  similarity?: number
}

/** Dosage comparison details */
export interface DosageComparison {
  /** Dosage from the offer (e.g., "10.8mg") */
  offerDosage?: string | null
  /** Dosage from the request (e.g., "3.6mg") */
  requestDosage?: string | null
  /** Whether dosages match */
  matches: boolean
  /** Whether dosage difference is being ignored per matching rules */
  ignored: boolean
  /** Explanation note */
  note?: string
}

/** Structured match details providing granular analysis breakdown */
export interface MatchDetails {
  /** Brand name comparison result */
  brandMatch: MatchField
  /** Arabic/transliteration name comparison */
  arabicMatch: MatchField
  /** Dosage comparison (may be ignored per rules) */
  dosage: DosageComparison
  /** Generic/active ingredient match (if applicable) */
  genericMatch?: MatchField | null
  /** Key differences found between offer and request */
  differences?: string[]
  /** Reasons supporting the final decision */
  decisionReasons?: string[]
}

/** Helper to get match type color */
export function getMatchTypeColor(matchType: string): string {
  switch (matchType) {
    case 'exact':
      return 'text-emerald'
    case 'transliteration':
      return 'text-teal'
    case 'fuzzy':
    case 'partial':
      return 'text-amber'
    case 'no_match':
      return 'text-destructive'
    default:
      return 'text-muted-foreground'
  }
}

/** Helper to get match status icon based on match type */
export function getMatchStatusIcon(
  matches: boolean,
): 'check' | 'warning' | 'x' {
  return matches ? 'check' : 'x'
}
