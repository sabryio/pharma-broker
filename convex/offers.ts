import { action, internalQuery, mutation, query } from "./_generated/server";
import { v } from "convex/values";
import { api, internal } from "./_generated/api";
import { Doc, Id } from "./_generated/dataModel";

// Helper type for internal vector search results
type VectorSearchResult = {
  _id: Id<"offers">;
  _score: number;
};

// List offers with optional filters
export const list = query({
  args: {
    status: v.optional(v.string()),
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    if (args.status) {
      return await ctx.db
        .query("offers")
        .withIndex("by_status", (idx) => idx.eq("status", args.status!))
        .order("desc")
        .take(args.limit ?? 100);
    }
    return await ctx.db
      .query("offers")
      .order("desc")
      .take(args.limit ?? 100);
  },
});

// Get offer by ID
export const getById = query({
  args: { id: v.string() },
  handler: async (ctx, args) => {
    try {
      // Try to parse as Convex Id first
      const docId = ctx.db.normalizeId("offers", args.id);
      if (!docId) return null;
      return await ctx.db.get(docId);
    } catch (e) {
      return null;
    }
  },
});

export const get = getById;

// Internal get for actions
export const getInternal = internalQuery({
  args: { id: v.id("offers") },
  handler: async (ctx, args) => {
    return await ctx.db.get(args.id);
  },
});

// Count active offers
export const countActive = query({
  args: {},
  handler: async (ctx) => {
    const active = await ctx.db
      .query("offers")
      .withIndex("by_status", (q) => q.eq("status", "ACTIVE"))
      .collect();
    return active.length;
  },
});

// Create offer
export const create = mutation({
  args: {
    sourceName: v.optional(v.string()),
    sourcePhone: v.string(),
    sourceGroup: v.string(),
    medication: v.string(),
    medicationRaw: v.string(),
    quantity: v.optional(v.number()),
    unit: v.optional(v.string()),
    price: v.optional(v.number()),
    currency: v.optional(v.string()),
    status: v.string(),
    urgencyLevel: v.string(),
    aiConfidence: v.number(),
    notes: v.optional(v.string()),
    rawMessageId: v.string(),
    contentEmbedding: v.optional(v.array(v.number())),
    expiryDate: v.optional(v.string()),
    batchNumber: v.optional(v.string()),
    expiryInfo: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    const now = Date.now();
    return await ctx.db.insert("offers", {
      ...args,
      createdAt: now,
      updatedAt: now,
    });
  },
});

// Update offer status
export const updateStatus = mutation({
  args: {
    id: v.string(),
    status: v.string(),
  },
  handler: async (ctx, args) => {
    const docId = args.id as Id<"offers">;
    await ctx.db.patch(docId, {
      status: args.status,
      updatedAt: Date.now(),
    });
    return await ctx.db.get(docId);
  },
});

// Find recent duplicate
export const findRecentDuplicate = query({
  args: {
    senderPhone: v.string(),
    medication: v.string(),
    withinMs: v.number(),
  },
  handler: async (ctx, args) => {
    const cutoff = Date.now() - args.withinMs;
    return await ctx.db
      .query("offers")
      .withIndex("by_source", (q) => q.eq("sourcePhone", args.senderPhone))
      .filter((q) =>
        q.and(
          q.eq(q.field("medication"), args.medication),
          q.eq(q.field("status"), "ACTIVE"),
          q.gte(q.field("createdAt"), cutoff)
        )
      )
      .first();
  },
});

// Search offers by medication (LIKE %query% approximate)
export const search = query({
  args: {
    query: v.string(),
    limit: v.optional(v.number()),
    status: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    if (args.status) {
      return await ctx.db
        .query("offers")
        .withSearchIndex("search_medication", (q) =>
          q.search("medication", args.query).eq("status", args.status!)
        )
        .take(args.limit ?? 100);
    }

    return await ctx.db
      .query("offers")
      .withSearchIndex("search_medication", (q) =>
        q.search("medication", args.query)
      )
      .take(args.limit ?? 100);
  },
});

// Offer with score
type OfferWithScore = Doc<"offers"> & { _score: number };

// Vector search for semantic duplicates
export const searchSimilar = action({
  args: {
    embedding: v.array(v.number()),
    limit: v.optional(v.number()),
    statusFilter: v.optional(v.string()),
    withinMs: v.optional(v.number()),
    similarityThreshold: v.optional(v.number()),
  },
  handler: async (ctx, args): Promise<OfferWithScore[]> => {
    const results: VectorSearchResult[] = await ctx.vectorSearch(
      "offers",
      "by_embedding",
      {
        vector: args.embedding,
        limit: args.limit ?? 10,
        filter: args.statusFilter
          ? (q) => q.eq("status", args.statusFilter!)
          : undefined,
      }
    );

    const cutoff = args.withinMs ? Date.now() - args.withinMs : 0;
    const offersWithScores: OfferWithScore[] = [];

    for (const result of results) {
      const offer = await ctx.runQuery(api.offers.getById, {
        id: result._id,
      });
      if (offer) {
        if (
          (!args.withinMs || offer.createdAt >= cutoff) &&
          (!args.similarityThreshold ||
            result._score >= args.similarityThreshold)
        ) {
          offersWithScores.push({ ...offer, _score: result._score });
        }
      }
    }

    return offersWithScores;
  },
});

// Delete offers created before cutoff (matches SeaORM delete_before)
export const deleteBefore = mutation({
  args: { cutoff: v.number() },
  handler: async (ctx, args) => {
    const toDelete = await ctx.db
      .query("offers")
      .withIndex("by_created", (q) => q.lt("createdAt", args.cutoff))
      .collect();

    for (const doc of toDelete) {
      await ctx.db.delete(doc._id);
    }
    return toDelete.length;
  },
});
