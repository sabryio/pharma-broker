// Audit Records Hooks
// TanStack React Query hooks for audit record management

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { queryKeys } from './query-keys'

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8082'

// =============================================================================
// Types
// =============================================================================

export interface PipelineStageSummary {
  stage: string
  durationMs: number
  candidatesOut: number
}

// =============================================================================
// Pipeline Visualization Types (Requirements 4.1, 4.2, 4.3, 4.4, 4.5)
// =============================================================================

export interface PipelineStageVisualization {
  stageType: string
  stageName: string
  status: string
  startedAt: string
  completedAt?: string
  durationMs: number
  candidatesIn: number
  candidatesOut: number
  involvesAi: boolean
  details: Record<string, unknown>
}

export interface CandidateVisualization {
  id: string
  score: number
  passed: boolean
  metadata?: Record<string, unknown>
}

export interface HierarchicalStageVisualization {
  stageNumber: number
  stageName: string
  threshold: number
  durationMs: number
  candidatesIn: number
  candidatesOut: number
  hasMatches: boolean
  candidates: CandidateVisualization[]
}

export interface ScoreComponentVisualization {
  name: string
  rawScore: number
  weight: number
  weightedScore: number
  contributionPercent: number
}

export interface ScoreBreakdownVisualization {
  finalScore: number
  formula: string
  components: ScoreComponentVisualization[]
  totalWeight: number
}

export interface TokenUsageVisualization {
  promptTokens: number
  completionTokens: number
  totalTokens: number
}

export interface AiReviewVisualization {
  modelName: string
  decision: string
  confidence: number
  reasoning?: string
  latencyMs: number
  tokenUsage?: TokenUsageVisualization
}

export interface ResolutionStageVisualization {
  stage: string
  matched: boolean
  candidatesFound: number
  bestScore?: number
  durationMs: number
}

export interface ResolutionVisualization {
  resolutionStage: string
  masterId?: string
  aliasId?: string
  similarityScore?: number
  embeddingDistance?: number
  stageResults: ResolutionStageVisualization[]
}

export interface ModelResultVisualization {
  modelId: string
  status?: string
  confidence?: number
  durationMs: number
  error?: string
}

export interface ConsensusVisualization {
  status: string
  confidence: number
  agreementRatio: number
  agreeingModels: number
  totalModels: number
  consensusReached: boolean
  explanation?: string
  modelResults: ModelResultVisualization[]
}

export interface ContrastiveVisualization {
  valid: boolean
  positiveScore: number
  avgNegativeScore: number
  maxNegativeScore: number
  marginVsAvg: number
  marginVsMax: number
  numNegatives: number
  reason: string
}

export interface CalibrationVisualization {
  rawScore: number
  calibratedScore: number
  method: string
  adjustment: number
  calibrationApplied: boolean
  ece?: number
  binIndex?: number
}

export interface PerformanceMetricsVisualization {
  memoryPeakBytes?: number
  aiQueueWaitMs?: number
  aiProcessingMs?: number
  totalAiTimeMs?: number
  dbQueryCount: number
  dbTotalMs: number
  stageLatencies: Record<string, number>
}

export interface PipelineVisualizationResponse {
  matchId: string
  offerId: string
  requestId: string
  pipelineVersion: string
  finalScore: number
  totalLatencyMs: number
  aiInvolved: boolean
  resolutionStage: string
  stages: PipelineStageVisualization[]
  hierarchicalDetails?: HierarchicalStageVisualization[]
  scoreBreakdown?: ScoreBreakdownVisualization
  aiReview?: AiReviewVisualization
  resolution?: ResolutionVisualization
  consensus?: ConsensusVisualization
  contrastive?: ContrastiveVisualization
  calibration?: CalibrationVisualization
  performanceMetrics: PerformanceMetricsVisualization
  createdAt: string
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

  const url = `${API_BASE}/api/audit-records?${searchParams.toString()}`
  const response = await fetch(url)
  if (!response.ok) {
    throw new Error(`Failed to fetch audit records: ${response.statusText}`)
  }
  return response.json()
}

async function fetchAuditRecord(matchId: string): Promise<AuditRecordDetail> {
  const response = await fetch(`${API_BASE}/api/audit-records/${matchId}`)
  if (!response.ok) {
    throw new Error(`Failed to fetch audit record: ${response.statusText}`)
  }
  return response.json()
}

