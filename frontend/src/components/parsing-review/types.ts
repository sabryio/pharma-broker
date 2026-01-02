// AI Parsing Review Types
// Shared interfaces for parsing review components

export interface ParsedOffer {
  type: 'offer'
  medication: string
  quantity?: string
  price?: string
  expiry?: string
  batchNumber?: string
  notes?: string
}

export interface ParsedRequest {
  type: 'request'
  medication: string
  quantity?: string
  maxPrice?: string
  urgency?: 'low' | 'medium' | 'high'
  notes?: string
}

export type ParsedResult = ParsedOffer | ParsedRequest

export interface ParsingReviewItem {
  id: string
  rawMessageId: string
  originalText: string
  senderName: string
  senderPhone?: string
  groupName: string
  timestamp: Date
  aiResult: ParsedResult
  confidence: number
  reason: string
  status: 'Pending' | 'Approved' | 'Rejected' | 'Skipped'
  reviewedBy?: string
  reviewNotes?: string
  reviewedAt?: Date
}

export interface ParsingStats {
  pending: number
  approved: number
  rejected: number
  skipped: number
  avgConfidence: number
  todayReviewed: number
}

// Helper to get confidence color
export function getParsingConfidenceColor(confidence: number): string {
  if (confidence >= 0.8) return 'text-emerald'
  if (confidence >= 0.6) return 'text-amber'
  return 'text-destructive'
}

// Helper to get confidence label
export function getConfidenceLabel(confidence: number): string {
  if (confidence >= 0.8) return 'High'
  if (confidence >= 0.6) return 'Medium'
  if (confidence >= 0.4) return 'Low'
  return 'Very Low'
}
