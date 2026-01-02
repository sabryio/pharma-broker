import apiClient from './client'

// ============================================================================
// Weights API Types
// ============================================================================

export interface Weights {
  medication: number
  dosage: number
  quantity: number
  price: number
  recency: number
  ai_logic: number
}

export interface WeightsResponse {
  weights: Weights
  prior_influence: number
  sample_count: number
}

export interface UpdateWeightsRequest {
  medication: number
  dosage: number
  quantity: number
  price: number
  recency: number
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
 * - Dosage Strictness (0-100) -> dosage weight
 * - Quantity Tolerance (0-50) -> quantity weight (inverted - higher tolerance = lower weight)
 * - Price Flexibility (0-50) -> price weight (inverted - higher flexibility = lower weight)
 *
 * Fixed weights: recency=0.05, ai_logic=0.00
 */
export function slidersToWeights(sliders: {
  medicationWeight: number
  dosageStrictness: number
  quantityTolerance: number
  priceFlexibility: number
}): Weights {
  // Fixed weights
  const recency = 0.05
  const ai_logic = 0.0
  const fixedTotal = recency + ai_logic

  // Available weight for dynamic allocation
  const availableWeight = 1.0 - fixedTotal // 0.95

  // Medication gets the lion's share based on slider (50-90% of available)
  const medicationPct = 0.5 + (sliders.medicationWeight / 100) * 0.4
  const medication = availableWeight * medicationPct

  // Remaining weight for dosage, quantity, price
  const remaining = availableWeight - medication

  // Dosage strictness: higher = more weight on dosage
  const dosageRatio = sliders.dosageStrictness / 100

  // Quantity/Price: higher tolerance/flexibility = LESS weight (inverted)
  const quantityRatio = 1 - sliders.quantityTolerance / 50
  const priceRatio = 1 - sliders.priceFlexibility / 50

  // Normalize the three ratios
  const totalRatio = dosageRatio + quantityRatio + priceRatio

  let dosage: number, quantity: number, price: number

  if (totalRatio > 0) {
    dosage = remaining * (dosageRatio / totalRatio)
    quantity = remaining * (quantityRatio / totalRatio)
    price = remaining * (priceRatio / totalRatio)
  } else {
    // Equal distribution if all sliders are at extremes
    dosage = remaining / 3
    quantity = remaining / 3
    price = remaining / 3
  }

  return {
    medication: Math.round(medication * 1000) / 1000,
    dosage: Math.round(dosage * 1000) / 1000,
    quantity: Math.round(quantity * 1000) / 1000,
    price: Math.round(price * 1000) / 1000,
    recency,
    ai_logic,
  }
}

/**
 * Convert backend weights to frontend slider values
 */
export function weightsToSliders(weights: Weights): {
  medicationWeight: number
  dosageStrictness: number
  quantityTolerance: number
  priceFlexibility: number
} {
  const availableWeight = 0.95 // 1.0 - recency - ai_logic

  // Medication weight percentage of available (50-90% range)
  const medicationPct = weights.medication / availableWeight
  const medicationWeight = Math.round(((medicationPct - 0.5) / 0.4) * 100)

  // For the secondary weights, calculate their relative proportions
  const secondaryTotal = weights.dosage + weights.quantity + weights.price

  let dosageStrictness = 80
  let quantityTolerance = 15
  let priceFlexibility = 10

  if (secondaryTotal > 0) {
    // Dosage strictness: proportion of secondary weights
    dosageStrictness = Math.round((weights.dosage / secondaryTotal) * 100)

    // Quantity tolerance: inverted (higher weight = lower tolerance)
    const quantityProportion = weights.quantity / secondaryTotal
    quantityTolerance = Math.round((1 - quantityProportion) * 50)

    // Price flexibility: inverted (higher weight = lower flexibility)
    const priceProportion = weights.price / secondaryTotal
    priceFlexibility = Math.round((1 - priceProportion) * 50)
  }

  return {
    medicationWeight: Math.max(0, Math.min(100, medicationWeight)),
    dosageStrictness: Math.max(0, Math.min(100, dosageStrictness)),
    quantityTolerance: Math.max(0, Math.min(50, quantityTolerance)),
    priceFlexibility: Math.max(0, Math.min(50, priceFlexibility)),
  }
}
