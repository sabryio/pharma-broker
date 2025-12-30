import { query, mutation } from "./_generated/server";
import { v } from "convex/values";

// List items needing review (matches entity/review_queue.rs)
export const listPending = query({
  args: { limit: v.optional(v.number()) },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("reviewQueue")
      .withIndex("by_status", (q) => q.eq("status", "pending"))
      .order("desc")
      .take(args.limit ?? 100);
  },
});

// List by status
export const listByStatus = query({
  args: {
    status: v.string(),
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("reviewQueue")
      .withIndex("by_status", (q) => q.eq("status", args.status))
      .order("desc")
      .take(args.limit ?? 100);
  },
});

// Get by ID
export const get = query({
  args: { id: v.string() },
  handler: async (ctx, args) => {
    const items = await ctx.db.query("reviewQueue").collect();
    return items.find((i) => i._id.toString() === args.id) ?? null;
  },
});

// Get by raw message ID
export const getByRawMessage = query({
  args: { rawMessageId: v.string() },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("reviewQueue")
      .withIndex("by_raw_message", (q) =>
        q.eq("rawMessageId", args.rawMessageId)
      )
      .collect();
  },
});

// Add item to review queue (matches entity/review_queue.rs)
export const add = mutation({
  args: {
    rawMessageId: v.string(),
    aiResult: v.any(),
    confidence: v.number(),
    reason: v.string(),
  },
  handler: async (ctx, args) => {
    // Check for existing pending review for same message
    const existing = await ctx.db
      .query("reviewQueue")
      .withIndex("by_raw_message", (q) =>
        q.eq("rawMessageId", args.rawMessageId)
      )
      .filter((q) => q.eq(q.field("status"), "pending"))
      .first();

    if (existing) {
      return existing._id;
    }

    return await ctx.db.insert("reviewQueue", {
      rawMessageId: args.rawMessageId,
      aiResult: args.aiResult,
      confidence: args.confidence,
      reason: args.reason,
      status: "pending",
      reviewedBy: undefined,
      reviewNotes: undefined,
      createdAt: Date.now(),
      reviewedAt: undefined,
    });
  },
});

// Complete review (approve)
export const complete = mutation({
  args: {
    id: v.id("reviewQueue"),
    reviewedBy: v.string(),
    notes: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    await ctx.db.patch(args.id, {
      status: "approved",
      reviewedBy: args.reviewedBy,
      reviewNotes: args.notes,
      reviewedAt: Date.now(),
    });
  },
});

// Dismiss review (reject/skip)
export const dismiss = mutation({
  args: {
    id: v.id("reviewQueue"),
    reviewedBy: v.string(),
    notes: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    await ctx.db.patch(args.id, {
      status: "rejected",
      reviewedBy: args.reviewedBy,
      reviewNotes: args.notes,
      reviewedAt: Date.now(),
    });
  },
});

// Get queue stats
export const stats = query({
  args: {},
  handler: async (ctx) => {
    const pending = await ctx.db
      .query("reviewQueue")
      .withIndex("by_status", (q) => q.eq("status", "pending"))
      .collect();

    const approved = await ctx.db
      .query("reviewQueue")
      .withIndex("by_status", (q) => q.eq("status", "approved"))
      .collect();

    const rejected = await ctx.db
      .query("reviewQueue")
      .withIndex("by_status", (q) => q.eq("status", "rejected"))
      .collect();

    const skipped = await ctx.db
      .query("reviewQueue")
      .withIndex("by_status", (q) => q.eq("status", "skipped"))
      .collect();

    return {
      pending: pending.length,
      approved: approved.length,
      rejected: rejected.length,
      skipped: skipped.length,
    };
  },
});
