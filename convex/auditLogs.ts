import { mutation, query } from "./_generated/server";
import { v } from "convex/values";

// Log an action (matches entity/audit_log.rs)
export const log = mutation({
  args: {
    entityType: v.string(),
    entityId: v.string(),
    action: v.string(),
    actor: v.string(),
    details: v.optional(v.any()),
    ipAddress: v.optional(v.string()),
    userAgent: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    return await ctx.db.insert("auditLogs", {
      ...args,
      createdAt: Date.now(),
    });
  },
});

// Get logs for an entity
export const getByEntity = query({
  args: {
    entityType: v.string(),
    entityId: v.string(),
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("auditLogs")
      .withIndex("by_entity", (q) =>
        q.eq("entityType", args.entityType).eq("entityId", args.entityId)
      )
      .order("desc")
      .take(args.limit ?? 50);
  },
});

// Get recent logs
export const getRecent = query({
  args: { limit: v.optional(v.number()) },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("auditLogs")
      .order("desc")
      .take(args.limit ?? 100);
  },
});

// Get logs by actor
export const getByActor = query({
  args: {
    actor: v.string(),
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("auditLogs")
      .withIndex("by_actor", (q) => q.eq("actor", args.actor))
      .order("desc")
      .take(args.limit ?? 50);
  },
});

// Get logs by action
export const getByAction = query({
  args: {
    action: v.string(),
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("auditLogs")
      .withIndex("by_action", (q) => q.eq("action", args.action))
      .order("desc")
      .take(args.limit ?? 50);
  },
});

// Get logs by date range
export const getByDateRange = query({
  args: {
    start: v.number(),
    end: v.number(),
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("auditLogs")
      .withIndex("by_created", (q) =>
        q.gte("createdAt", args.start).lte("createdAt", args.end)
      )
      .order("desc")
      .take(args.limit ?? 100);
  },
});

// Count all logs
export const count = query({
  args: {},
  handler: async (ctx) => {
    const logs = await ctx.db.query("auditLogs").collect();
    return logs.length;
  },
});

// Delete audit logs created before cutoff (matches SeaORM delete_before)
export const deleteBefore = mutation({
  args: { cutoff: v.number() },
  handler: async (ctx, args) => {
    const toDelete = await ctx.db
      .query("auditLogs")
      .withIndex("by_created", (q) => q.lt("createdAt", args.cutoff))
      .collect();

    for (const doc of toDelete) {
      await ctx.db.delete(doc._id);
    }
    return toDelete.length;
  },
});
