import { apiClient } from './client'

// ============================================================================
// Types
// ============================================================================

export interface AiHealthResponse {
  status: string
  circuitBreaker: CircuitBreakerStatus
  modelInfo: ModelInfo
  performance: PerformanceMetrics
  retryQueue: RetryQueueStats
  recentErrors: RecentError[]
}

export interface CircuitBreakerStatus {
  state: 'closed' | 'open' | 'half_open'
  failureCount: number
  successCount: number
  lastFailureTime: string | null
  nextRetryTime: string | null
}

export interface ModelInfo {
  endpoint: string
  modelName: string
  timeoutSeconds: number
  maxRetries: number
}

export interface PerformanceMetrics {
  successRate1h: number
  successRate24h: number
  avgResponseTimeMs: number
  p95ResponseTimeMs: number
  totalRequests1h: number
  totalRequests24h: number
}

export interface RetryQueueStats {
  pending: number
  processing: number
  completed: number
  failed: number
  byReason: FailureReasonCount[]
}

export interface FailureReasonCount {
  reason: string
  count: number
}

export interface RecentError {
  timestamp: string
  errorType: string
  message: string
  rawMessageId: string | null
}

export interface TestConnectionResponse {
  success: boolean
  responseTimeMs: number
  error: string | null
}

// ============================================================================
// API Functions
// ============================================================================

/**
 * Get comprehensive AI health status
 */
export async function getAiHealth(): Promise<AiHealthResponse> {
  const response = await apiClient.get<AiHealthResponse>('/api/ai-health')
  return response.data
}

/**
 * Get circuit breaker status only
 */
export async function getCircuitBreaker(): Promise<CircuitBreakerStatus> {
  const response = await apiClient.get<CircuitBreakerStatus>(
    '/api/ai-health/circuit-breaker',
  )
  return response.data
}

/**
 * Get retry queue statistics
 */
export async function getRetryQueue(): Promise<RetryQueueStats> {
  const response = await apiClient.get<RetryQueueStats>(
    '/api/ai-health/retry-queue',
  )
  return response.data
}

/**
 * Test connection to AI gateway
 */
export async function testConnection(): Promise<TestConnectionResponse> {
  const response = await apiClient.post<TestConnectionResponse>(
    '/api/ai-health/test-connection',
  )
  return response.data
}
