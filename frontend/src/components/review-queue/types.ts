// Review Queue Types
// Shared interfaces for review queue components

export interface ReviewOffer {
  id?: string
  product: string
  medicationRaw: string | null
  source: string
  sourceGroup: string | null
  senderName: string | null
  senderJid: string | null
  rawMessage: string | null
  quantity: string
  price: string
  expiry: string
  masterId?: string | null
  medicationAliasId?: string | null
  curationStatus?: string | null
}

export interface ReviewRequest {
  id?: string
  product: string
  medicationRaw: string | null
  source: string
  sourceGroup: string | null
  senderName: string | null
  senderJid: string | null
  rawMessage: string | null
  quantity: string
  maxPrice: string
  urgency: 'Low' | 'Medium' | 'High'
  masterId?: string | null
  medicationAliasId?: string | null
  curationStatus?: string | null
}

export interface Review {
  id: number
  uuid?: string
  confidence: number
  offer: ReviewOffer
  request: ReviewRequest
  issues: string[]
  aiStatus?: 'Approved' | 'Flagged' | 'Rejected' | null
  aiConfidence?: number | null
  aiExplanation?: string | null
}

export interface AdjustmentSettings {
  priceFlexibility: number
  quantityTolerance: number
  dosageStrictness: number
}

export interface HistoryEntry {
  id: string
  reviewId: number
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
  matchId: number
  matchUuid: string
  confidence: number
  issues: string[]
  aiStatus?: 'Approved' | 'Flagged' | 'Rejected' | null
  aiConfidence?: number | null
  aiExplanation?: string | null
}

/** An offer with all its related request matches */
export interface OfferWithMatches {
  offer: ReviewOffer
  offerKey: string
  matches: Array<MatchEntry & { request: ReviewRequest }>
}

/** A request with all its related offer matches */
export interface RequestWithMatches {
  request: ReviewRequest
  requestKey: string
  matches: Array<MatchEntry & { offer: ReviewOffer }>
}

/** Group reviews by offer - returns array of offers with their matched requests */
export function groupByOffer(reviews: Review[]): OfferWithMatches[] {
  const map = new Map<string, OfferWithMatches>()

  for (const review of reviews) {
    // Use offer source as key (could be improved with actual offer ID)
    const key = review.offer.id || review.offer.source

    if (!map.has(key)) {
      map.set(key, {
        offer: review.offer,
        offerKey: key,
        matches: [],
      })
    }

    map.get(key)!.matches.push({
      matchId: review.id,
      matchUuid: review.uuid || String(review.id),
      confidence: review.confidence,
      issues: review.issues,
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
    // Use request source as key
    const key = review.request.id || review.request.source

    if (!map.has(key)) {
      map.set(key, {
        request: review.request,
        requestKey: key,
        matches: [],
      })
    }

    map.get(key)!.matches.push({
      matchId: review.id,
      matchUuid: review.uuid || String(review.id),
      confidence: review.confidence,
      issues: review.issues,
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
