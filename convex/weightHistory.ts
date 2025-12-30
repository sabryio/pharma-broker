import { query, mutation } from "./_generated/server";
import { v } from "convex/values";

// Save weight history (matches entity/weight_history.rs)
export const save = mutation({
  args: {
    medicationWeight: v.number(),
    dosageWeight: v.number(),
    quantityWeight: v.number(),
    priceWeight: v.number(),
    recencyWeight: v.number(),
    source: v.string(),
    sampleCount: v.number(),
  },
  handler: async (ctx, args) => {
    return await ctx.db.insert("weightHistory", {
      ...args,
      createdAt: Date.now(),
    });
  },
});

// Get current (most recent) weights
export const getCurrent = query({
  args: {},
  handler: async (ctx) => {
    return await ctx.db
      .query("weightHistory")
      .withIndex("by_created")
      .order("desc")
      .first();
  },
});

// Get weight history
export const getHistory = query({
  args: { limit: v.optional(v.number()) },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("weightHistory")
      .withIndex("by_created")
      .order("desc")
      .take(args.limit ?? 50);
  },
});

// Get by ID
export const getById = query({
  args: { id: v.string() },
  handler: async (ctx, args) => {
    const items = await ctx.db.query("weightHistory").collect();
    return items.find((i) => i._id.toString() === args.id) ?? null;
  },
});

// Count all weight history entries
export const count = query({
  args: {},
  handler: async (ctx) => {
    const all = await ctx.db.query("weightHistory").collect();
    return all.length;
  },
});
