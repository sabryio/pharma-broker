/**
 * Offer entity schema and types
 */
import { Schema } from "@effect/schema";
import { OfferId, GroupJid, SenderJid, RawMessageId } from "../ids";
import { ItemStatus } from "../enums";

export const Offer = Schema.Struct({
  id: OfferId,
  medication: Schema.String,
  medicationRaw: Schema.NullOr(Schema.String),
  quantity: Schema.NullOr(Schema.String),
  price: Schema.NullOr(Schema.String),
  expiry: Schema.NullOr(Schema.Date),
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

export type Offer = Schema.Schema.Type<typeof Offer>;

export const CreateOfferInput = Schema.Struct({
  medication: Schema.String,
  medicationRaw: Schema.optional(Schema.String),
  quantity: Schema.optional(Schema.String),
  price: Schema.optional(Schema.String),
  expiry: Schema.optional(Schema.Date),
  groupJid: Schema.optional(GroupJid),
  senderJid: Schema.optional(SenderJid),
  rawMessageId: Schema.optional(RawMessageId),
  aiConfidence: Schema.optional(Schema.Number),
  embedding: Schema.optional(Schema.Array(Schema.Number)),
});

export type CreateOfferInput = Schema.Schema.Type<typeof CreateOfferInput>;

export const UpdateOfferInput = Schema.Struct({
  medication: Schema.optional(Schema.String),
  medicationRaw: Schema.optional(Schema.String),
  quantity: Schema.optional(Schema.String),
  price: Schema.optional(Schema.String),
  expiry: Schema.optional(Schema.Date),
  status: Schema.optional(ItemStatus),
  aiConfidence: Schema.optional(Schema.Number),
  embedding: Schema.optional(Schema.Array(Schema.Number)),
});

export type UpdateOfferInput = Schema.Schema.Type<typeof UpdateOfferInput>;
