import apiClient from './client'
import {
  SupervisionStatsSchema,
  ConfigResponseSchema,
  AuditLogResponseSchema,
  SuccessResponseSchema,
  type SupervisionStats,
  type ConfigResponse,
  type AuditLogResponse,
  type AuditQueryParams,
  type AutoApproveConfig,
  type OverrideRequest,
  type UndoRequest,
  type PauseRequest,
  type SuccessResponse,
} from '@/schema/supervision'

// =============================================================================
// API Functions for AI Supervision
// Requirements: 3.1, 3.2, 5.1
// =============================================================================

/**
 * Get auto-approve statistics
 * Requirements: 3.2
 */
export async function getSupervisionStats(): Promise<SupervisionStats> {
  const response = await apiClient.get<SupervisionStats>(
    '/api/supervision/stats',
  )
  return SupervisionStatsSchema.parse(response.data)
}

/**
 * Get auto-approve configuration with stats
 * Requirements: 5.1
 */
export async function getSupervisionConfig(): Promise<ConfigResponse> {
  const response = await apiClient.get<ConfigResponse>(
    '/api/supervision/config',
  )
  return ConfigResponseSchema.parse(response.data)
}

/**
 * Update auto-approve configuration
 * Requirements: 5.1
 */
export async function updateSupervisionConfig(
  config: AutoApproveConfig,
): Promise<SuccessResponse> {
  const response = await apiClient.put<SuccessResponse>(
    '/api/supervision/config',
    config,
  )
  return SuccessResponseSchema.parse(response.data)
}

/**
 * Get supervision audit log with filtering
 * Requirements: 2.3
 */
export async function getSupervisionAudit(
  params: AuditQueryParams = {},
): Promise<AuditLogResponse> {
  const queryParams: Record<string, string | number | boolean> = {}

  if (params.eventType) queryParams.event_type = params.eventType
  if (params.matchId) queryParams.match_id = params.matchId
  if (params.minConfidence !== undefined)
    queryParams.min_confidence = params.minConfidence
  if (params.maxConfidence !== undefined)
    queryParams.max_confidence = params.maxConfidence
  if (params.overridden !== undefined)
    queryParams.overridden = params.overridden
  if (params.startDate) queryParams.start_date = params.startDate
  if (params.endDate) queryParams.end_date = params.endDate
  if (params.limit !== undefined) queryParams.limit = params.limit
  if (params.offset !== undefined) queryParams.offset = params.offset

  const response = await apiClient.get<AuditLogResponse>(
    '/api/supervision/audit',
    { params: queryParams },
  )
  return AuditLogResponseSchema.parse(response.data)
}

/**
 * Override an AI auto-approval decision
 * Requirements: 4.1
 */
export async function overrideDecision(
  matchId: string,
  request: OverrideRequest,
): Promise<SuccessResponse> {
  const response = await apiClient.post<SuccessResponse>(
    `/api/supervision/override/${matchId}`,
    {
      user_id: request.userId,
      reason: request.reason,
    },
  )
  return SuccessResponseSchema.parse(response.data)
}

/**
 * Undo an auto-approval within the undo window
 * Requirements: 4.2
 */
export async function undoApproval(
  matchId: string,
  request: UndoRequest,
): Promise<SuccessResponse> {
  const response = await apiClient.post<SuccessResponse>(
    `/api/supervision/undo/${matchId}`,
    {
      user_id: request.userId,
    },
  )
  return SuccessResponseSchema.parse(response.data)
}

/**
 * Pause the auto-approve system
 */
export async function pauseSystem(
  request: PauseRequest,
): Promise<SuccessResponse> {
  const response = await apiClient.post<SuccessResponse>(
    '/api/supervision/pause',
    {
      user_id: request.userId,
      reason: request.reason,
    },
  )
  return SuccessResponseSchema.parse(response.data)
}

/**
 * Resume the auto-approve system
 */
export async function resumeSystem(): Promise<SuccessResponse> {
  const response = await apiClient.post<SuccessResponse>(
    '/api/supervision/resume',
  )
  return SuccessResponseSchema.parse(response.data)
}
