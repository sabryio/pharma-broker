// Debug Recordings Types
// Core type definitions for the recording and pipeline analysis system

import type { MatchReviewItem } from '@/schema/match-review'

// ============================================================================
// Recording Event Types
// ============================================================================

export type RecordingEventType =
  | 'view'
  | 'approve'
  | 'reject'
  | 'restore'
  | 'adjust_price'
  | 'adjust_quantity'
  | 'adjust_dosage'
  | 'ai_review'
  | 'confidence_change'
  | 'navigate'
  | 'bulk_select'
  | 'bulk_action'
  | 'filter_change'
  | 'sort_change'

export interface RecordingEvent {
  type: RecordingEventType
  label: string
  description?: string
  data?: Record<string, unknown>
}

// ============================================================================
// Score Breakdown Types
// ============================================================================

export interface ScoreBreakdown {
  medicationSimilarity: number
  rawSimilarity: number
  embeddingSimilarity: number | null
  dosageMatch: number
  quantityMatch: number
  priceMatch: number | null
  recencyBonus: number
  aiLogicScore: number | null
  finalScore: number
}

export interface WeightConfig {
  medication: number
  raw: number
  embedding: number
  dosage: number
  quantity: number
  price: number
  recency: number
  aiLogic: number
}

// ============================================================================
// Recording Metadata
// ============================================================================

export interface RecordingMetadata {
  userAgent?: string
  sessionId?: string
  previousSnapshotId?: string | null
  scoreBreakdown?: ScoreBreakdown
  weights?: WeightConfig
}

// ============================================================================
// Adjustment Settings
// ============================================================================

export interface AdjustmentSettings {
  priceFlexibility: number
  quantityTolerance: number
  dosageStrictness: number
}

// ============================================================================
// Recording Snapshot
// ============================================================================

export interface MatchRecordingSnapshot {
  id: string
  timestamp: Date
  matchReview: MatchReviewItem
  offer: {
    id: string
    product: string
    medicationRaw?: string | null
    quantity: string | null
    price?: string | null
  }
  request: {
    id: string
    product: string
    medicationRaw?: string | null
    quantity: string | null
    maxPrice?: string | null
  }
  confidence: number
  aiStatus: string | null
  aiConfidence: number | null
  aiExplanation: string | null
  issues: string[]
  reasoning?: string | null
  adjustments: AdjustmentSettings
  event: RecordingEvent
  metadata: RecordingMetadata
}

// ============================================================================
// Match Recording
// ============================================================================

export interface MatchRecording {
  id: string
  matchId: string
  startedAt: Date
  endedAt?: Date
  duration?: number
  outcome?: 'approved' | 'rejected' | 'pending'
  snapshots: MatchRecordingSnapshot[]
}

// ============================================================================
// Event Colors and Icons
// ============================================================================

export const EVENT_COLORS: Record<
  RecordingEventType,
  { bg: string; text: string; border: string }
> = {
  view: {
    bg: 'bg-blue-500/20',
    text: 'text-blue-400',
    border: 'border-blue-500/30',
  },
  approve: {
    bg: 'bg-emerald-500/20',
    text: 'text-emerald-400',
    border: 'border-emerald-500/30',
  },
  reject: {
    bg: 'bg-red-500/20',
    text: 'text-red-400',
    border: 'border-red-500/30',
  },
  restore: {
    bg: 'bg-amber-500/20',
    text: 'text-amber-400',
    border: 'border-amber-500/30',
  },
  adjust_price: {
    bg: 'bg-violet-500/20',
    text: 'text-violet-400',
    border: 'border-violet-500/30',
  },
  adjust_quantity: {
    bg: 'bg-cyan-500/20',
    text: 'text-cyan-400',
    border: 'border-cyan-500/30',
  },
  adjust_dosage: {
    bg: 'bg-pink-500/20',
    text: 'text-pink-400',
    border: 'border-pink-500/30',
  },
  ai_review: {
    bg: 'bg-purple-500/20',
    text: 'text-purple-400',
    border: 'border-purple-500/30',
  },
  confidence_change: {
    bg: 'bg-orange-500/20',
    text: 'text-orange-400',
    border: 'border-orange-500/30',
  },
  navigate: {
    bg: 'bg-slate-500/20',
    text: 'text-slate-400',
    border: 'border-slate-500/30',
  },
  bulk_select: {
    bg: 'bg-indigo-500/20',
    text: 'text-indigo-400',
    border: 'border-indigo-500/30',
  },
  bulk_action: {
    bg: 'bg-teal-500/20',
    text: 'text-teal-400',
    border: 'border-teal-500/30',
  },
  filter_change: {
    bg: 'bg-lime-500/20',
    text: 'text-lime-400',
    border: 'border-lime-500/30',
  },
  sort_change: {
    bg: 'bg-rose-500/20',
    text: 'text-rose-400',
    border: 'border-rose-500/30',
  },
}

export const EVENT_ICONS: Record<RecordingEventType, string> = {
  view: '👁️',
  approve: '✅',
  reject: '❌',
  restore: '↩️',
  adjust_price: '💰',
  adjust_quantity: '📦',
  adjust_dosage: '💊',
  ai_review: '🤖',
  confidence_change: '📊',
  navigate: '🧭',
  bulk_select: '☑️',
  bulk_action: '⚡',
  filter_change: '🔍',
  sort_change: '↕️',
}
