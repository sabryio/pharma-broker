/**
 * Domain enums matching Rust/Prisma definitions
 */
import { Schema } from "@effect/schema";

// Item status for offers and requests
export const ItemStatus = Schema.Literal(
  "active",
  "matched",
  "expired",
  "cancelled"
);
export type ItemStatus = Schema.Schema.Type<typeof ItemStatus>;

// Urgency level for requests
export const UrgencyLevel = Schema.Literal("low", "medium", "high", "critical");
export type UrgencyLevel = Schema.Schema.Type<typeof UrgencyLevel>;

// Match status
export const MatchStatus = Schema.Literal(
  "pending",
  "confirmed",
  "rejected",
  "cancelled",
  "expired"
);
export type MatchStatus = Schema.Schema.Type<typeof MatchStatus>;

// Review queue status
export const ReviewStatus = Schema.Literal(
  "pending",
  "approved",
  "rejected",
  "skipped"
);
export type ReviewStatus = Schema.Schema.Type<typeof ReviewStatus>;

// Match queue status
export const QueueStatus = Schema.Literal(
  "pending",
  "processing",
  "completed",
  "failed"
);
export type QueueStatus = Schema.Schema.Type<typeof QueueStatus>;

// Medication catalog statuses
export const MedicationStatus = Schema.Literal(
  "active",
  "inactive",
  "deprecated"
);
export type MedicationStatus = Schema.Schema.Type<typeof MedicationStatus>;

export const CurationStatus = Schema.Literal("pending", "approved", "rejected");
export type CurationStatus = Schema.Schema.Type<typeof CurationStatus>;

// Message type from AI parsing
export const MessageType = Schema.Literal(
  "offer",
  "request",
  "both",
  "unknown"
);
export type MessageType = Schema.Schema.Type<typeof MessageType>;

// Feedback decision
export const FeedbackDecision = Schema.Literal("confirmed", "rejected");
export type FeedbackDecision = Schema.Schema.Type<typeof FeedbackDecision>;

// Confidence bands for matching
export const ConfidenceBand = Schema.Literal(
  "auto",
  "suggest",
  "review",
  "none"
);
export type ConfidenceBand = Schema.Schema.Type<typeof ConfidenceBand>;

/**
 * Get confidence band from score
 * - Auto: >= 0.90 (automatic confirmation)
 * - Suggest: 0.70 - 0.89 (operator approval)
 * - Review: 0.50 - 0.69 (manual review)
 * - None: < 0.50 (no match)
 */
export const getConfidenceBand = (score: number): ConfidenceBand => {
  if (score >= 0.9) return "auto";
  if (score >= 0.7) return "suggest";
  if (score >= 0.5) return "review";
  return "none";
};
