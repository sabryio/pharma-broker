import { z } from 'zod'

// =============================================================================
// Zod Schemas for AI Supervision
// =============================================================================

export const SystemStatusSchema = z.enum(['active', 'paused', 'disabled'])

export const SupervisionStatsSchema = z.object({
  totalApprovedToday: z.number(),
  totalQueuedToday: z.number(),
  totalBlockedToday: z.number(),
  overrideRate: z.number(),
  averageConfidence: z.number(),
  pendingReviewCount: z.number(),
  systemStatus: SystemStatusSchema,
  pauseReason: z.string().nullable().optional(),
})

export const AutoApproveConfigSchema = z.object({
  enabled: z.boolean(),
  confidenceThreshold: z.number(),
  batchSize: z.number(),
  processingIntervalSecs: z.number(),
  undoWindowMins: z.number(),
  overrideRatePauseThreshold: z.number(),
  consecutiveOverrideLimit: z.number(),
  overrideCooldownMins: z.number(),
  categoryThresholds: z.record(z.string(), z.number()),
  schedule: z.string().nullable().optional(),
})

export const ConfigResponseSchema = z.object({
  config: AutoApproveConfigSchema,
  stats: SupervisionStatsSchema,
})

export const SupervisionEventTypeSchema = z.enum([
  'AutoApproved',
  'QueuedForReview',
  'Blocked',
  'Overridden',
  'UndoApproval',
  'ConfigChanged',
  'SystemPaused',
  'SystemResumed',
])

export const AuditEntrySchema = z.object({
  id: z.string().uuid(),
  matchId: z.string().uuid().nullable().optional(),
  timestamp: z.string(),
  eventType: z.string(),
  aiConfidence: z.number().nullable().optional(),
  aiExplanation: z.string().nullable().optional(),
  decision: z.string().nullable().optional(),
  overridden: z.boolean(),
  overrideBy: z.string().uuid().nullable().optional(),
  overrideReason: z.string().nullable().optional(),
  overrideAt: z.string().nullable().optional(),
})

export const AuditLogResponseSchema = z.object({
  entries: z.array(AuditEntrySchema),
  total: z.number(),
})

export const SuccessResponseSchema = z.object({
  success: z.boolean(),
  message: z.string(),
})

// =============================================================================
// Inferred Types
// =============================================================================

export type SystemStatus = z.infer<typeof SystemStatusSchema>
export type SupervisionStats = z.infer<typeof SupervisionStatsSchema>
export type AutoApproveConfig = z.infer<typeof AutoApproveConfigSchema>
export type ConfigResponse = z.infer<typeof ConfigResponseSchema>
export type SupervisionEventType = z.infer<typeof SupervisionEventTypeSchema>
export type AuditEntry = z.infer<typeof AuditEntrySchema>
export type AuditLogResponse = z.infer<typeof AuditLogResponseSchema>
export type SuccessResponse = z.infer<typeof SuccessResponseSchema>

// =============================================================================
// Request Types
// =============================================================================

export interface AuditQueryParams {
  eventType?: string
  matchId?: string
  minConfidence?: number
  maxConfidence?: number
  overridden?: boolean
  startDate?: string
  endDate?: string
  limit?: number
  offset?: number
}

export interface OverrideRequest {
  userId: string
  reason: string
}

export interface UndoRequest {
  userId: string
}

export interface PauseRequest {
  userId: string
  reason: string
}

// =============================================================================
// WebSocket Event Types (for real-time updates)
// =============================================================================

export interface AutoApproveEvent {
  matchId: string
  offerMedication: string
  requestMedication: string
  aiConfidence: number
  aiExplanation: string
  isBorderline: boolean
  approvedAt: string
}

export interface AutoApproveOverrideEvent {
  matchId: string
  userId: string
  reason: string
  originalConfidence: number
  overriddenAt: string
}

export interface AutoApproveUndoEvent {
  matchId: string
  userId: string
  undoneAt: string
}

export interface AutoApprovePauseEvent {
  userId: string | null
  reason: string
  pausedAt: string
}

export interface QueuedForReviewEvent {
  matchId: string
  offerMedication: string
  requestMedication: string
  aiConfidence: number
  aiExplanation: string
  isBorderline: boolean
  queuedAt: string
}

export interface AutoApproveBlockedEvent {
  matchId: string
  offerMedication: string
  requestMedication: string
  blockReason: string
  blockedAt: string
}

// Live feed item for the dashboard
export interface LiveFeedItem {
  id: string
  matchId: string
  timestamp: string
  action: 'approved' | 'queued' | 'blocked'
  aiConfidence: number
  aiExplanation: string
  offerMedication: string
  requestMedication: string
  isBorderline: boolean
  blockReason?: string
}
