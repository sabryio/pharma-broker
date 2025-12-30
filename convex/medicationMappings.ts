import { query, mutation } from "./_generated/server";
import { v } from "convex/values";

// List all medication mappings
export const list = query({
  args: { limit: v.optional(v.number()) },
  handler: async (ctx, args) => {
    return await ctx.db.query("medicationMappings").take(args.limit ?? 1000);
  },
});

// Get mapping by Arabic name
export const getByArabic = query({
  args: { arabicName: v.string() },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("medicationMappings")
      .withIndex("by_arabic", (q) => q.eq("arabicName", args.arabicName))
      .first();
  },
});

// Get mapping by English name
export const getByEnglish = query({
  args: { englishName: v.string() },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("medicationMappings")
      .withIndex("by_english", (q) => q.eq("englishName", args.englishName))
      .first();
  },
});

// Search by partial name (Arabic or English)
export const search = query({
  args: {
    query: v.string(),
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    const all = await ctx.db.query("medicationMappings").collect();
    const q = args.query.toLowerCase();
    const limit = args.limit ?? 50;

    return all
      .filter(
        (m) =>
          m.arabicName.toLowerCase().includes(q) ||
          m.englishName.toLowerCase().includes(q) ||
          (m.synonyms && m.synonyms.some((s) => s.toLowerCase().includes(q)))
      )
      .slice(0, limit);
  },
});

// Upsert medication mapping (matches entity/medication_mapping.rs)
export const upsert = mutation({
  args: {
    arabicName: v.string(),
    englishName: v.string(),
    synonyms: v.optional(v.array(v.string())),
    embedding: v.optional(v.array(v.number())),
  },
  handler: async (ctx, args) => {
    const existing = await ctx.db
      .query("medicationMappings")
      .withIndex("by_arabic", (q) => q.eq("arabicName", args.arabicName))
      .first();

    const now = Date.now();

    if (existing) {
      await ctx.db.patch(existing._id, {
        englishName: args.englishName,
        synonyms: args.synonyms,
        embedding: args.embedding,
        updatedAt: now,
      });
      return existing._id;
    }

    return await ctx.db.insert("medicationMappings", {
      arabicName: args.arabicName,
      englishName: args.englishName,
      synonyms: args.synonyms,
      embedding: args.embedding,
      createdAt: now,
      updatedAt: now,
    });
  },
});

// Bulk import mappings
export const bulkImport = mutation({
  args: {
    mappings: v.array(
      v.object({
        arabicName: v.string(),
        englishName: v.string(),
        synonyms: v.optional(v.array(v.string())),
      })
    ),
  },
  handler: async (ctx, args) => {
    let added = 0;
    let updated = 0;
    const now = Date.now();

    for (const mapping of args.mappings) {
      const existing = await ctx.db
        .query("medicationMappings")
        .withIndex("by_arabic", (q) => q.eq("arabicName", mapping.arabicName))
        .first();

      if (existing) {
        await ctx.db.patch(existing._id, {
          ...mapping,
          updatedAt: now,
        });
        updated++;
      } else {
        await ctx.db.insert("medicationMappings", {
          ...mapping,
          embedding: undefined,
          createdAt: now,
          updatedAt: now,
        });
        added++;
      }
    }

    return { added, updated };
  },
});

// Delete mapping
export const remove = mutation({
  args: { arabicName: v.string() },
  handler: async (ctx, args) => {
    const existing = await ctx.db
      .query("medicationMappings")
      .withIndex("by_arabic", (q) => q.eq("arabicName", args.arabicName))
      .first();

    if (existing) {
      await ctx.db.delete(existing._id);
    }
  },
});
