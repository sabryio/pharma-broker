/**
 * Match scoring implementation
 * Mirrors Rust matching/scorer.rs
 */
import { Effect } from "effect";
import type { Offer, Request, MatchScore } from "@core-v2/domain";
import { getConfidenceBand } from "@core-v2/domain";
import type { MatchWeights } from "./weights";

/**
 * Score medication similarity using fuzzy matching
 * Returns 0-1 score based on Levenshtein distance
 */
export const scoreMedication = (
  offer: Offer,
  request: Request
): Effect.Effect<number> =>
  Effect.sync(() => {
    const offerMed = (offer.medication || "").toLowerCase().trim();
    const requestMed = (request.medication || "").toLowerCase().trim();

    if (!offerMed || !requestMed) return 0;
    if (offerMed === requestMed) return 1.0;

    // Simple fuzzy match using includes
    if (offerMed.includes(requestMed) || requestMed.includes(offerMed)) {
      return 0.85;
    }

    // Levenshtein distance normalized
    const distance = levenshteinDistance(offerMed, requestMed);
    const maxLen = Math.max(offerMed.length, requestMed.length);
    return Math.max(0, 1 - distance / maxLen);
  });

/**
 * Score quantity fulfillment
 * Returns 1.0 if offer quantity >= request quantity
 */
export const scoreQuantity = (
  offer: Offer,
  request: Request
): Effect.Effect<number> =>
  Effect.sync(() => {
    const offerQty = parseQuantity(offer.quantity);
    const requestQty = parseQuantity(request.quantity);

    if (offerQty === null || requestQty === null) return 0.5; // Unknown
    if (requestQty === 0) return 1.0;
    if (offerQty >= requestQty) return 1.0;

    return offerQty / requestQty;
  });

/**
 * Score dosage match
 */
export const scoreDosage = (
  offer: Offer,
  _request: Request
): Effect.Effect<number> =>
  Effect.sync(() => {
    // TODO: Extract and compare dosage from medication strings
    // For now, return neutral score
    return offer.medication ? 0.7 : 0.5;
  });

/**
 * Score price fit (offer price <= request max price)
 */
export const scorePrice = (
  offer: Offer,
  request: Request
): Effect.Effect<number> =>
  Effect.sync(() => {
    const offerPrice = parsePrice(offer.price);
    const maxPrice = parsePrice(request.maxPrice);

    if (offerPrice === null || maxPrice === null) return 0.5;
    if (offerPrice <= maxPrice) return 1.0;

    // Penalize based on how much over budget
    const ratio = maxPrice / offerPrice;
    return Math.max(0, ratio);
  });

/**
 * Score recency (exponential decay based on offer age)
 */
export const scoreRecency = (offer: Offer): Effect.Effect<number> =>
  Effect.sync(() => {
    const ageMs = Date.now() - offer.createdAt.getTime();
    const ageHours = ageMs / (1000 * 60 * 60);

    // Exponential decay: half-life of 24 hours
    const halfLife = 24;
    return Math.exp((-0.693 * ageHours) / halfLife);
  });

/**
 * Calculate total match score
 */
export const calculateScore = (
  offer: Offer,
  request: Request,
  weights: MatchWeights
): Effect.Effect<MatchScore> =>
  Effect.gen(function* () {
    const medication = yield* scoreMedication(offer, request);
    const quantity = yield* scoreQuantity(offer, request);
    const dosage = yield* scoreDosage(offer, request);
    const price = yield* scorePrice(offer, request);
    const recency = yield* scoreRecency(offer);

    const total =
      medication * weights.medication +
      quantity * weights.quantity +
      dosage * weights.dosage +
      price * weights.price +
      recency * weights.recency;

    const band = getConfidenceBand(total);
    const reasoning = generateReasoning({
      medication,
      quantity,
      dosage,
      price,
      recency,
      total,
    });

    return {
      total,
      medication,
      quantity,
      dosage,
      price,
      recency,
      band,
      reasoning,
    };
  });

// Helper functions
const parseQuantity = (qty: string | null): number | null => {
  if (!qty) return null;
  const match = qty.match(/(\d+)/);
  return match ? parseInt(match[1], 10) : null;
};

const parsePrice = (price: string | null): number | null => {
  if (!price) return null;
  const match = price.match(/(\d+(?:\.\d+)?)/);
  return match ? parseFloat(match[1]) : null;
};

const levenshteinDistance = (a: string, b: string): number => {
  const matrix: number[][] = [];
  for (let i = 0; i <= b.length; i++) matrix[i] = [i];
  for (let j = 0; j <= a.length; j++) matrix[0][j] = j;
  for (let i = 1; i <= b.length; i++) {
    for (let j = 1; j <= a.length; j++) {
      matrix[i][j] =
        b[i - 1] === a[j - 1]
          ? matrix[i - 1][j - 1]
          : Math.min(
              matrix[i - 1][j - 1] + 1,
              matrix[i][j - 1] + 1,
              matrix[i - 1][j] + 1
            );
    }
  }
  return matrix[b.length][a.length];
};

const generateReasoning = (scores: Record<string, number>): string => {
  const parts: string[] = [];
  if (scores.medication >= 0.9) parts.push("Exact medication match");
  else if (scores.medication >= 0.7) parts.push("Similar medication");
  if (scores.quantity >= 1.0) parts.push("Quantity fulfilled");
  if (scores.price >= 1.0) parts.push("Within budget");
  return parts.join(". ") || "Partial match";
};
