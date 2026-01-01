// Review Queue Types
// Shared interfaces for review queue components

export interface ReviewOffer {
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
}

export interface ReviewRequest {
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
}

export interface Review {
  id: number
  confidence: number
  offer: ReviewOffer
  request: ReviewRequest
  issues: string[]
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
