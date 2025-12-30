import { query, mutation } from "./_generated/server";
import { v } from "convex/values";
import { Doc, Id } from "./_generated/dataModel";

// List matches with optional filters
export const list = query({
  args: {
    status: v.optional(v.string()),
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    if (args.status) {
      return await ctx.db
        .query("matches")
        .withIndex("by_status", (idx) => idx.eq("status", args.status!))
        .order("desc")
        .take(args.limit ?? 100);
    }
    return await ctx.db
      .query("matches")
      .order("desc")
      .take(args.limit ?? 100);
  },
});

// Get match by ID
export const get = query({
  args: { id: v.string() },
  handler: async (ctx, args) => {
    try {
      return await ctx.db.get(args.id as Id<"matches">);
    } catch {
      return null;
    }
  },
});

// Get match by offer and request
export const getByOfferAndRequest = query({
  args: {
    offerId: v.string(),
    requestId: v.string(),
  },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("matches")
      .withIndex("by_offer", (q) => q.eq("offerId", args.offerId))
      .filter((q) => q.eq(q.field("requestId"), args.requestId))
      .first();
  },
});

// Get match with full offer and request data
export const getWithDetails = query({
  args: { id: v.id("matches") },
  handler: async (ctx, args) => {
    const match = await ctx.db.get(args.id);
    if (!match) return null;

    const offer = await ctx.db.get(match.offerId as Id<"offers">);
    const request = await ctx.db.get(match.requestId as Id<"requests">);

    return { match, offer, request };
  },
});

// Create match
export const create = mutation({
  args: {
    offerId: v.string(),
    requestId: v.string(),
    score: v.number(),
    reasoning: v.optional(v.string()),
    matchedBy: v.optional(v.string()),
    status: v.string(),
    notes: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    const now = Date.now();
    return await ctx.db.insert("matches", {
      offerId: args.offerId,
      requestId: args.requestId,
      score: args.score,
      reasoning: args.reasoning,
      matchedBy: args.matchedBy,
      status: args.status,
      createdAt: now,
      confirmedAt: undefined,
      notes: args.notes,
    });
  },
});

// Confirm match
export const confirm = mutation({
  args: {
    id: v.id("matches"),
    matchedBy: v.string(),
    notes: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    const match = await ctx.db.get(args.id);
    if (!match) throw new Error("Match not found");

    // Update match
    await ctx.db.patch(args.id, {
      status: "CONFIRMED",
      confirmedAt: Date.now(),
      matchedBy: args.matchedBy,
      notes: args.notes,
    });

    // Update offer and request status (need to find by string ID)
    const offer = await ctx.db.get(match.offerId as Id<"offers">);
    const request = await ctx.db.get(match.requestId as Id<"requests">);

    if (offer) {
      await ctx.db.patch(offer._id, { status: "MATCHED" });
    }
    if (request) {
      await ctx.db.patch(request._id, { status: "MATCHED" });
    }

    return await ctx.db.get(args.id);
  },
});

// Reject match
export const reject = mutation({
  args: {
    id: v.id("matches"),
    matchedBy: v.string(),
    notes: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    await ctx.db.patch(args.id, {
      status: "REJECTED",
      matchedBy: args.matchedBy,
      notes: args.notes,
    });
    return await ctx.db.get(args.id);
  },
});

// Get pending matches count
export const countPending = query({
  args: {},
  handler: async (ctx) => {
    const matches = await ctx.db
      .query("matches")
      .withIndex("by_status", (q) => q.eq("status", "PENDING"))
      .collect();
    return matches.length;
  },
});

// Delete matches created before cutoff (matches SeaORM delete_before)
export const deleteBefore = mutation({
  args: { cutoff: v.number() },
  handler: async (ctx, args) => {
    const toDelete = await ctx.db
      .query("matches")
      .withIndex("by_created", (q) => q.lt("createdAt", args.cutoff))
      .collect();

    for (const doc of toDelete) {
      await ctx.db.delete(doc._id);
    }
    return toDelete.length;
  },
});
