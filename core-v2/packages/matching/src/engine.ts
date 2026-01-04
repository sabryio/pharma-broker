/**
 * Matching Engine service
 */
import { Effect, Context, Layer, Ref } from "effect";
import type { Offer, Request, MatchScore } from "@core-v2/domain";
import type { MatchWeights } from "./weights";
import { DEFAULT_WEIGHTS, normalizeWeights } from "./weights";
import { calculateScore } from "./scorer";

/**
 * Matching Engine service interface
 */
export class MatchingEngine extends Context.Tag("MatchingEngine")<
  MatchingEngine,
  {
    readonly findMatches: (
      request: Request,
      offers: Offer[]
    ) => Effect.Effect<MatchScore[]>;
    readonly scoreMatch: (
      offer: Offer,
      request: Request
    ) => Effect.Effect<MatchScore>;
    readonly getWeights: () => Effect.Effect<MatchWeights>;
    readonly updateWeights: (weights: MatchWeights) => Effect.Effect<void>;
  }
>() {}

/**
 * Matching Engine configuration
 */
export interface MatchingEngineConfig {
  readonly minScore: number;
  readonly maxResults: number;
  readonly initialWeights: MatchWeights;
}

export const defaultConfig: MatchingEngineConfig = {
  minScore: 0.5,
  maxResults: 10,
  initialWeights: DEFAULT_WEIGHTS,
};

/**
 * Live implementation of MatchingEngine
 */
export const MatchingEngineLive = (
  config: MatchingEngineConfig = defaultConfig
) =>
  Layer.effect(
    MatchingEngine,
    Effect.gen(function* () {
      const weightsRef = yield* Ref.make(
        normalizeWeights(config.initialWeights)
      );

      return {
        findMatches: (request, offers) =>
          Effect.gen(function* () {
            const weights = yield* Ref.get(weightsRef);

            const scores = yield* Effect.forEach(
              offers,
              (offer) => calculateScore(offer, request, weights),
              { concurrency: 10 }
            );

            return scores
              .filter((s) => s.total >= config.minScore)
              .sort((a, b) => b.total - a.total)
              .slice(0, config.maxResults);
          }),

        scoreMatch: (offer, request) =>
          Effect.gen(function* () {
            const weights = yield* Ref.get(weightsRef);
            return yield* calculateScore(offer, request, weights);
          }),

        getWeights: () => Ref.get(weightsRef),

        updateWeights: (weights) =>
          Ref.set(weightsRef, normalizeWeights(weights)),
      };
    })
  );
