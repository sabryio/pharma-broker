/**
 * Branded ID types for type-safe entity references
 * Prevents accidentally mixing IDs from different entities
 */
import { Schema } from "effect";

// Core business entity IDs
export const OfferId = Schema.UUID.pipe(Schema.brand("OfferId"));
export type OfferId = Schema.Schema.Type<typeof OfferId>;

export const RequestId = Schema.UUID.pipe(Schema.brand("RequestId"));
export type RequestId = Schema.Schema.Type<typeof RequestId>;

export const MatchId = Schema.UUID.pipe(Schema.brand("MatchId"));
export type MatchId = Schema.Schema.Type<typeof MatchId>;

// WhatsApp entity IDs
export const RawMessageId = Schema.UUID.pipe(Schema.brand("RawMessageId"));
export type RawMessageId = Schema.Schema.Type<typeof RawMessageId>;

export const GroupId = Schema.UUID.pipe(Schema.brand("GroupId"));
export type GroupId = Schema.Schema.Type<typeof GroupId>;

export const ParticipantId = Schema.UUID.pipe(Schema.brand("ParticipantId"));
export type ParticipantId = Schema.Schema.Type<typeof ParticipantId>;

// WhatsApp JID (string-based identifier)
export const GroupJid = Schema.String.pipe(Schema.brand("GroupJid"));
export type GroupJid = Schema.Schema.Type<typeof GroupJid>;

export const SenderJid = Schema.String.pipe(Schema.brand("SenderJid"));
export type SenderJid = Schema.Schema.Type<typeof SenderJid>;

// Medication catalog IDs
export const MedicationMasterId = Schema.UUID.pipe(
  Schema.brand("MedicationMasterId")
);
export type MedicationMasterId = Schema.Schema.Type<typeof MedicationMasterId>;

export const MedicationAliasId = Schema.UUID.pipe(
  Schema.brand("MedicationAliasId")
);
export type MedicationAliasId = Schema.Schema.Type<typeof MedicationAliasId>;

export const MedicationMappingId = Schema.UUID.pipe(
  Schema.brand("MedicationMappingId")
);
export type MedicationMappingId = Schema.Schema.Type<
  typeof MedicationMappingId
>;

// Audit & queue IDs
export const AuditLogId = Schema.UUID.pipe(Schema.brand("AuditLogId"));
export type AuditLogId = Schema.Schema.Type<typeof AuditLogId>;

export const FeedbackRecordId = Schema.UUID.pipe(
  Schema.brand("FeedbackRecordId")
);
export type FeedbackRecordId = Schema.Schema.Type<typeof FeedbackRecordId>;

export const ReviewQueueId = Schema.UUID.pipe(Schema.brand("ReviewQueueId"));
export type ReviewQueueId = Schema.Schema.Type<typeof ReviewQueueId>;

export const MatchQueueId = Schema.UUID.pipe(Schema.brand("MatchQueueId"));
export type MatchQueueId = Schema.Schema.Type<typeof MatchQueueId>;
