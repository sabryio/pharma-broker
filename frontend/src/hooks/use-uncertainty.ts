// Uncertainty Estimation Hooks
// TanStack React Query hooks for uncertainty estimation

import { useQuery, useMutation } from '@tanstack/react-query'
import { queryKeys } from './query-keys'

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8082'

// =============================================================================
// Response Transformation (snake_case -> camelCase)
// =============================================================================

interface ApiUncertaintyResult {
  mean_score: number
  std_dev: number
  coefficient_of_variation: number
  ci_lower: number
  ci_upper: number
  num_samples: number
  is_certain: boolean
  original_score: number
  uncertainty_level: string
  is_robust: boolean
}

interface ApiUncertaintyResponse {
  offer_id: string
  request_id: string
  result: ApiUncertaintyResult
}

function transformUncertaintyResult(
  api: ApiUncertaintyResult,
): UncertaintyResult {
  return {
    meanScore: api.mean_score,
    stdDev: api.std_dev,
    coefficientOfVariation: api.coefficient_of_variation,
    ciLower: api.ci_lower,
    ciUpper: api.ci_upper,
    numSamples: api.num_samples,
    isCertain: api.is_certain,
    originalScore: api.original_score,
    uncertaintyLevel: api.uncertainty_level,
    isRobust: api.is_robust,
  }
}

function transformUncertaintyResponse(
  api: ApiUncertaintyResponse,
): UncertaintyResponse {
  return {
    offerId: api.offer_id,
    requestId: api.request_id,
    result: transformUncertaintyResult(api.result),
  }
}

// =============================================================================
// Types
// =============================================================================

export interface UncertaintyResult {
  meanScore: number
  stdDev: number
  coefficientOfVariation: number
  ciLower: number
  ciUpper: number
  numSamples: number
  isCertain: boolean
  originalScore: number
  uncertaintyLevel: string
  isRobust: boolean
}

export interface UncertaintyResponse {
  offerId: string
  requestId: string
  result: UncertaintyResult
}

export interface UncertaintyConfig {
  numSamples: number
  perturbationStd: number
  confidenceLevel: number
  maxUncertaintyThreshold: number
  symmetricPerturbation: boolean
}

export interface UncertaintyStatusResponse {
  defaultConfig: UncertaintyConfig
}

export interface EstimateUncertaintyRequest {
  offerId: string
  requestId: string
  config?: {
    numSamples?: number
    perturbationStd?: number
    confidenceLevel?: number
  }
}

export interface BatchEstimateRequest {
  pairs: Array<{ offerId: string; requestId: string }>
  config?: {
    numSamples?: number
    perturbationStd?: number
    confidenceLevel?: number
  }
}

export interface BatchSummary {
  total: number
  certainCount: number
  uncertainCount: number
  avgStdDev: number
  avgMeanScore: number
}

export interface BatchUncertaintyResponse {
  results: UncertaintyResponse[]
  summary: BatchSummary
}

// =============================================================================
// API Response Types (snake_case from backend)
// =============================================================================

interface ApiUncertaintyConfig {
  num_samples: number
  perturbation_std: number
  confidence_level: number
  max_uncertainty_threshold: number
  symmetric_perturbation: boolean
}

interface ApiUncertaintyStatusResponse {
  default_config: ApiUncertaintyConfig
}

interface ApiBatchSummary {
  total: number
  certain_count: number
  uncertain_count: number
  avg_std_dev: number
  avg_mean_score: number
}

interface ApiBatchUncertaintyResponse {
  results: ApiUncertaintyResponse[]
  summary: ApiBatchSummary
}

function transformUncertaintyConfig(
  api: ApiUncertaintyConfig,
): UncertaintyConfig {
  return {
    numSamples: api.num_samples,
    perturbationStd: api.perturbation_std,
    confidenceLevel: api.confidence_level,
    maxUncertaintyThreshold: api.max_uncertainty_threshold,
    symmetricPerturbation: api.symmetric_perturbation,
  }
}

function transformUncertaintyStatusResponse(
  api: ApiUncertaintyStatusResponse,
): UncertaintyStatusResponse {
  return {
    defaultConfig: transformUncertaintyConfig(api.default_config),
  }
}

function transformBatchUncertaintyResponse(
  api: ApiBatchUncertaintyResponse,
): BatchUncertaintyResponse {
  return {
    results: api.results.map(transformUncertaintyResponse),
    summary: {
      total: api.summary.total,
      certainCount: api.summary.certain_count,
      uncertainCount: api.summary.uncertain_count,
      avgStdDev: api.summary.avg_std_dev,
      avgMeanScore: api.summary.avg_mean_score,
    },
  }
}

