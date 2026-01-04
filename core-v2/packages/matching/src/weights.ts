/**
 * Matching engine weights configuration
 * Mirrors Rust matching/weights.rs
 */
import { Schema } from "effect";

export const MatchWeights = Schema.Struct({
  medication: Schema.Number.pipe(Schema.between(0, 1)),
  quantity: Schema.Number.pipe(Schema.between(0, 1)),
  dosage: Schema.Number.pipe(Schema.between(0, 1)),
  price: Schema.Number.pipe(Schema.between(0, 1)),
  recency: Schema.Number.pipe(Schema.between(0, 1)),
  aiLogic: Schema.Number.pipe(Schema.between(0, 1)),
});

export type MatchWeights = Schema.Schema.Type<typeof MatchWeights>;

/**
 * Default weights matching Rust implementation
 * - Medication: 40% (most important)
 * - Quantity: 20%
 * - Dosage: 15%
 * - Price: 15%
 * - Recency: 10%
 */
export const DEFAULT_WEIGHTS: MatchWeights = {
  medication: 0.4,
  quantity: 0.2,
  dosage: 0.15,
  price: 0.15,
  recency: 0.1,
  aiLogic: 0.0, // Optional AI logic score
};

/**
 * Normalize weights to sum to 1.0
 */
export const normalizeWeights = (weights: MatchWeights): MatchWeights => {
  const sum =
    weights.medication +
    weights.quantity +
    weights.dosage +
    weights.price +
    weights.recency +
    weights.aiLogic;

  if (sum === 0) return DEFAULT_WEIGHTS;

  return {
    medication: weights.medication / sum,
    quantity: weights.quantity / sum,
    dosage: weights.dosage / sum,
    price: weights.price / sum,
    recency: weights.recency / sum,
    aiLogic: weights.aiLogic / sum,
  };
};
