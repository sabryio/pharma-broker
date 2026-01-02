// Audit Records Hooks
// TanStack React Query hooks for audit record management

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { queryKeys } from './query-keys'

const API_BASE = '/api'

// =============================================================================
// Types
// =============================================================================

export interface PipelineStageSummary {
  stage: string
  durationMs: number
  candidatesOut: number
}

export interface FrontendAuditRecord {
  id: string
  matchId: string
  offerProduct: string
  requestProduct: string
  finalScore: number
  resolutionStage: string
  aiInvolved: boolean
  totalLatencyMs: number
  createdAt: string
  reviewStatus: string | null
  pipelineSummary: PipelineStageSummary[]
}

export interface AuditRecordsResponse {
  records: FrontendAuditRecord[]
  total: number
}

export interface AuditRecordDetail {
  record: {
    id: string
    matchId: string
    offerId: string
    requestId: string
    pipelineVersion: string
    offerSnapshot: Record<string, unknown>
    requestSnapshot: Record<string, unknown>
    weightsSnapshot: Record<string, unknown>
    configSnapshot: Record<string, unknown> | null
    scoreBreakdown: Record<string, unknown>
    finalScore: number
    pipelineStages: Array<{
      stage: string
      startedAt: string
      durationMs: number
      candidatesIn: number
      candidatesOut: number
      details: Record<string, unknown> | null
    }>
    aiInvolved: boolean
    aiRecord: {
      model: string
      promptTokens: number | null
      completionTokens: number | null
      latencyMs: number
      response: Record<string, unknown>
    } | null
    resolutionStage: string
    resolutionDetails: Record<string, unknown> | null
    totalLatencyMs: number
    createdAt: string
    reviewStatus: string | null
    reviewedBy: string | null
    reviewedAt: string | null
    reviewNotes: string | null
    sessionId: string | null
    clientMetadata: Record<string, unknown> | null
  }
  replayContext: {
    offer: Record<string, unknown>
    request: Record<string, unknown>
    weights: Record<string, unknown>
  } | null
}

export interface AuditRecorderStatus {
  enabled: boolean
  bufferSize: number
  currentBufferLen: number
  stats: {
    recordsCreated: number
    recordsPersisted: number
    recordsSampledOut: number
    recordsFilteredByScore: number
    persistErrors: number
  }
  config: {
    bufferSize: number
    persistToDb: boolean
    minScoreThreshold: number | null
    sampleRate: number
  }
}

export interface ListAuditRecordsParams {
  limit?: number
  sessionId?: string
  minScore?: number
  aiInvolved?: boolean
}

export interface UpdateReviewRequest {
  status: string
  notes?: string
}

// =============================================================================
// API Functions
// =============================================================================

async function fetchAuditRecords(
  params: ListAuditRecordsParams,
): Promise<AuditRecordsResponse> {
  const searchParams = new URLSearchParams()
  if (params.limit) searchParams.set('limit', params.limit.toString())
  if (params.sessionId) searchParams.set('session_id', params.sessionId)
  if (params.minScore) searchParams.set('min_score', params.minScore.toString())
  if (params.aiInvolved !== undefined)
    searchParams.set('ai_involved', params.aiInvolved.toString())

  const url = `${API_BASE}/audit-records?${searchParams.toString()}`
  const response = await fetch(url)
  if (!response.ok) {
    throw new Error(`Failed to fetch audit records: ${response.statusText}`)
  }
  return response.json()
}

async function fetchAuditRecord(matchId: string): Promise<AuditRecordDetail> {
  const response = await fetch(`${API_BASE}/audit-records/${matchId}`)
  if (!response.ok) {
    throw new Error(`Failed to fetch audit record: ${response.statusText}`)
  }
  return response.json()
}

async function fetchSessionRecords(
  sessionId: string,
): Promise<AuditRecordsResponse> {
  const response = await fetch(`${API_BASE}/audit-records/session/${sessionId}`)
  if (!response.ok) {
    throw new Error(`Failed to fetch session records: ${response.statusText}`)
  }
  return response.json()
}

async function fetchAuditRecorderStatus(): Promise<AuditRecorderStatus> {
  const response = await fetch(`${API_BASE}/audit-records/status`)
  if (!response.ok) {
    throw new Error(`Failed to fetch recorder status: ${response.statusText}`)
  }
  return response.json()
}

async function updateAuditReview(
  matchId: string,
  data: UpdateReviewRequest,
): Promise<{ success: boolean; message: string }> {
  const response = await fetch(`${API_BASE}/audit-records/${matchId}/review`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  if (!response.ok) {
    throw new Error(`Failed to update review: ${response.statusText}`)
  }
  return response.json()
}

// =============================================================================
// Hooks
// =============================================================================

/** List audit records with optional filters */
export function useAuditRecords(params: ListAuditRecordsParams = {}) {
  return useQuery({
    queryKey: queryKeys.auditRecords.list(params),
    queryFn: () => fetchAuditRecords(params),
  })
}

/** Get a single audit record by match ID */
export function useAuditRecord(matchId: string | undefined) {
  return useQuery({
    queryKey: queryKeys.auditRecords.detail(matchId ?? ''),
    queryFn: () => fetchAuditRecord(matchId!),
    enabled: !!matchId,
  })
}

/** Get audit records for a specific session */
export function useSessionAuditRecords(sessionId: string | undefined) {
  return useQuery({
    queryKey: queryKeys.auditRecords.session(sessionId ?? ''),
    queryFn: () => fetchSessionRecords(sessionId!),
    enabled: !!sessionId,
  })
}

/** Get audit recorder status */
export function useAuditRecorderStatus() {
  return useQuery({
    queryKey: queryKeys.auditRecords.status(),
    queryFn: fetchAuditRecorderStatus,
  })
}

/** Update review status for an audit record */
export function useUpdateAuditReview() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({
      matchId,
      data,
    }: {
      matchId: string
      data: UpdateReviewRequest
    }) => updateAuditReview(matchId, data),
    onSuccess: (_, { matchId }) => {
      // Invalidate related queries
      queryClient.invalidateQueries({
        queryKey: queryKeys.auditRecords.detail(matchId),
      })
      queryClient.invalidateQueries({
        queryKey: queryKeys.auditRecords.lists(),
      })
    },
  })
}
