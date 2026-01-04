/**
 * Request entity schema and types
 */
import { Schema } from "@effect/schema";
import { RequestId, GroupJid, SenderJid, RawMessageId } from "../ids";
import { ItemStatus, UrgencyLevel } from "../enums";

export const Request = Schema.Struct({
  id: RequestId,
  medication: Schema.String,
  medicationRaw: Schema.NullOr(Schema.String),
  quantity: Schema.NullOr(Schema.String),
  maxPrice: Schema.NullOr(Schema.String),
  urgency: UrgencyLevel,
  status: ItemStatus,
  groupJid: Schema.NullOr(GroupJid),
  senderJid: Schema.NullOr(SenderJid),
  rawMessageId: Schema.NullOr(RawMessageId),
  aiConfidence: Schema.NullOr(Schema.Number),
  embedding: Schema.NullOr(Schema.Array(Schema.Number)),
  confirmedMatchCount: Schema.Number,
  createdAt: Schema.Date,
  updatedAt: Schema.Date,
});

export type Request = Schema.Schema.Type<typeof Request>;

export const CreateRequestInput = Schema.Struct({
  medication: Schema.String,
  medicationRaw: Schema.optional(Schema.String),
  quantity: Schema.optional(Schema.String),
  maxPrice: Schema.optional(Schema.String),
  urgency: Schema.optional(UrgencyLevel),
  groupJid: Schema.optional(GroupJid),
  senderJid: Schema.optional(SenderJid),
  rawMessageId: Schema.optional(RawMessageId),
  aiConfidence: Schema.optional(Schema.Number),
  embedding: Schema.optional(Schema.Array(Schema.Number)),
});

export type CreateRequestInput = Schema.Schema.Type<typeof CreateRequestInput>;

export const UpdateRequestInput = Schema.Struct({
  medication: Schema.optional(Schema.String),
  medicationRaw: Schema.optional(Schema.String),
  quantity: Schema.optional(Schema.String),
  maxPrice: Schema.optional(Schema.String),
  urgency: Schema.optional(UrgencyLevel),
  status: Schema.optional(ItemStatus),
  aiConfidence: Schema.optional(Schema.Number),
  embedding: Schema.optional(Schema.Array(Schema.Number)),
});

export type UpdateRequestInput = Schema.Schema.Type<typeof UpdateRequestInput>;
