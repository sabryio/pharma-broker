/**
 * Match entity schema and types
 */
import { Schema } from "@effect/schema";
import { MatchId, OfferId, RequestId } from "../ids";
import { MatchStatus, ConfidenceBand } from "../enums";

export const Match = Schema.Struct({
  id: MatchId,
  offerId: OfferId,
  requestId: RequestId,
  score: Schema.Number,
  status: MatchStatus,
  reasoning: Schema.NullOr(Schema.String),
  notes: Schema.NullOr(Schema.String),
  aiStatus: Schema.NullOr(Schema.String),
  aiConfidence: Schema.NullOr(Schema.Number),
  aiExplanation: Schema.NullOr(Schema.String),
  confirmedAt: Schema.NullOr(Schema.Date),
  confirmedBy: Schema.NullOr(Schema.String),
  createdAt: Schema.Date,
  updatedAt: Schema.Date,
});

export type Match = Schema.Schema.Type<typeof Match>;

export const MatchScore = Schema.Struct({
  total: Schema.Number,
  medication: Schema.Number,
  quantity: Schema.Number,
  dosage: Schema.Number,
  price: Schema.Number,
  recency: Schema.Number,
  band: ConfidenceBand,
  reasoning: Schema.String,
});

export type MatchScore = Schema.Schema.Type<typeof MatchScore>;

export const CreateMatchInput = Schema.Struct({
  offerId: OfferId,
  requestId: RequestId,
  score: Schema.Number,
  reasoning: Schema.optional(Schema.String),
});

export type CreateMatchInput = Schema.Schema.Type<typeof CreateMatchInput>;
