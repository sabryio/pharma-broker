import { query, mutation } from "./_generated/server";
import { v } from "convex/values";

// List pending items in match queue
export const listPending = query({
  args: { limit: v.optional(v.number()) },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("matchQueueItems")
      .withIndex("by_status", (q) => q.eq("status", "PENDING"))
      .order("desc")
      .take(args.limit ?? 100);
  },
});

// Add to match queue (matches entity/match_queue.rs)
export const add = mutation({
  args: {
    requestId: v.string(),
    priority: v.number(),
  },
  handler: async (ctx, args) => {
    // Check for existing entry
    const existing = await ctx.db
      .query("matchQueueItems")
      .withIndex("by_request", (q) => q.eq("requestId", args.requestId))
      .filter((q) => q.eq(q.field("status"), "PENDING"))
      .first();

    if (existing) {
      return existing._id;
    }

    const now = Date.now();
    return await ctx.db.insert("matchQueueItems", {
      requestId: args.requestId,
      status: "PENDING",
      priority: args.priority,
      attempts: 0,
      lastError: undefined,
      nextAttemptAt: now,
      createdAt: now,
      updatedAt: now,
    });
  },
});

// Mark as processed (completed successfully)
export const markProcessed = mutation({
  args: { id: v.id("matchQueueItems") },
  handler: async (ctx, args) => {
    await ctx.db.patch(args.id, {
      status: "COMPLETED",
      updatedAt: Date.now(),
    });
  },
});

// Mark as failed
export const fail = mutation({
  args: {
    id: v.id("matchQueueItems"),
    error: v.string(),
  },
  handler: async (ctx, args) => {
    const item = await ctx.db.get(args.id);
    if (!item) throw new Error("Queue item not found");

    const now = Date.now();
    const nextAttempt = now + 60000 * Math.pow(2, item.attempts); // Exponential backoff

    await ctx.db.patch(args.id, {
      status: item.attempts >= 3 ? "FAILED" : "PENDING",
      attempts: item.attempts + 1,
      lastError: args.error,
      nextAttemptAt: nextAttempt,
      updatedAt: now,
    });
  },
});

// Get queue stats
export const stats = query({
  args: {},
  handler: async (ctx) => {
    const pending = await ctx.db
      .query("matchQueueItems")
      .withIndex("by_status", (q) => q.eq("status", "PENDING"))
      .collect();

    const processing = await ctx.db
      .query("matchQueueItems")
      .withIndex("by_status", (q) => q.eq("status", "PROCESSING"))
      .collect();

    const completed = await ctx.db
      .query("matchQueueItems")
      .withIndex("by_status", (q) => q.eq("status", "COMPLETED"))
      .collect();

    const failed = await ctx.db
      .query("matchQueueItems")
      .withIndex("by_status", (q) => q.eq("status", "FAILED"))
      .collect();

    return {
      pending: pending.length,
      processing: processing.length,
      completed: completed.length,
      failed: failed.length,
    };
  },
});

// Delete match queue items created before cutoff (matches SeaORM delete_before)
export const deleteBefore = mutation({
  args: { cutoff: v.number() },
  handler: async (ctx, args) => {
    const toDelete = await ctx.db
      .query("matchQueueItems")
      .withIndex("by_created", (q) => q.lt("createdAt", args.cutoff))
      .collect();

    for (const doc of toDelete) {
      await ctx.db.delete(doc._id);
    }
    return toDelete.length;
  },
});
