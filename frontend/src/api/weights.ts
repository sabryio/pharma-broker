import apiClient from './client'

// ============================================================================
// Weights API Types
// ============================================================================

export interface Weights {
  medication: number
  pharmaceutical: number
  recency: number
  expiry: number
  supplier: number
  ai_logic: number
}

export interface WeightsResponse {
  weights: Weights
  prior_influence: number
  sample_count: number
}

export interface UpdateWeightsRequest {
  medication: number
  pharmaceutical: number
  recency: number
  expiry: number
  supplier: number
  ai_logic: number
  reason?: string
}

export interface UpdateWeightsResponse {
  success: boolean
  weights: Weights
  message: string
}

// ============================================================================
// API Functions
// ============================================================================

/**
 * Get current matching weights
 */
export async function getWeights(): Promise<WeightsResponse> {
  const response = await apiClient.get<WeightsResponse>('/api/weights')
  return response.data
}

/**
 * Update matching weights
 */
export async function updateWeights(
  request: UpdateWeightsRequest,
): Promise<UpdateWeightsResponse> {
  const response = await apiClient.put<UpdateWeightsResponse>(
    '/api/weights',
    request,
  )
  return response.data
}

// ============================================================================
// Helper Functions
// ============================================================================

/**
 * Convert frontend slider values (0-100) to backend weights (sum to 1.0)
 *
 * Frontend sliders:
 * - Medication Name Weight (0-100) -> medication weight
 * - Pharmaceutical Strictness (0-100) -> pharmaceutical weight (concentration + form)
 *
 * Fixed weights: recency=0.10, ai_logic=0.10, expiry=0.0, supplier=0.0
 */
export function slidersToWeights(sliders: {
  medicationWeight: number
  pharmaceuticalStrictness: number
}): Weights {
  // Fixed weights
  const recency = 0.1
  const ai_logic = 0.1
  const expiry = 0.0
  const supplier = 0.0
  const fixedTotal = recency + ai_logic + expiry + supplier

  // Available weight for dynamic allocation
  const availableWeight = 1.0 - fixedTotal // 0.80

  // Medication gets weight based on slider (40-80% of available)
  const medicationPct = 0.4 + (sliders.medicationWeight / 100) * 0.4
  const medication = availableWeight * medicationPct

  // Pharmaceutical gets the remaining weight
  const pharmaceutical = availableWeight - medication

  // Ensure weights sum to exactly 1.0 by normalizing
  const sum =
    medication + pharmaceutical + recency + ai_logic + expiry + supplier

  return {
    medication: medication / sum,
    pharmaceutical: pharmaceutical / sum,
    recency: recency / sum,
    expiry: expiry / sum,
    supplier: supplier / sum,
    ai_logic: ai_logic / sum,
  }
}

/**
 * Convert backend weights to frontend slider values
 */
export function weightsToSliders(weights: Weights): {
  medicationWeight: number
  pharmaceuticalStrictness: number
} {
  const availableWeight = 0.8 // 1.0 - recency - ai_logic - expiry - supplier

  // Medication weight percentage of available (40-80% range)
  const medicationPct = weights.medication / availableWeight
  const medicationWeight = Math.round(((medicationPct - 0.4) / 0.4) * 100)

  // Pharmaceutical strictness: proportion of available weight
  const pharmaceuticalPct = weights.pharmaceutical / availableWeight
  const pharmaceuticalStrictness = Math.round(pharmaceuticalPct * 100)

  return {
    medicationWeight: Math.max(0, Math.min(100, medicationWeight)),
    pharmaceuticalStrictness: Math.max(
      0,
      Math.min(100, pharmaceuticalStrictness),
    ),
  }
}