async function fetchSessionRecords(
  sessionId: string,
): Promise<AuditRecordsResponse> {
  const response = await fetch(
    `${API_BASE}/api/audit-records/session/${sessionId}`,
  )
  if (!response.ok) {
    throw new Error(`Failed to fetch session records: ${response.statusText}`)
  }
  return response.json()
}

async function fetchAuditRecorderStatus(): Promise<AuditRecorderStatus> {
  const response = await fetch(`${API_BASE}/api/audit-records/status`)
  if (!response.ok) {
    throw new Error(`Failed to fetch recorder status: ${response.statusText}`)
  }
  return response.json()
}

async function updateAuditReview(
  matchId: string,
  data: UpdateReviewRequest,
): Promise<{ success: boolean; message: string }> {
  const response = await fetch(
    `${API_BASE}/api/audit-records/${matchId}/review`,
    {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    },
  )
  if (!response.ok) {
    throw new Error(`Failed to update review: ${response.statusText}`)
  }
  return response.json()
}

async function fetchPipelineVisualization(
  matchId: string,
): Promise<PipelineVisualizationResponse> {
  const response = await fetch(
    `${API_BASE}/api/audit-records/${matchId}/pipeline`,
  )
  if (!response.ok) {
    throw new Error(
      `Failed to fetch pipeline visualization: ${response.statusText}`,
    )
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

/** Get pipeline visualization for a match (Requirements 4.1-4.5) */
export function usePipelineVisualization(matchId: string | undefined) {
  return useQuery({
    queryKey: queryKeys.auditRecords.pipeline(matchId ?? ''),
    queryFn: () => fetchPipelineVisualization(matchId!),
    enabled: !!matchId,
    staleTime: 30000, // Cache for 30 seconds
    retry: 2,
  })
}

// =============================================================================
// Performance Analytics Types (Requirements 8.4, 8.5)
// =============================================================================

export interface LatencyStats {
  count: number
  minMs: number
  maxMs: number
  avgMs: number
  medianMs: number
  p95Ms: number
  p99Ms: number
  stdDevMs: number
}

export interface AiMetrics {
  invocationCount: number
  queueWait: LatencyStats
  processingTime: LatencyStats
  totalTime: LatencyStats
  avgTokens?: number
}

export interface DbMetrics {
  totalQueries: number
  avgQueriesPerRecord: number
  queryLatency: LatencyStats
}

export interface MemoryMetrics {
  sampleCount: number
  minBytes: number
  maxBytes: number
  avgBytes: number
  p95Bytes: number
}

export interface SlowStageAlert {
  stage: string
  avgMs: number
  p95Ms: number
  thresholdMs: number
  occurrences: number
}

export interface PerformanceAnalyticsResponse {
  recordsAnalyzed: number
  overallLatency: LatencyStats
  stageLatencies: Record<string, LatencyStats>
  aiMetrics?: AiMetrics
  dbMetrics: DbMetrics
  memoryMetrics?: MemoryMetrics
  slowStages: SlowStageAlert[]
}

export interface AnalyticsParams {
  limit?: number
  minScore?: number
  aiInvolved?: boolean
  hours?: number
}

// =============================================================================
// Analytics API Functions
// =============================================================================

async function fetchPerformanceAnalytics(
  params: AnalyticsParams,
): Promise<PerformanceAnalyticsResponse> {
  const searchParams = new URLSearchParams()
  if (params.limit) searchParams.set('limit', params.limit.toString())
  if (params.minScore) searchParams.set('min_score', params.minScore.toString())
  if (params.aiInvolved !== undefined)
    searchParams.set('ai_involved', params.aiInvolved.toString())
  if (params.hours) searchParams.set('hours', params.hours.toString())

  const url = `${API_BASE}/api/audit-records/analytics?${searchParams.toString()}`
  const response = await fetch(url)
  if (!response.ok) {
    throw new Error(
      `Failed to fetch performance analytics: ${response.statusText}`,
    )
  }
  return response.json()
}

// =============================================================================
// Analytics Hook (Requirements 8.4, 8.5)
// =============================================================================

/** Get aggregated performance analytics */
export function usePerformanceAnalytics(params: AnalyticsParams = {}) {
  return useQuery({
    queryKey: queryKeys.auditRecords.analytics(params),
    queryFn: () => fetchPerformanceAnalytics(params),
    staleTime: 60000, // Cache for 1 minute
    retry: 2,
  })
}
