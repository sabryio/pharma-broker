import { mutation, query } from "./_generated/server";
import { v } from "convex/values";

// Save raw message with deduplication (matches entity/raw_message.rs)
export const save = mutation({
  args: {
    externalId: v.optional(v.string()),
    groupJid: v.string(),
    groupName: v.string(), // Required in SeaORM
    senderJid: v.string(),
    senderPhone: v.optional(v.string()),
    senderName: v.optional(v.string()),
    content: v.string(),
    timestamp: v.number(),
    replyToId: v.optional(v.string()),
    replyToContent: v.optional(v.string()),
    replyToSender: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    // Check for duplicate by external ID
    if (args.externalId) {
      const existing = await ctx.db
        .query("rawMessages")
        .withIndex("by_external_id", (q) => q.eq("externalId", args.externalId))
        .first();

      if (existing) {
        // Already exists, return existing ID
        return existing._id;
      }
    }

    return await ctx.db.insert("rawMessages", {
      ...args,
      processedAt: undefined,
      error: undefined,
      createdAt: Date.now(),
    });
  },
});

// Mark as processed
export const markProcessed = mutation({
  args: {
    id: v.id("rawMessages"),
    error: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    await ctx.db.patch(args.id, {
      processedAt: Date.now(),
      error: args.error,
    });
    return await ctx.db.get(args.id);
  },
});

// Get unprocessed messages
export const getUnprocessed = query({
  args: { limit: v.optional(v.number()) },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("rawMessages")
      .filter((q) => q.eq(q.field("processedAt"), undefined))
      .order("asc")
      .take(args.limit ?? 100);
  },
});

// Get by external ID
export const getByExternalId = query({
  args: { externalId: v.string() },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("rawMessages")
      .withIndex("by_external_id", (q) => q.eq("externalId", args.externalId))
      .first();
  },
});

// Get recent messages for a group
export const getByGroup = query({
  args: {
    groupJid: v.string(),
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("rawMessages")
      .withIndex("by_group", (q) => q.eq("groupJid", args.groupJid))
      .order("desc")
      .take(args.limit ?? 50);
  },
});

// Delete raw messages processed before cutoff (matches SeaORM delete_before)
export const deleteBefore = mutation({
  args: { cutoff: v.number() },
  handler: async (ctx, args) => {
    const toDelete = await ctx.db
      .query("rawMessages")
      .withIndex("by_processed", (q) => q.lt("processedAt", args.cutoff))
      .collect();

    for (const doc of toDelete) {
      await ctx.db.delete(doc._id);
    }
    return toDelete.length;
  },
});
