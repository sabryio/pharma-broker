// Uncertainty Estimation Hooks
// TanStack React Query hooks for uncertainty estimation

import { useQuery, useMutation } from '@tanstack/react-query'
import { queryKeys } from './query-keys'

const API_BASE = '/api'

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
// API Functions
// =============================================================================

async function fetchUncertaintyStatus(): Promise<UncertaintyStatusResponse> {
  const response = await fetch(`${API_BASE}/uncertainty/status`)
  if (!response.ok) {
    throw new Error(
      `Failed to fetch uncertainty status: ${response.statusText}`,
    )
  }
  return response.json()
}

async function estimateUncertainty(
  data: EstimateUncertaintyRequest,
): Promise<UncertaintyResponse> {
  const response = await fetch(`${API_BASE}/uncertainty/estimate`, {
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
  return response.json()
}

async function batchEstimateUncertainty(
  data: BatchEstimateRequest,
): Promise<BatchUncertaintyResponse> {
  const response = await fetch(`${API_BASE}/uncertainty/batch`, {
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
  return response.json()
}

async function fetchMatchUncertainty(
  matchId: string,
): Promise<UncertaintyResponse> {
  const response = await fetch(`${API_BASE}/uncertainty/match/${matchId}`)
  if (!response.ok) {
    throw new Error(`Failed to fetch match uncertainty: ${response.statusText}`)
  }
  return response.json()
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