// =============================================================================
// API Functions
// =============================================================================

async function fetchUncertaintyStatus(): Promise<UncertaintyStatusResponse> {
  const response = await fetch(`${API_BASE}/api/uncertainty/status`)
  if (!response.ok) {
    throw new Error(
      `Failed to fetch uncertainty status: ${response.statusText}`,
    )
  }
  const data: ApiUncertaintyStatusResponse = await response.json()
  return transformUncertaintyStatusResponse(data)
}

async function estimateUncertainty(
  data: EstimateUncertaintyRequest,
): Promise<UncertaintyResponse> {
  const response = await fetch(`${API_BASE}/api/uncertainty/estimate`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      offer_id: data.offerId,
      request_id: data.requestId,
      config: data.config,
    }),
  })
  if (!response.ok) {
    throw new Error(`Failed to estimate uncertainty: ${response.statusText}`)
  }
  const apiData: ApiUncertaintyResponse = await response.json()
  return transformUncertaintyResponse(apiData)
}

async function batchEstimateUncertainty(
  data: BatchEstimateRequest,
): Promise<BatchUncertaintyResponse> {
  const response = await fetch(`${API_BASE}/api/uncertainty/batch`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      pairs: data.pairs.map((p) => ({
        offer_id: p.offerId,
        request_id: p.requestId,
      })),
      config: data.config,
    }),
  })
  if (!response.ok) {
    throw new Error(
      `Failed to batch estimate uncertainty: ${response.statusText}`,
    )
  }
  const apiData: ApiBatchUncertaintyResponse = await response.json()
  return transformBatchUncertaintyResponse(apiData)
}

async function fetchMatchUncertainty(
  matchId: string,
): Promise<UncertaintyResponse> {
  const response = await fetch(`${API_BASE}/api/uncertainty/match/${matchId}`)
  if (!response.ok) {
    throw new Error(`Failed to fetch match uncertainty: ${response.statusText}`)
  }
  const data: ApiUncertaintyResponse = await response.json()
  return transformUncertaintyResponse(data)
}

// =============================================================================
// Hooks
// =============================================================================

/** Get uncertainty estimator status and config */
export function useUncertaintyStatus() {
  return useQuery({
    queryKey: queryKeys.uncertainty.status(),
    queryFn: fetchUncertaintyStatus,
  })
}

/** Get uncertainty for an existing match */
export function useMatchUncertainty(matchId: string | undefined) {
  return useQuery({
    queryKey: queryKeys.uncertainty.match(matchId ?? ''),
    queryFn: () => fetchMatchUncertainty(matchId!),
    enabled: !!matchId,
  })
}

/** Estimate uncertainty for an offer/request pair */
export function useEstimateUncertainty() {
  return useMutation({
    mutationFn: estimateUncertainty,
  })
}

/** Batch estimate uncertainty for multiple pairs */
export function useBatchEstimateUncertainty() {
  return useMutation({
    mutationFn: batchEstimateUncertainty,
  })
}

// =============================================================================
// Utility Functions
// =============================================================================

/** Get color for uncertainty level */
export function getUncertaintyColor(level: string): string {
  switch (level) {
    case 'very_low':
      return 'text-green-600'
    case 'low':
      return 'text-green-500'
    case 'moderate':
      return 'text-yellow-500'
    case 'high':
      return 'text-orange-500'
    case 'very_high':
      return 'text-red-500'
    default:
      return 'text-gray-500'
  }
}

/** Get badge variant for uncertainty level */
export function getUncertaintyBadgeVariant(
  level: string,
): 'default' | 'secondary' | 'destructive' | 'outline' {
  switch (level) {
    case 'very_low':
    case 'low':
      return 'default'
    case 'moderate':
      return 'secondary'
    case 'high':
    case 'very_high':
      return 'destructive'
    default:
      return 'outline'
  }
}

/** Format uncertainty as percentage */
export function formatUncertainty(stdDev: number): string {
  return `±${(stdDev * 100).toFixed(1)}%`
}

/** Format confidence interval */
export function formatConfidenceInterval(lower: number, upper: number): string {
  return `[${(lower * 100).toFixed(1)}%, ${(upper * 100).toFixed(1)}%]`
}
