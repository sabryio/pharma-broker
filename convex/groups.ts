import { query, mutation } from "./_generated/server";
import { v } from "convex/values";

// List all groups ordered by name
export const list = query({
  args: {},
  handler: async (ctx) => {
    return await ctx.db.query("groups").withIndex("by_name").collect();
  },
});

// List monitored groups only ordered by name
export const listMonitored = query({
  args: {},
  handler: async (ctx) => {
    return await ctx.db
      .query("groups")
      .withIndex("by_monitored", (q) => q.eq("monitored", true))
      .collect();
  },
});

// Get group by JID
export const getByJid = query({
  args: { jid: v.string() },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("groups")
      .withIndex("by_jid", (q) => q.eq("jid", args.jid))
      .first();
  },
});

// Create or update group (upsert)
export const upsert = mutation({
  args: {
    jid: v.string(),
    name: v.string(),
    description: v.optional(v.string()),
    monitored: v.boolean(),
  },
  handler: async (ctx, args) => {
    const existing = await ctx.db
      .query("groups")
      .withIndex("by_jid", (q) => q.eq("jid", args.jid))
      .first();

    const now = Date.now();

    if (existing) {
      await ctx.db.patch(existing._id, {
        name: args.name,
        description: args.description,
        monitored: args.monitored,
      });
      return await ctx.db.get(existing._id);
    }

    const id = await ctx.db.insert("groups", {
      jid: args.jid,
      name: args.name,
      description: args.description,
      monitored: args.monitored,
      addedAt: now,
      lastMessage: undefined,
      messageCount: 0,
    });
    return await ctx.db.get(id);
  },
});

// Update last message timestamp
export const updateLastMessage = mutation({
  args: { jid: v.string() },
  handler: async (ctx, args) => {
    const group = await ctx.db
      .query("groups")
      .withIndex("by_jid", (q) => q.eq("jid", args.jid))
      .first();

    if (group) {
      await ctx.db.patch(group._id, {
        lastMessage: Date.now(),
      });
    }
  },
});

// Increment message count
export const incrementMessageCount = mutation({
  args: { jid: v.string() },
  handler: async (ctx, args) => {
    const group = await ctx.db
      .query("groups")
      .withIndex("by_jid", (q) => q.eq("jid", args.jid))
      .first();

    if (group) {
      await ctx.db.patch(group._id, {
        messageCount: group.messageCount + 1,
      });
    }
  },
});

// Set monitored status directly
export const setMonitored = mutation({
  args: { jid: v.string(), monitored: v.boolean() },
  handler: async (ctx, args) => {
    const group = await ctx.db
      .query("groups")
      .withIndex("by_jid", (q) => q.eq("jid", args.jid))
      .first();

    if (!group) {
      throw new Error(`Group not found: ${args.jid}`);
    }

    await ctx.db.patch(group._id, {
      monitored: args.monitored,
    });

    return await ctx.db.get(group._id);
  },
});

// Delete group
export const remove = mutation({
  args: { jid: v.string() },
  handler: async (ctx, args) => {
    const group = await ctx.db
      .query("groups")
      .withIndex("by_jid", (q) => q.eq("jid", args.jid))
      .first();

    if (group) {
      await ctx.db.delete(group._id);
    }
  },
});
